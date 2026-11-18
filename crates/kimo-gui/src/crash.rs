//! Opt-in crash reports. A panic hook writes a plain-text log to `<data>/crashes/crash-<unix ms>.log` (version, OS,
//! thread, message, source location, backtrace) and nothing else: no network, ever, from the hook. On the next launch
//! the app lists the logs nobody answered yet and asks the person whether to send a report. "Send report" opens a
//! pre-filled GitHub issue in their browser, which they read and submit themselves (their home folder is replaced by
//! `~` first); "Don't send" just files the log away. Either way the log stays on disk in `crashes/answered/` (the
//! newest few are kept), so nothing is lost and nothing leaves the computer without the person's own click.
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Where new issues go when the person chooses to send a report.
pub const ISSUES_URL: &str = "https://github.com/naman-sharma12345/kimo/issues/new";
/// Logs kept in `answered/` after the person answered the prompt.
const KEEP_ANSWERED: usize = 20;
/// Longest issue body put in a URL (browsers and `rundll32` cope with far more, GitHub truncates long query strings).
const MAX_BODY: usize = 6000;

pub fn dir(data_dir: &Path) -> PathBuf { data_dir.join("crashes") }

/// One crash the person has not answered yet.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Report {
    /// File name inside the crashes folder (what `answer` takes back).
    pub file: String,
    /// Full path, for "Show log".
    pub path: String,
    /// Unix ms of the crash.
    pub at: u64,
    /// The panic message, one line.
    pub message: String,
    /// The whole log.
    pub text: String,
}

/// The log for one panic.
pub fn render(version: &str, thread: &str, message: &str, location: &str, backtrace: &str, at_ms: u64) -> String {
    format!(
        "kimo crash report\nversion: {version}\nos: {} {}\ntime_ms: {at_ms}\nthread: {thread}\nmessage: {message}\nlocation: {location}\n\nbacktrace:\n{backtrace}\n",
        std::env::consts::OS, std::env::consts::ARCH,
    )
}

/// Write one log. Returns its path. Never panics (it runs inside the panic hook).
pub fn write(crash_dir: &Path, text: &str, at_ms: u64) -> std::io::Result<PathBuf> {
    write_with(crash_dir, text, at_ms, &|p, t| std::fs::write(p, t))
}

fn write_with(crash_dir: &Path, text: &str, at_ms: u64, persist: &dyn Fn(&Path, &str) -> std::io::Result<()>) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(crash_dir)?;
    let mut p = crash_dir.join(format!("crash-{at_ms}.log"));
    let mut n = 1;
    while p.exists() { p = crash_dir.join(format!("crash-{at_ms}-{n}.log")); n += 1; }
    persist(&p, text)?;
    Ok(p)
}

fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let p = info.payload();
    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "(no message)".into())
}

/// Install the hook: every panic in this process leaves a log in `crash_dir`, then the previous hook runs as before
/// (so the message still reaches stderr). Works with `panic = "abort"` too: the hook runs before the abort.
pub fn install(crash_dir: PathBuf, version: &'static str) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = now_ms();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let loc = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        let _ = write(&crash_dir, &render(version, &thread, &panic_message(info), &loc, &bt, at), at);
        prev(info);
    }));
}

fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> { text.lines().find_map(|l| l.strip_prefix(key).map(str::trim)) }

/// Crashes not answered yet, newest first.
pub fn pending(crash_dir: &Path) -> Vec<Report> {
    let Ok(rd) = std::fs::read_dir(crash_dir) else { return vec![] };
    let mut v: Vec<Report> = rd.flatten().filter_map(|e| {
        let p = e.path();
        let name = e.file_name().to_str()?.to_string();
        if !(name.starts_with("crash-") && name.ends_with(".log") && p.is_file()) { return None; }
        let text = std::fs::read_to_string(&p).ok()?;
        let at = field(&text, "time_ms:").and_then(|t| t.parse().ok()).unwrap_or(0);
        let message = field(&text, "message:").unwrap_or("").to_string();
        Some(Report { file: name, path: p.display().to_string(), at, message, text })
    }).collect();
    v.sort_by(|a, b| b.at.cmp(&a.at).then(b.file.cmp(&a.file)));
    v
}

/// The person answered the prompt (sent or not): file the log under `answered/` and keep only the newest few there.
/// `file` must be a bare log name from `pending` (no path parts).
pub fn answer(crash_dir: &Path, file: &str) -> Result<(), String> {
    if file.contains(['/', '\\']) || file.contains("..") || !file.starts_with("crash-") || !file.ends_with(".log") { return Err("not a crash log".into()); }
    let from = crash_dir.join(file);
    if !from.is_file() { return Ok(()); }
    let ans = crash_dir.join("answered");
    std::fs::create_dir_all(&ans).map_err(|e| e.to_string())?;
    std::fs::rename(&from, ans.join(file)).map_err(|e| e.to_string())?;
    let mut kept: Vec<PathBuf> = std::fs::read_dir(&ans).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect()).unwrap_or_default();
    kept.sort();
    let extra = kept.len().saturating_sub(KEEP_ANSWERED);
    for p in kept.into_iter().take(extra) { let _ = std::fs::remove_file(p); }
    Ok(())
}

/// The log as it would be sent: the person's home folder replaced by `~` (paths in backtraces and messages).
pub fn scrub(text: &str, home: Option<&Path>) -> String {
    scrub_platform(text, home, cfg!(windows))
}

fn scrub_platform(text: &str, home: Option<&Path>, windows: bool) -> String {
    let Some(h) = home.map(|h| h.display().to_string()).filter(|h| h.len() > 3) else { return text.to_string() };
    let mut out = text.replace(&h, "~");
    if windows { out = out.replace(&h.replace('\\', "/"), "~"); }
    out
}

fn pct(s: &str) -> String {
    let mut o = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { o.push(b as char) } else { o.push_str(&format!("%{b:02X}")) }
    }
    o
}

/// The pre-filled "new issue" page for one crash. Opening it sends nothing: the person reviews and submits it.
pub fn issue_url(r: &Report, home: Option<&Path>) -> String {
    let text = scrub(&r.text, home);
    let mut body = text.chars().take(MAX_BODY).collect::<String>();
    if body.len() < text.len() { body.push_str("\n…(log truncated; the full log is in the kimo data folder, crashes/answered)"); }
    let body = format!("kimo crashed. Log (home folder replaced by ~):\n\n```\n{body}\n```\n\nWhat I was doing:\n");
    let msg: String = r.message.chars().take(80).collect();
    format!("{ISSUES_URL}?labels=crash&title={}&body={}", pct(&format!("Crash: {msg}")), pct(&body))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("kimo-crash-{n}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); d }

    #[test]
    fn a_log_is_pending_until_answered_then_kept_locally() {
        let d = tmp("pending");
        assert!(pending(&d).is_empty(), "no folder, nothing pending");
        let t1 = render("9.0.0", "main", "index out of bounds", "src/main.rs:1:1", "0: frame", 1000);
        let t2 = render("9.0.0", "autosave", "boom", "src/recovery.rs:2:2", "", 2000);
        write(&d, &t1, 1000).unwrap();
        let p2 = write(&d, &t2, 2000).unwrap();
        write(&d, &t2, 2000).unwrap(); // same millisecond: a second file, not an overwrite
        let p = pending(&d);
        assert_eq!(p.len(), 3);
        assert_eq!((p[0].at, p.last().unwrap().message.as_str()), (2000, "index out of bounds"), "newest first");
        assert!(p[0].text.contains("thread: autosave"));
        answer(&d, p2.file_name().unwrap().to_str().unwrap()).unwrap();
        assert_eq!(pending(&d).len(), 2);
        assert!(d.join("answered").join(p2.file_name().unwrap()).is_file(), "an answered log stays on this computer");
        assert!(answer(&d, "../settings.json").is_err() && answer(&d, "crash-1/../../x.log").is_err(), "only crash logs, only in the folder");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn answered_logs_are_capped() {
        let d = tmp("cap");
        for i in 0..(KEEP_ANSWERED as u64 + 5) { let p = write(&d, "x", 10_000 + i).unwrap(); answer(&d, p.file_name().unwrap().to_str().unwrap()).unwrap(); }
        assert_eq!(std::fs::read_dir(d.join("answered")).unwrap().count(), KEEP_ANSWERED);
        assert!(d.join("answered").join(format!("crash-{}.log", 10_000 + KEEP_ANSWERED as u64 + 4)).is_file(), "the newest are kept");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_report_link_hides_the_home_folder_and_is_one_url() {
        let home = PathBuf::from("/home/someone");
        let text = render("9.0.0", "main", "bad & worse", &format!("{}/src/x.rs:1:1", home.display()), "", 5);
        let r = Report { file: "crash-5.log".into(), path: String::new(), at: 5, message: "bad & worse".into(), text };
        let u = issue_url(&r, Some(&home));
        assert!(u.starts_with("https://github.com/") && u.contains("labels=crash"), "{u}");
        assert!(!u.contains("someone"), "the home folder never leaves in the link: {u}");
        assert!(!u.contains(' ') && !u.contains('\n') && u.contains("bad%20%26%20worse"));
        let long = Report { text: "y".repeat(50_000), ..r };
        assert!(issue_url(&long, None).len() < 30_000, "a huge log is truncated in the link");
    }

    #[test]
    fn the_hook_writes_a_log_for_a_panic() {
        let d = tmp("hook");
        install(d.clone(), "test");
        let _ = std::thread::Builder::new().name("forced".into()).spawn(|| panic!("forced for the test")).unwrap().join();
        let _ = std::panic::take_hook(); // back to the default hook
        // other tests may panic meanwhile (their logs land here too): look for ours
        let p = pending(&d);
        let r = p.iter().find(|r| r.message == "forced for the test").expect("the forced panic left a log");
        assert!(r.text.contains("thread: forced") && r.text.contains("version: test") && r.text.contains("crash.rs"), "{}", r.text);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn malformed_logs_and_archive_errors_preserve_pending_reports() {
        let root = tmp("failures");
        let d = dir(&root);
        std::fs::create_dir_all(&d).unwrap();
        let blocker = root.join("file");
        std::fs::write(&blocker, b"original").unwrap();
        assert!(write(&blocker, "report", 1).is_err());
        std::fs::write(d.join("crash-invalid.log"), b"time_ms: invalid\n").unwrap();
        std::fs::write(d.join("crash-unreadable.log"), [0xff, 0xfe]).unwrap();
        std::fs::write(d.join("unrelated.log"), b"not a report").unwrap();
        std::fs::create_dir(d.join("crash-directory.log")).unwrap();
        let reports = pending(&d);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].at, 0);
        assert_eq!(reports[0].message, "");
        for invalid in ["plain.log", "crash-1.txt", "crash-..log", "crash-sub\\x.log"] {
            assert_eq!(answer(&d, invalid).unwrap_err(), "not a crash log");
        }
        answer(&d, "crash-missing.log").unwrap();
        let archive = d.join("answered");
        std::fs::write(&archive, b"block archive directory").unwrap();
        assert!(answer(&d, "crash-invalid.log").is_err());
        assert_eq!(pending(&d).len(), 1);
        std::fs::remove_file(&archive).unwrap();
        std::fs::create_dir_all(archive.join("crash-invalid.log")).unwrap();
        assert!(answer(&d, "crash-invalid.log").is_err());
        assert_eq!(pending(&d).len(), 1);
        assert_eq!(scrub("retain /x", Some(Path::new("/x"))), "retain /x");
        assert_eq!(scrub_platform(r"C:\Users\fixture\x and C:/Users/fixture/y", Some(Path::new(r"C:\Users\fixture")), true), r"~\x and ~/y");
        assert_eq!(scrub_platform("/home/fixture/x", Some(Path::new("/home/fixture")), false), "~/x");
        let error = write_with(&d, "must persist", 99, &|path, text| {
            assert_eq!(path, d.join("crash-99.log"));
            assert_eq!(text, "must persist");
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "fixture storage denied"))
        }).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(!d.join("crash-99.log").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_log_names_are_not_offered() {
        use std::os::unix::ffi::OsStringExt;
        let d = tmp("nonutf8");
        std::fs::create_dir_all(&d).unwrap();
        let name = std::ffi::OsString::from_vec(b"crash-\xff.log".to_vec());
        std::fs::write(d.join(name), b"message: ignored").unwrap();
        assert!(pending(&d).is_empty());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn non_unicode_log_names_are_not_offered() {
        use std::os::windows::ffi::OsStringExt;
        let d = tmp("nonunicode");
        std::fs::create_dir_all(&d).unwrap();
        let units: Vec<u16> = "crash-".encode_utf16().chain([0xd800]).chain(".log".encode_utf16()).collect();
        std::fs::write(d.join(std::ffi::OsString::from_wide(&units)), b"message: ignored").unwrap();
        assert!(pending(&d).is_empty());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn hook_preserves_string_and_non_string_panic_payloads_in_a_child_process() {
        const CHILD: &str = "KIMO_TEST_CRASH_PAYLOADS";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "crash::tests::hook_preserves_string_and_non_string_panic_payloads_in_a_child_process", "--nocapture"])
                .env(CHILD, "1").output().unwrap();
            assert!(output.status.success(), "{} {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            return;
        }
        let d = tmp("payloads");
        install(d.clone(), "payload-test");
        assert!(std::thread::spawn(|| std::panic::panic_any(String::from("owned panic"))).join().is_err());
        assert!(std::thread::spawn(|| std::panic::panic_any(42_u32)).join().is_err());
        let reports = pending(&d);
        assert_eq!(reports.len(), 2);
        assert!(reports.iter().any(|r| r.message == "owned panic"));
        assert!(reports.iter().any(|r| r.message == "(no message)"));
        assert!(reports.iter().all(|r| r.text.contains("thread: unnamed") && r.at > 0));
        std::fs::remove_dir_all(d).unwrap();
    }
}
