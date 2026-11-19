//! Analysis proxies (analysis_proxy.rs): building a short-GOP proxy, re-using it from the cache (sidecar stamp), and
//! finding it again from a project asset. The source is a 64x64, 10 fps, 3 s lavfi clip with one H.264 keyframe every
//! 3 s, so every encode takes well under a second.
use kimo_core::analysis_proxy::{ensure, max_keyframe_gap, prebuild};
use kimo_core::commands::{dispatch, Session};
use serde_json::json;

fn tmpdir(tag: &str) -> std::path::PathBuf { let d = std::env::temp_dir().join(format!("kimo-ap-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
fn clip(dir: &std::path::Path, name: &str, codec: &[&str]) -> String {
    let p = dir.join(name); let mut a = vec!["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=s=64x64:r=10:d=3"]; a.extend_from_slice(codec); a.push(p.to_str().unwrap());
    assert!(std::process::Command::new("ffmpeg").args(&a).status().unwrap().success()); p.to_str().unwrap().to_string()
}

#[test] fn prebuild_caches_and_ensure_reuses_the_proxy() {
    let d = tmpdir("build"); let cache = d.join("cache");
    std::env::set_var("KIMO_ANALYSIS_PROXY_CACHE", &cache);
    let src = clip(&d, "src.mp4", &["-c:v", "libx264", "-g", "30", "-keyint_min", "30", "-sc_threshold", "0", "-bf", "0"]);
    let mut seen = vec![]; let ap = prebuild(None, &src, 3.0, 10.0, None, &mut |f| seen.push(f)).unwrap();
    assert_eq!((ap.gop, ap.fps), (5, 10.0)); assert!(std::path::Path::new(&ap.path).is_file()); assert!(ap.path.starts_with(cache.to_str().unwrap()));
    assert!(seen.iter().all(|f| (0.0..=1.0).contains(f)));
    let side: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(&ap.path).with_extension("json")).unwrap()).unwrap();
    assert_eq!((side["gop"].as_u64(), side["fps"].as_f64()), (Some(5), Some(10.0)));
    assert!(max_keyframe_gap(&ap.path, 3.0).unwrap() < 0.65, "the proxy has a keyframe every half second");
    let again = prebuild(None, &src, 3.0, 10.0, Some(0.0), &mut |_| {}).unwrap(); assert_eq!(again.path, ap.path, "a matching sidecar is a cache hit, whatever the gap");
    // a project asset finds the cached proxy; an unknown asset or a still has none
    let mut s = Session::new(); dispatch(&mut s, "project_set", &json!({"width": 64, "height": 64, "fps": 10})).unwrap();
    let a = dispatch(&mut s, "import_media", &json!({"path": src})).unwrap()["asset_id"].as_str().unwrap().to_string();
    assert_eq!(ensure(&s.project, &a, &src).map(|p| p.path), Some(ap.path.clone()));
    // a corrupt sidecar is a miss: an unknown asset then has nothing to build from
    std::fs::write(std::path::Path::new(&ap.path).with_extension("json"), "{").unwrap();
    assert!(ensure(&s.project, "nope", &src).is_none());
    let rebuilt = ensure(&s.project, &a, &src).unwrap(); assert_eq!(rebuilt.gop, 5, "rebuilt after the corrupt sidecar");
    assert_eq!(prebuild(None, &src, 3.0, 10.0, Some(0.5), &mut |_| {}).map(|p| p.gop), Ok(5), "cached again");
    std::fs::write(std::path::Path::new(&ap.path).with_extension("json"), json!({"source_key": "other", "gop": 5}).to_string()).unwrap();
    assert_eq!(prebuild(None, &src, 3.0, 10.0, Some(0.5), &mut |_| {}).err().unwrap(), "keyframes are already frequent, or the analysis encode failed", "a stale stamp is not reused");
    let _ = std::fs::remove_dir_all(&d);
}
