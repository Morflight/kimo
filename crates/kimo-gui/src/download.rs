//! First-run downloads (FFmpeg, whisper, the speech model): resumable over HTTP Range into `<dest>.part`, SHA-512
//! checked before the file is moved into place, mirrors tried in order, progress with speed and time left.
//! The parts list is data (`parts.json`, REFERENCE ONLY values the maintainer pins at release time).
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Part {
    pub id: String,
    pub name: String,
    /// Mirrors, tried in order.
    pub urls: Vec<String>,
    pub sha512: String,
    pub size: u64,
    /// Path under the app's data dir, e.g. "tools/ffmpeg" or "models/ggml-base.en.bin".
    pub dest: String,
    #[serde(default)]
    pub executable: bool,
    /// Optional parts can be skipped (e.g. the speech model when the user does not need captions).
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub note: String,
    /// "linux", "windows", "macos", or "" for every OS.
    #[serde(default)]
    pub os: String,
}

/// The parts for the OS this binary was built for.
pub fn for_this_os(parts: Vec<Part>) -> Vec<Part> {
    parts.into_iter().filter(|p| p.os.is_empty() || p.os == std::env::consts::OS).collect()
}

/// A part whose pin is not filled in yet (not uploaded): the window shows it as unavailable instead of fetching.
pub fn pinned(p: &Part) -> bool { p.sha512.len() == 128 && p.size > 0 }

#[derive(Clone, Debug, Serialize)]
pub struct Progress { pub id: String, pub done: u64, pub total: u64, pub bytes_per_s: f64, pub eta_s: Option<f64>, pub phase: &'static str, pub mirror: usize }

#[derive(Debug, PartialEq)]
pub enum Failure { Offline(String), Checksum { expected: String, got: String }, Http(u16, String), Io(String), Cancelled }
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Offline(m) => write!(f, "No connection to the download server ({m}). Check your internet and press Retry."),
            Failure::Checksum { .. } => write!(f, "The file did not match its SHA-512 fingerprint, so it was thrown away. Press Retry."),
            Failure::Http(c, u) => write!(f, "The server answered {c} for {u}."),
            Failure::Io(m) => write!(f, "Could not write the file: {m}"),
            Failure::Cancelled => write!(f, "Paused."),
        }
    }
}

pub fn sha512_file(p: &Path) -> std::io::Result<String> {
    sha512_reader(&mut std::fs::File::open(p)?)
}

fn sha512_reader(f: &mut dyn Read) -> std::io::Result<String> {
    let mut h = Sha512::new();
    let mut buf = vec![0u8; 1 << 16];
    loop { let n = f.read(&mut buf)?; if n == 0 { break; } h.update(&buf[..n]); }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// True when the part is already in place with the right fingerprint.
pub fn installed(root: &Path, p: &Part) -> bool {
    let d = root.join(&p.dest);
    d.metadata().map(|m| m.len() == p.size).unwrap_or(false) && sha512_file(&d).map(|h| h.eq_ignore_ascii_case(&p.sha512)).unwrap_or(false)
}

pub fn load_parts(json: &str) -> Result<Vec<Part>, String> {
    #[derive(Deserialize)] struct M { parts: Vec<Part> }
    serde_json::from_str::<M>(json).map(|m| m.parts).map_err(|e| e.to_string())
}

/// Download one part, resuming a previous `.part` file. `on` is called at most ~10 times a second.
pub fn fetch(root: &Path, p: &Part, cancel: &AtomicBool, on: &mut dyn FnMut(Progress)) -> Result<PathBuf, Failure> {
    let dest = root.join(&p.dest);
    if installed(root, p) { on(Progress { id: p.id.clone(), done: p.size, total: p.size, bytes_per_s: 0.0, eta_s: Some(0.0), phase: "done", mirror: 0 }); return Ok(dest); }
    if let Some(d) = dest.parent() { std::fs::create_dir_all(d).map_err(|e| Failure::Io(e.to_string()))?; }
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let mut last = Failure::Offline("no mirror listed".into());
    for (mi, url) in p.urls.iter().enumerate() {
        for _attempt in 0..3 {
            match fetch_from(url, &part, p, mi, cancel, on) {
                Ok(()) => {
                    on(Progress { id: p.id.clone(), done: p.size, total: p.size, bytes_per_s: 0.0, eta_s: None, phase: "verifying", mirror: mi });
                    let got = sha512_file(&part).map_err(|e| Failure::Io(e.to_string()))?;
                    if !got.eq_ignore_ascii_case(&p.sha512) {
                        let _ = std::fs::remove_file(&part);
                        last = Failure::Checksum { expected: p.sha512.clone(), got };
                        break; // a bad file from this mirror: next mirror
                    }
                    std::fs::rename(&part, &dest).map_err(|e| Failure::Io(e.to_string()))?;
                    #[cfg(unix)]
                    if p.executable { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755)); }
                    on(Progress { id: p.id.clone(), done: p.size, total: p.size, bytes_per_s: 0.0, eta_s: Some(0.0), phase: "done", mirror: mi });
                    return Ok(dest);
                }
                Err(Failure::Cancelled) => return Err(Failure::Cancelled),
                Err(Failure::Http(c, u)) => { last = Failure::Http(c, u); break; } // 404 etc: next mirror
                Err(e) => { last = e; std::thread::sleep(Duration::from_millis(300)); } // dropped: resume from the same mirror
            }
        }
    }
    Err(last)
}

fn fetch_from(url: &str, part: &Path, p: &Part, mirror: usize, cancel: &AtomicBool, on: &mut dyn FnMut(Progress)) -> Result<(), Failure> {
    let have = part.metadata().map(|m| m.len()).unwrap_or(0);
    if have == p.size { return Ok(()); }
    if have > p.size { let _ = std::fs::remove_file(part); }
    let have = part.metadata().map(|m| m.len()).unwrap_or(0);
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(30)).build();
    let mut req = agent.get(url);
    if have > 0 { req = req.set("Range", &format!("bytes={have}-")); }
    let resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(416, _)) => { let _ = std::fs::remove_file(part); return Err(Failure::Offline("range refused, restarting".into())); }
        Err(ureq::Error::Status(c, _)) => return Err(Failure::Http(c, url.to_string())),
        Err(ureq::Error::Transport(t)) => return Err(Failure::Offline(t.kind().to_string())),
    };
    let resumed = resp.status() == 206;
    let mut f = std::fs::OpenOptions::new().create(true).append(resumed).write(true).truncate(!resumed).open(part).map_err(|e| Failure::Io(e.to_string()))?;
    copy_body(&mut resp.into_reader(), &mut f, p, mirror, cancel, on, if resumed { have } else { 0 })
}

fn copy_body(r: &mut dyn Read, f: &mut dyn Write, p: &Part, mirror: usize, cancel: &AtomicBool, on: &mut dyn FnMut(Progress), start_done: u64) -> Result<(), Failure> {
    let mut done = start_done;
    let t0 = Instant::now();
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut buf = vec![0u8; 1 << 16];
    loop {
        if cancel.load(Ordering::SeqCst) { let _ = f.flush(); return Err(Failure::Cancelled); }
        let n = match r.read(&mut buf) { Ok(0) => break, Ok(n) => n, Err(e) => { let _ = f.flush(); return Err(Failure::Offline(e.to_string())); } };
        f.write_all(&buf[..n]).map_err(|e| Failure::Io(e.to_string()))?;
        done += n as u64;
        if last_emit.elapsed() >= Duration::from_millis(100) {
            last_emit = Instant::now();
            let el = t0.elapsed().as_secs_f64().max(1e-3);
            let bps = (done - start_done) as f64 / el;
            // This branch follows a nonempty read; elapsed is clamped positive, so bps is positive.
            let eta = Some((p.size.saturating_sub(done)) as f64 / bps);
            on(Progress { id: p.id.clone(), done, total: p.size, bytes_per_s: bps, eta_s: eta, phase: "downloading", mirror });
        }
    }
    f.flush().map_err(|e| Failure::Io(e.to_string()))?;
    if done < p.size { return Err(Failure::Offline(format!("connection dropped at {done} of {} bytes", p.size))); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{serve, Resp};
    use std::sync::{atomic::AtomicUsize, Arc};

    fn ignore_progress(_: Progress) {}
    fn body() -> Vec<u8> { (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect() }
    fn hex(b: &[u8]) -> String { Sha512::digest(b).iter().map(|x| format!("{x:02x}")).collect() }
    fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("kimo-dl-{n}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }

    /// Serves /good with Range support; the first full request drops after 100 KB.
    fn fixture(tamper: bool) -> (crate::testutil::Fixture, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let h2 = hits.clone();
        let fx = serve(move |r| {
            let mut b = body();
            if tamper { b[5] ^= 1; }
            if r.path != "/good" { return Resp::bytes(404, b"no".to_vec()); }
            let n = h2.fetch_add(1, Ordering::SeqCst);
            match r.header("range").and_then(|v| v.strip_prefix("bytes=")).and_then(|v| v.trim_end_matches('-').parse::<usize>().ok()) {
                Some(from) => { let mut resp = Resp::bytes(206, b[from..].to_vec()); resp.headers.push(("Content-Range".into(), format!("bytes {from}-{}/{}", b.len() - 1, b.len()))); resp }
                None => { let mut resp = Resp::bytes(200, b); if n == 0 { resp.drop_after = Some(100_000); } resp }
            }
        });
        (fx, hits)
    }
    fn part(base: &str, urls: &[&str]) -> Part {
        let b = body();
        Part { id: "ffmpeg".into(), name: "FFmpeg".into(), urls: urls.iter().map(|u| format!("{base}{u}")).collect(), sha512: hex(&b), size: b.len() as u64, dest: "tools/ffmpeg".into(), executable: true, optional: false, note: String::new(), os: String::new() }
    }

    #[test]
    fn drop_then_resume_with_range_and_verify() {
        let (fx, hits) = fixture(false);
        let root = tmp("resume");
        let p = part(&fx.base, &["/missing", "/good"]); // first mirror 404s
        let mut seen = vec![];
        let out = fetch(&root, &p, &AtomicBool::new(false), &mut |pr| seen.push(pr.phase)).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), body());
        assert!(hits.load(Ordering::SeqCst) >= 2, "dropped once, resumed");
        let log = fx.log.lock().unwrap();
        assert!(log.iter().any(|r| r.header("range") == Some("bytes=100000-")), "resumed from the bytes already on disk");
        assert!(seen.contains(&"verifying") && seen.last() == Some(&"done"));
        assert!(!root.join("tools/ffmpeg.part").exists());
        drop(log);
        assert!(installed(&root, &p));
        // a second run is a no-op
        let before = hits.load(Ordering::SeqCst);
        fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress).unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tampered_file_is_refused_and_removed() {
        let (fx, _) = fixture(true);
        let root = tmp("tamper");
        let p = part(&fx.base, &["/good"]);
        let e = fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress).unwrap_err();
        assert!(matches!(e, Failure::Checksum { .. }), "{e:?}");
        assert!(!root.join("tools/ffmpeg").exists() && !root.join("tools/ffmpeg.part").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn offline_is_a_plain_error() {
        let root = tmp("offline");
        let p = part("http://127.0.0.1:9", &["/good"]);
        let e = fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress).unwrap_err();
        assert!(matches!(e, Failure::Offline(_)), "{e:?}");
        assert!(e.to_string().contains("Retry"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cancel_keeps_partial_for_resume() {
        let (fx, _) = fixture(false);
        let root = tmp("cancel");
        let p = part(&fx.base, &["/good"]);
        let cancel = AtomicBool::new(true);
        assert_eq!(fetch(&root, &p, &cancel, &mut ignore_progress).unwrap_err(), Failure::Cancelled);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parts_manifest_parses() {
        let m = r#"{"parts":[{"id":"model","name":"Speech model","urls":["https://x/y"],"sha512":"ab","size":1,"dest":"models/m.bin","optional":true}]}"#;
        let p = load_parts(m).unwrap();
        assert!(p[0].optional && !p[0].executable);
    }
    #[test]
    fn manifest_filters_pins_and_failure_messages_are_explicit() {
        let mut p = part("", &[]);
        assert!(pinned(&p));
        p.sha512 = "not-pinned".into(); assert!(!pinned(&p));
        p.sha512 = hex(&body()); p.size = 0; assert!(!pinned(&p));
        let mut local = p.clone(); local.os = std::env::consts::OS.into();
        let mut other = p.clone(); other.os = "different-os".into();
        assert_eq!(for_this_os(vec![p.clone(), local.clone(), other]), vec![p, local]);
        assert!(load_parts("invalid").unwrap_err().contains("expected"));
        assert!(Failure::Checksum { expected: "a".into(), got: "b".into() }.to_string().contains("SHA-512"));
        assert_eq!(Failure::Http(404, "fixture".into()).to_string(), "The server answered 404 for fixture.");
        assert_eq!(Failure::Io("blocked".into()).to_string(), "Could not write the file: blocked");
        assert_eq!(Failure::Cancelled.to_string(), "Paused.");
    }

    #[test]
    fn complete_partial_skips_network_and_invalid_ranges_restart() {
        let root = tmp("partial-boundaries");
        let fx = serve(|r| if r.header("range").is_some() { Resp::bytes(416, vec![]) } else { Resp::bytes(200, body()) });
        let p = part(&fx.base, &["/good"]);
        std::fs::create_dir_all(root.join("tools")).unwrap();
        let partial = root.join("tools/ffmpeg.part");
        std::fs::write(&partial, body()).unwrap();
        assert_eq!(fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress).unwrap(), root.join(&p.dest));
        assert!(fx.log.lock().unwrap().is_empty());
        std::fs::remove_file(root.join(&p.dest)).unwrap();
        std::fs::write(&partial, b"partial").unwrap();
        fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress).unwrap();
        assert!(installed(&root, &p));
        assert_eq!(fx.log.lock().unwrap().len(), 2);
        std::fs::remove_file(root.join(&p.dest)).unwrap();
        std::fs::write(&partial, vec![0; p.size as usize + 1]).unwrap();
        fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress).unwrap();
        assert!(installed(&root, &p));
        assert!(fx.log.lock().unwrap().last().unwrap().header("range").is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn filesystem_failures_preserve_existing_files() {
        let root = tmp("io-boundaries");
        let p = part("", &[]);
        std::fs::write(root.join("tools"), b"keep").unwrap();
        assert!(matches!(fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress), Err(Failure::Io(_))));
        assert_eq!(std::fs::read(root.join("tools")).unwrap(), b"keep");
        std::fs::remove_file(root.join("tools")).unwrap();
        assert!(matches!(fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress), Err(Failure::Offline(ref m)) if m == "no mirror listed"));
        let fx = serve(|_| Resp::bytes(200, body()));
        let p = part(&fx.base, &["/good"]);
        std::fs::create_dir(root.join("tools/ffmpeg.part")).unwrap();
        assert!(matches!(fetch_from(&p.urls[0], &root.join("tools/ffmpeg.part"), &p, 0, &AtomicBool::new(false), &mut ignore_progress), Err(Failure::Io(_))));
        std::fs::remove_dir(root.join("tools/ffmpeg.part")).unwrap();
        std::fs::create_dir(root.join(&p.dest)).unwrap();
        assert!(matches!(fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress), Err(Failure::Io(_))));
        assert_eq!(std::fs::read(root.join("tools/ffmpeg.part")).unwrap(), body());
        assert!(root.join(&p.dest).is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verification_io_and_short_bodies_never_publish_a_download() {
        struct InterruptedDownload(std::io::Cursor<Vec<u8>>);
        impl Read for InterruptedDownload {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.position() == self.0.get_ref().len() as u64 {
                    Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "owned download interrupted"))
                } else { self.0.read(buf) }
            }
        }
        let error = sha512_reader(&mut InterruptedDownload(std::io::Cursor::new(body()))).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
        assert_eq!(error.to_string(), "owned download interrupted");
        let root = tmp("verification-errors");
        assert!(sha512_file(&root.join("missing")).is_err());
        assert!(sha512_file(&root).is_err());
        let fx = serve(|_| Resp::bytes(200, body()));
        let mut p = part(&fx.base, &["/good"]);
        let partial = root.join("tools/ffmpeg.part");
        let result = fetch(&root, &p, &AtomicBool::new(false), &mut |progress| {
            if progress.phase == "verifying" { std::fs::remove_file(&partial).unwrap(); }
        });
        assert!(matches!(result, Err(Failure::Io(_))));
        assert!(!root.join(&p.dest).exists());
        let short = serve(|_| Resp::bytes(200, b"short".to_vec()));
        let failure = fetch_from(&short.base, &partial, &p, 0, &AtomicBool::new(false), &mut ignore_progress).unwrap_err();
        assert_eq!(failure, Failure::Offline(format!("connection dropped at 5 of {} bytes", p.size)));
        assert_eq!(std::fs::read(&partial).unwrap(), b"short");
        p.executable = false;
        fetch(&root, &p, &AtomicBool::new(false), &mut ignore_progress).unwrap();
        assert!(installed(&root, &p));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transfer_write_flush_and_read_errors_keep_their_failure_kind() {
        struct BrokenWriter { flush_only: bool }
        impl Write for BrokenWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.flush_only { Ok(bytes.len()) } else { Err(std::io::Error::other("synthetic write failure")) }
            }
            fn flush(&mut self) -> std::io::Result<()> { Err(std::io::Error::other("synthetic flush failure")) }
        }
        struct BrokenReader;
        impl Read for BrokenReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> { Err(std::io::Error::other("synthetic read failure")) }
        }
        let p = part("", &[]);
        for flush_only in [false, true] {
            let error = copy_body(&mut &body()[..], &mut BrokenWriter { flush_only }, &p, 0, &AtomicBool::new(false), &mut ignore_progress, 0).unwrap_err();
            assert_eq!(error, Failure::Io(if flush_only { "synthetic flush failure" } else { "synthetic write failure" }.into()));
        }
        assert_eq!(copy_body(&mut BrokenReader, &mut Vec::new(), &p, 0, &AtomicBool::new(false), &mut ignore_progress, 0), Err(Failure::Offline("synthetic read failure".into())));
    }

    #[test]
    fn empty_destination_and_repeated_full_requests_are_explicit() {
        let mut p = part("", &[]); p.dest.clear();
        assert_eq!(fetch(Path::new(""), &p, &AtomicBool::new(false), &mut ignore_progress), Err(Failure::Offline("no mirror listed".into())));
        let (fx, hits) = fixture(false);
        for attempt in 0..2 {
            let response = ureq::get(&format!("{}/good", fx.base)).call().unwrap();
            assert_eq!(response.status(), 200);
            let mut bytes = Vec::new();
            let result = response.into_reader().read_to_end(&mut bytes);
            if attempt == 0 { assert!(result.is_err()); assert_eq!(bytes.len(), 100_000); }
            else { result.unwrap(); assert_eq!(bytes, body()); }
        }
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

}
