//! audio.rs: onset/beat detection and tempo on a synthetic 3 s click track at 120 bpm.
use kimo_core::audio::detect_beats;

fn tmp(name: &str) -> String { std::env::temp_dir().join(format!("kimo-audiomix-{}-{name}", std::process::id())).to_string_lossy().into_owned() }
fn ff(args: &[&str]) { assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y"]).args(args).status().unwrap().success(), "{args:?}"); }

#[test]
fn beats_are_found_on_each_click_and_the_tempo_reads_120() {
    let w = tmp("clicks.wav");
    ff(&["-f", "lavfi", "-i", "aevalsrc='0.7*exp(-mod(t\\,0.5)*40)*sin(2*PI*1200*mod(t\\,0.5))':s=22050:d=3", "-c:a", "pcm_s16le", &w]);
    let beats = detect_beats(&w, 0.25);
    assert!(beats.len() >= 5, "{beats:?}");
    for b in &beats { let k = (b / 0.5).round(); assert!((b - k * 0.5).abs() < 0.05, "beat {b} is not on the 0.5 s grid: {beats:?}"); }
    let (_, bpm) = kimo_core::audio::analyze(&w, 0.25);
    assert!((bpm - 120.0).abs() < 4.0, "bpm {bpm}");
    assert!(detect_beats(&tmp("missing.wav"), 0.25).is_empty(), "an unreadable file has no beats");
    std::fs::remove_file(w).ok();
}
