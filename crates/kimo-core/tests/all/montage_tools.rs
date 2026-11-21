//! montage_tools (V8.3-M): score_windows / best_segments -- the non-dialogue sibling of auto_rough_cut. Each test here
//! checks a real editorial outcome (junk actually gets dropped, good footage actually survives), not just "it returns ok".
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn dir() -> PathBuf { let d = std::env::temp_dir().join(format!("kimo_montage_tools_{}", std::process::id())); std::fs::create_dir_all(&d).unwrap(); d }
fn ffmpeg(args: &[&str]) { let st = Command::new("ffmpeg").args(["-v", "error", "-y"]).args(args).stdin(Stdio::null()).status().expect("ffmpeg"); assert!(st.success(), "ffmpeg {args:?}"); }
fn call(ss: &mut Session, name: &str, a: Value) -> Value { dispatch(ss, name, &a).unwrap_or_else(|e| panic!("{name} {a}: {e}")) }
fn sid(v: &Value, k: &str) -> String { v[k].as_str().unwrap_or_else(|| panic!("no {k} in {v}")).to_string() }
fn proj() -> Session { let mut ss = Session::new(); call(&mut ss, "project_set", json!({"width": 640, "height": 360, "fps": 30})); ss }

/// A clean, sharp, well-exposed, locked-off clip: a Mandelbrot zoom (lots of fine detail, no camera motion).
fn good(name: &str, dur: f64) -> String {
    let p = dir().join(name); if p.exists() { return p.to_string_lossy().into_owned(); }
    ffmpeg(&["-f", "lavfi", "-i", &format!("mandelbrot=s=640x360:r=30:end_pts={}", (dur * 30.0) as i64), "-t", &dur.to_string(), "-an", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}
/// Heavily blurred: same source, gblur -- should score low on sharpness.
fn blurry(name: &str, dur: f64) -> String {
    let p = dir().join(name); if p.exists() { return p.to_string_lossy().into_owned(); }
    ffmpeg(&["-f", "lavfi", "-i", &format!("mandelbrot=s=640x360:r=30:end_pts={}", (dur * 30.0) as i64), "-t", &dur.to_string(), "-vf", "gblur=sigma=20", "-an", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}
/// Near-black: should score low on exposure.
fn dark(name: &str, dur: f64) -> String {
    let p = dir().join(name); if p.exists() { return p.to_string_lossy().into_owned(); }
    ffmpeg(&["-f", "lavfi", "-i", &format!("testsrc2=s=640x360:r=30:d={dur}"), "-vf", "colorchannelmixer=rr=0.03:gg=0.03:bb=0.03", "-an", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}
/// Jittery hand-held stand-in: an oversized source jittered by a fast sin/cos crop pan -- should score high on shake.
fn shaky(name: &str, dur: f64) -> String {
    let p = dir().join(name); if p.exists() { return p.to_string_lossy().into_owned(); }
    ffmpeg(&["-f", "lavfi", "-i", &format!("mandelbrot=s=900x600:r=30:end_pts={}", (dur * 30.0) as i64), "-t", &dur.to_string(),
        "-vf", "crop=640:360:x='40+35*sin(2*PI*9*t)':y='40+35*cos(2*PI*12*t)'", "-an", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}

#[test] fn a_clean_clip_scores_high_and_is_kept() {
    let mut ss = proj();
    let a = sid(&call(&mut ss, "import_media", json!({"path": good("clean.mp4", 3.0)})), "asset_id");
    let t = sid(&call(&mut ss, "add_track", json!({})), "track_id");
    let c = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0.0, "duration": 3.0, "in_point": 0.0})), "clip_id");
    let r = call(&mut ss, "score_windows", json!({"clip_ids": [c], "window_s": 1.0}));
    assert!(r["ok"] == json!(true) || r.get("windows").is_some(), "{r}");
    let wins = r["windows"].as_array().unwrap();
    assert!(wins.len() >= 2, "expected several windows: {r}");
    for w in wins { assert!(w["kept"] == json!(true), "clean clip window should be kept: {w}"); assert!(w["score"].as_f64().unwrap() > 0.45, "{w}"); }
}

#[test] fn blurry_dark_and_shaky_clips_are_scored_low_and_dropped() {
    let mut ss = proj();
    let ag = sid(&call(&mut ss, "import_media", json!({"path": good("g2.mp4", 2.0)})), "asset_id");
    let ab = sid(&call(&mut ss, "import_media", json!({"path": blurry("b2.mp4", 2.0)})), "asset_id");
    let ad = sid(&call(&mut ss, "import_media", json!({"path": dark("d2.mp4", 2.0)})), "asset_id");
    let ash = sid(&call(&mut ss, "import_media", json!({"path": shaky("s2.mp4", 2.0)})), "asset_id");
    let t = sid(&call(&mut ss, "add_track", json!({})), "track_id");
    let cg = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": ag, "start": 0.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let cb = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": ab, "start": 2.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let cd = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": ad, "start": 4.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let csh = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": ash, "start": 6.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let r = call(&mut ss, "score_windows", json!({"clip_ids": [cg.clone(), cb.clone(), cd.clone(), csh.clone()], "window_s": 2.0}));
    let wins = r["windows"].as_array().unwrap();
    let of = |cid: &str| -> Value { wins.iter().find(|w| w["clip_id"] == json!(cid)).unwrap().clone() };
    let (wg, wb, wd, wsh) = (of(&cg), of(&cb), of(&cd), of(&csh));
    assert!(wg["kept"] == json!(true), "good clip should be kept: {wg}");
    assert!(wb["score"].as_f64().unwrap() < wg["score"].as_f64().unwrap(), "blurry should score lower than good: {wb} vs {wg}");
    assert!(wb["kept"] == json!(false), "heavily blurred clip should be dropped: {wb}");
    assert!(wd["kept"] == json!(false), "near-black clip should be dropped: {wd}");
    assert!(wd["score_parts"]["exposure"].as_f64().unwrap() < 0.5, "{wd}");
    assert!(wsh["shake_px"].as_f64().unwrap() > wg["shake_px"].as_f64().unwrap(), "shaky clip should measure more shake than the locked-off good clip: {wsh} vs {wg}");
    assert!(wsh["kept"] == json!(false), "shaky clip should be dropped: {wsh}");
}

#[test] fn best_segments_actually_cuts_the_junk_and_keeps_the_rest() {
    let mut ss = proj();
    let ag = sid(&call(&mut ss, "import_media", json!({"path": good("g3.mp4", 2.0)})), "asset_id");
    let ab = sid(&call(&mut ss, "import_media", json!({"path": blurry("b3.mp4", 2.0)})), "asset_id");
    let t = sid(&call(&mut ss, "add_track", json!({})), "track_id");
    let cg = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": ag, "start": 0.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let cb = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": ab, "start": 2.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let r = call(&mut ss, "best_segments", json!({"clip_ids": [cg.clone(), cb.clone()], "window_s": 2.0}));
    assert_eq!(r["dropped_count"].as_u64().unwrap(), 1, "{r}");
    let kept: Vec<String> = r["kept_clips"].as_array().unwrap().iter().map(|c| c["clip_id"].as_str().unwrap().to_string()).collect();
    assert!(kept.contains(&cg), "the good clip should survive: {r}");
    assert!(!kept.contains(&cb), "the blurry clip should be gone: {r}");
    // the dropped clip is actually gone from the project, not just unlisted
    assert!(ss.project.find_clip(&cb).is_none(), "blurry clip should have been deleted from the timeline");
    assert!(ss.project.find_clip(&cg).is_some(), "good clip should still be on the timeline");
}

#[test] fn a_mixed_clip_is_split_so_only_its_good_half_survives() {
    // One clip, first half sharp, second half heavily blurred (built as two concatenated segments) -- best_segments
    // should split it and keep only the sharp half, not drop or keep the whole thing as one piece.
    let mut ss = proj();
    let p = dir().join("mixed.mp4");
    if !p.exists() {
        let half_g = good("mixed_g.mp4", 2.0); let half_b = blurry("mixed_b.mp4", 2.0);
        let list = dir().join("mixed_list.txt");
        std::fs::write(&list, format!("file '{half_g}'\nfile '{half_b}'\n")).unwrap();
        ffmpeg(&["-f", "concat", "-safe", "0", "-i", list.to_str().unwrap(), "-c", "copy", p.to_str().unwrap()]);
    }
    let a = sid(&call(&mut ss, "import_media", json!({"path": p.to_string_lossy()})), "asset_id");
    let t = sid(&call(&mut ss, "add_track", json!({})), "track_id");
    let c = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0.0, "duration": 4.0, "in_point": 0.0})), "clip_id");
    let r = call(&mut ss, "best_segments", json!({"clip_ids": [c.clone()], "window_s": 1.0}));
    let kept = r["kept_clips"].as_array().unwrap();
    assert!(!kept.is_empty(), "the sharp half should survive: {r}");
    let total_kept: f64 = kept.iter().map(|k| k["end"].as_f64().unwrap() - k["start"].as_f64().unwrap()).sum();
    assert!(total_kept < 3.0, "only the sharp half (~2s) should survive, not the whole 4s clip: {r}");
    assert!(total_kept > 0.5, "something real should survive: {r}");
}


/// A synthetic click track with KNOWN beat times (every click_period seconds) so the test can assert cuts land
/// exactly on them -- no reliance on audio_transients' own beat-tracking accuracy on real music.
fn clicks(name: &str, period: f64, n: usize) -> String {
    let p = dir().join(name); if p.exists() { return p.to_string_lossy().into_owned(); }
    let dur = period * n as f64 + 1.0;
    let expr = format!("0.7*exp(-mod(t\\,{period})*40)*sin(2*PI*1200*mod(t\\,{period}))");
    ffmpeg(&["-f", "lavfi", "-i", &format!("aevalsrc='{expr}':s=48000:d={dur}"), "-c:a", "pcm_s16le", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}
/// A click track peaky enough that a single plain-gain normalize would clip (exercises the compressor-retry path),
/// but not so extreme (a gentler decay than `clicks`, more sustain) that one compressor pass can't fix it either.
fn peaky_clicks(name: &str, period: f64, n: usize) -> String {
    let p = dir().join(name); if p.exists() { return p.to_string_lossy().into_owned(); }
    let dur = period * n as f64 + 1.0;
    let expr = format!("0.55*exp(-mod(t\\,{period})*8)*sin(2*PI*800*mod(t\\,{period}))");
    ffmpeg(&["-f", "lavfi", "-i", &format!("aevalsrc='{expr}':s=48000:d={dur}"), "-c:a", "pcm_s16le", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}

#[test] fn beat_sync_montage_cuts_land_exactly_on_real_beats_and_ends_on_a_phrase() {
    let mut ss = proj();
    // 0.5s period = 120bpm, 40 beats (10 bars of 4/4) -- plenty of room for an energy walk plus a phrase-aligned end.
    let music = sid(&call(&mut ss, "import_media", json!({"path": clicks("beat_music.wav", 0.5, 40)})), "asset_id");
    let a = sid(&call(&mut ss, "import_media", json!({"path": good("montage1.mp4", 8.0)})), "asset_id");
    let b = sid(&call(&mut ss, "import_media", json!({"path": good("montage2.mp4", 8.0)})), "asset_id");
    let t = sid(&call(&mut ss, "add_track", json!({})), "track_id");
    let c1 = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0.0, "duration": 8.0, "in_point": 0.0})), "clip_id");
    let c2 = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": b, "start": 8.0, "duration": 8.0, "in_point": 0.0})), "clip_id");
    let r = call(&mut ss, "beat_sync_montage", json!({"clip_ids": [c1, c2], "music_asset_id": music, "beats_per_phrase": 4, "min_beats_per_shot": 1, "max_beats_per_shot": 2, "target_duration": 10.0}));
    let cut_times: Vec<f64> = r["cut_times"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!(cut_times.len() >= 3, "{r}");
    // every cut time must land within 1 frame (1/30s) of an actual 0.5s-period beat
    for ct in &cut_times {
        let nearest_beat = (ct / 0.5).round() * 0.5;
        assert!((ct - nearest_beat).abs() < 1.0 / 30.0 + 1e-6, "cut at {ct} is not within 1 frame of a beat (nearest {nearest_beat}): {r}");
    }
    // the edit must end on a beats_per_phrase (4-beat = 2s) boundary
    let end = r["end"].as_f64().unwrap();
    let nearest_phrase = (end / 2.0).round() * 2.0;
    assert!((end - nearest_phrase).abs() < 1.0 / 30.0 + 1e-6, "end {end} is not on a 4-beat phrase boundary: {r}");
    // the music clip must actually be on the timeline, normalized, and have a fade (two volume keys)
    let music_clip_id = r["music_clip_id"].as_str().unwrap();
    let mc = ss.project.find_clip(music_clip_id).unwrap();
    assert!(mc.audio.volume.keys.len() >= 2, "expected a fade-out (>=2 volume keys), got {:?}", mc.audio.volume.keys);
    assert!(r["fade_out_s"].as_f64().unwrap() > 0.0, "{r}");
}


#[test] fn beat_sync_montage_reaches_loudness_target_even_on_a_peaky_music_bed() {
    // A sharp, peaky click track (like the one above) refuses a plain normalize_loudness gain without pushing
    // sample peaks over -1dBFS -- beat_sync_montage must take the refusal's own suggested fix (a light compressor)
    // and retry, not silently leave the music quiet (gain stuck at 1.0, loudness untouched).
    let mut ss = proj();
    let music = sid(&call(&mut ss, "import_media", json!({"path": peaky_clicks("peaky_music.wav", 0.5, 30)})), "asset_id");
    let a = sid(&call(&mut ss, "import_media", json!({"path": good("montage3.mp4", 8.0)})), "asset_id");
    let t = sid(&call(&mut ss, "add_track", json!({})), "track_id");
    let c1 = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0.0, "duration": 8.0, "in_point": 0.0})), "clip_id");
    let r = call(&mut ss, "beat_sync_montage", json!({"clip_ids": [c1], "music_asset_id": music, "target_lufs": -14.0}));
    let norm = &r["loudness_normalize"];
    let applied = norm["applied"].as_array().unwrap();
    assert!(!applied.is_empty(), "normalize should have succeeded (directly or via the compressor retry), not stayed in failed/skipped: {norm}");
    let gain_db = applied[0]["gain_db"].as_f64().unwrap();
    assert!(gain_db.abs() > 0.5, "a real gain correction should have been applied (not a no-op): {gain_db} dB, {norm}");
    assert!((r["music_gain_applied"].as_f64().unwrap() - 1.0).abs() > 0.05, "music_gain_applied should reflect the real correction, not the 1.0 failure fallback: {r}");
}


/// A warm-tinted clip (boosted red/reduced blue) and a cool-tinted clip (boosted blue/reduced red) from the same
/// source -- two shots that would read as an obvious colour mismatch cut together.
fn warm(name: &str, dur: f64) -> String {
    let p = dir().join(name); if p.exists() { return p.to_string_lossy().into_owned(); }
    ffmpeg(&["-f", "lavfi", "-i", &format!("testsrc2=s=640x360:r=30:d={dur}"), "-vf", "colorchannelmixer=rr=1.0:gg=0.75:bb=0.35", "-an", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}
fn cool(name: &str, dur: f64) -> String {
    let p = dir().join(name); if p.exists() { return p.to_string_lossy().into_owned(); }
    ffmpeg(&["-f", "lavfi", "-i", &format!("testsrc2=s=640x360:r=30:d={dur}"), "-vf", "colorchannelmixer=rr=0.35:gg=0.75:bb=1.0", "-an", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}

#[test] fn montage_color_match_pulls_other_clips_toward_the_first_clips_look() {
    let mut ss = proj();
    let aw = sid(&call(&mut ss, "import_media", json!({"path": warm("warm.mp4", 2.0)})), "asset_id");
    let ac = sid(&call(&mut ss, "import_media", json!({"path": cool("cool.mp4", 2.0)})), "asset_id");
    let t = sid(&call(&mut ss, "add_track", json!({})), "track_id");
    let c1 = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": aw, "start": 0.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let c2 = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": ac, "start": 2.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let r = call(&mut ss, "montage_color_match", json!({"clip_ids": [c1, c2], "strength": 0.8}));
    assert_eq!(r["anchor_clip_ids"], json!([c1]), "{r}");
    assert_eq!(r["matched_clip_ids"], json!([c2]), "{r}");
    let matched = r["match_shot_look_result"]["matched"].as_array().unwrap();
    assert_eq!(matched.len(), 1, "{r}");
    // c2 (cool: low R gain, high B gain) matched toward c1 (warm: high R gain, low B gain) should push its red
    // channel gain UP and its blue channel gain DOWN -- the match is actually pulling the right direction, not
    // just running and reporting "ok".
    let gain = matched[0]["gain"].as_array().unwrap();
    let (r_gain, b_gain) = (gain[0].as_f64().unwrap(), gain[2].as_f64().unwrap());
    assert!(r_gain > 1.05, "expected red gain pushed up to warm the cool clip, got {r_gain}: {r}");
    assert!(b_gain < 0.95, "expected blue gain pushed down to warm the cool clip, got {b_gain}: {r}");
}

#[test] fn montage_color_match_honours_an_explicit_reference_and_rejects_all_clips_as_reference() {
    let mut ss = proj();
    let aw = sid(&call(&mut ss, "import_media", json!({"path": warm("warm2.mp4", 2.0)})), "asset_id");
    let ac = sid(&call(&mut ss, "import_media", json!({"path": cool("cool2.mp4", 2.0)})), "asset_id");
    let t = sid(&call(&mut ss, "add_track", json!({})), "track_id");
    let c1 = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": aw, "start": 0.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    let c2 = sid(&call(&mut ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": ac, "start": 2.0, "duration": 2.0, "in_point": 0.0})), "clip_id");
    // explicit reference = the cool clip this time -- the warm one should be the one that moves.
    let r = call(&mut ss, "montage_color_match", json!({"clip_ids": [c1, c2], "reference_clip_ids": [c2], "strength": 0.8}));
    assert_eq!(r["anchor_clip_ids"], json!([c2]), "{r}");
    assert_eq!(r["matched_clip_ids"], json!([c1]), "{r}");
    // everything also listed as its own reference must error cleanly, not silently no-op.
    let mut ss2 = Session::new();
    call(&mut ss2, "project_set", json!({"width": 640, "height": 360, "fps": 30}));
    let err = dispatch(&mut ss2, "montage_color_match", &json!({"clip_ids": ["x", "y"], "reference_clip_ids": ["x", "y"]}));
    assert!(err.is_err(), "expected an error when every clip_id is also a reference_clip_id (nothing left to match)");
}

// ------------------------------------------------------------------------------------------------------------------
// Fast tiny-media versions (32x24 or 64x48 lossless gray, 10 fps). Textures: CHECK = 2 px checker 40/215 (sharp, fully
// exposed: nothing crushed or clipped), DARK = the same checker at 1/4 (every pixel crushed), FLAT = mid grey 128 (no
// detail at all, nothing crushed or clipped).
// ------------------------------------------------------------------------------------------------------------------
const CHECK: &str = "40+175*mod(floor(X/2)+floor(Y/2),2)";
const DARK: &str = "1+3*mod(floor(X/2)+floor(Y/2),2)";
fn tiny(name: &str, lum: &str, dur: f64, size: &str) -> String {
    let p = dir().join(name);
    ffmpeg(&["-f", "lavfi", "-i", &format!("nullsrc=s={size}:r=10:d={dur},format=gray,geq=lum='{lum}'"), "-c:v", "ffv1", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}
fn tiny_proj(w: u32, h: u32) -> Session { let mut ss = Session::new(); call(&mut ss, "project_set", json!({"width": w, "height": h, "fps": 10})); ss }
/// Lays `media` back to back on one new track; returns the clip ids.
fn lay(ss: &mut Session, media: &[(String, f64)]) -> Vec<String> {
    let t = sid(&call(ss, "add_track", json!({})), "track_id");
    let mut at = 0.0; let mut ids = vec![];
    for (p, d) in media {
        let a = sid(&call(ss, "import_media", json!({"path": p})), "asset_id");
        ids.push(sid(&call(ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": at, "duration": d, "in_point": 0.0})), "clip_id"));
        at += d;
    }
    ids
}

#[test] fn tiny_score_windows_names_the_failing_axis_of_each_window() {
    let mut ss = tiny_proj(32, 24);
    let ids = lay(&mut ss, &[(tiny("t_check.mkv", CHECK, 0.4, "32x24"), 0.4), (tiny("t_dark.mkv", DARK, 0.4, "32x24"), 0.4), (tiny("t_flat.mkv", "128", 0.4, "32x24"), 0.4)]);
    let r = call(&mut ss, "score_windows", json!({"clip_ids": ids, "window_s": 0.2}));
    let w = r["windows"].as_array().unwrap();
    assert_eq!((r["window_count"].as_u64(), r["kept_count"].as_u64(), r["dropped_count"].as_u64()), (Some(6), Some(2), Some(4)), "{r}");
    let spans: Vec<(f64, f64)> = w.iter().map(|x| (x["start"].as_f64().unwrap(), x["end"].as_f64().unwrap())).collect();
    assert_eq!(spans, vec![(0.0, 0.2), (0.2, 0.4), (0.4, 0.6), (0.6, 0.8), (0.8, 1.0), (1.0, 1.2)]);
    for x in &w[0..2] {
        assert_eq!((x["kept"].clone(), x["score_parts"]["sharpness"].as_f64(), x["score_parts"]["exposure"].as_f64(), x["score_parts"]["motion"].as_f64()), (json!(true), Some(1.0), Some(1.0), Some(0.0)), "{x}");
        assert!(x["drop_reason"].is_null());
    }
    for x in &w[2..4] { assert!(x["drop_reason"].as_str().unwrap().starts_with("exposure 0 under min_exposure 0.25"), "{x}"); assert_eq!(x["crushed_frac"].as_f64(), Some(1.0)); }
    for x in &w[4..6] { assert!(x["drop_reason"].as_str().unwrap().starts_with("sharpness 0 under min_sharpness 0.12"), "{x}"); assert_eq!(x["score_parts"]["exposure"].as_f64(), Some(1.0)); }
    // the blended-score floor on its own
    let r2 = call(&mut ss, "score_windows", json!({"clip_ids": [ids[0]], "window_s": 0.2, "min_score": 0.99}));
    for x in r2["windows"].as_array().unwrap() { assert!(x["kept"] == json!(false) && x["drop_reason"].as_str().unwrap().contains("under min_score 0.99"), "{x}"); }
    // a 0.05 s tail under a quarter window is not a window of its own
    let r3 = call(&mut ss, "score_windows", json!({"clip_ids": [ids[0]], "window_s": 0.35}));
    assert_eq!((r3["window_count"].clone(), r3["windows"][0]["end"].clone()), (json!(1), json!(0.35)));
}

#[test] fn tiny_shaky_clip_measures_more_shake_and_is_dropped_by_the_shake_gate() {
    // stabilize needs at least 64x64: 96x72 blocky hash noise, the second clip jittered by up to 4 px per frame
    let mut ss = tiny_proj(96, 72);
    let noise = |dx: &str, dy: &str| format!("255*abs(mod(sin(floor((X+{dx})/2)*12.9898+floor((Y+{dy})/2)*78.233)*43758.5453\\,1))");
    let tex = noise("0", "0");
    let jit = noise("trunc(4*sin(N*2.1))", "trunc(4*cos(N*1.3))");
    let ids = lay(&mut ss, &[(tiny("t_still.mkv", &tex, 1.0, "96x72"), 1.0), (tiny("t_shaky.mkv", &jit, 1.0, "96x72"), 1.0)]);
    let r = call(&mut ss, "score_windows", json!({"clip_ids": ids, "window_s": 1.0}));
    let w = r["windows"].as_array().unwrap();
    let (still, shaky) = (w[0]["shake_px"].as_f64().unwrap(), w[1]["shake_px"].as_f64().unwrap());
    assert!(shaky > still + 0.5, "jittered {shaky} vs still {still}: {r}");
    let gate = (still + shaky) / 2.0;
    let r2 = call(&mut ss, "score_windows", json!({"clip_ids": ids, "window_s": 1.0, "max_shake_px": gate, "min_score": 0.0, "min_sharpness": 0.0}));
    let w2 = r2["windows"].as_array().unwrap();
    assert!(w2[1]["drop_reason"].as_str().unwrap().starts_with("shake "), "{r2}");
    assert_eq!(w2[0]["kept"], json!(true), "{r2}");
}

#[test] fn tiny_best_segments_splits_mixed_clips_and_deletes_all_junk_ones() {
    let mut ss = tiny_proj(32, 24);
    let kdk = tiny("t_kdk.mkv", &format!("if(between(N,2,3),{DARK},{CHECK})"), 0.6, "32x24");
    let kd = tiny("t_kd.mkv", &format!("if(gte(N,2),{DARK},{CHECK})"), 0.4, "32x24");
    let ids = lay(&mut ss, &[(tiny("t_check2.mkv", CHECK, 0.4, "32x24"), 0.4), (kdk, 0.6), (kd, 0.4), (tiny("t_dark2.mkv", DARK, 0.4, "32x24"), 0.4)]);
    let r = call(&mut ss, "best_segments", json!({"clip_ids": ids, "window_s": 0.2}));
    assert_eq!((r["window_count"].as_u64(), r["dropped_count"].as_u64(), r["kept_count"].as_u64()), (Some(9), Some(3), Some(4)), "{r}");
    let kept: Vec<(String, f64, f64)> = r["kept_clips"].as_array().unwrap().iter().map(|k| (k["clip_id"].as_str().unwrap().to_string(), k["start"].as_f64().unwrap(), k["end"].as_f64().unwrap())).collect();
    assert_eq!((kept[0].0.as_str(), kept[0].1, kept[0].2), (ids[0].as_str(), 0.0, 0.4), "an all-good clip stays one piece");
    assert_eq!((kept[1].0.as_str(), kept[1].1, kept[1].2), (ids[1].as_str(), 0.4, 0.6));
    assert_eq!((kept[2].1, kept[2].2), (0.8, 1.0));
    assert_eq!((kept[3].0.as_str(), kept[3].1, kept[3].2), (ids[2].as_str(), 1.0, 1.2));
    let mut on_track: Vec<(f64, f64)> = ss.project.tracks[0].clips.iter().map(|c| ((c.start * 1e6).round() / 1e6, ((c.start + c.duration) * 1e6).round() / 1e6)).collect();
    on_track.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(on_track, vec![(0.0, 0.4), (0.4, 0.6), (0.8, 1.0), (1.0, 1.2)], "the dark middle, the dark tail and the all-dark clip are gone");
    assert!(ss.project.find_clip(&ids[3]).is_none());
}

fn tiny_clicks(name: &str, period: f64, n: usize, decay: f64, amp: &str, rate: u32) -> String {
    let p = dir().join(name);
    let expr = format!("{amp}*exp(-mod(t\\,{period})*{decay})*sin(2*PI*1200*mod(t\\,{period}))");
    ffmpeg(&["-f", "lavfi", "-i", &format!("aevalsrc='{expr}':s={rate}:d={}", period * n as f64 + 0.5), "-c:a", "pcm_s16le", p.to_str().unwrap()]);
    p.to_string_lossy().into_owned()
}

#[test] fn tiny_beat_sync_montage_cuts_on_beats_ends_on_a_phrase_and_fades() {
    let mut ss = tiny_proj(32, 24);
    let music = sid(&call(&mut ss, "import_media", json!({"path": tiny_clicks("t_beats.wav", 0.5, 16, 40.0, "(1-0.4*mod(floor(t/0.5)\\,3))", 16000)})), "asset_id");
    let ids = lay(&mut ss, &[(tiny("t_m1.mkv", CHECK, 4.0, "32x24"), 4.0), (tiny("t_m2.mkv", CHECK, 4.0, "32x24"), 4.0)]);
    let r = call(&mut ss, "beat_sync_montage", json!({"clip_ids": ids, "music_asset_id": music, "beats_per_phrase": 4, "min_beats_per_shot": 1, "max_beats_per_shot": 3, "target_duration": 5.0, "start": 1.0}));
    let cuts: Vec<f64> = r["cut_times"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert_eq!(r["cut_times"], r["beat_times_used"]);
    assert!(cuts.len() >= 3, "{r}");
    let beat0 = cuts[0];
    assert!((beat0 - 1.0).abs() < 1e-9, "the first beat lands on start: {r}");
    for w in cuts.windows(2) { let gap = w[1] - w[0]; assert!(gap > 0.5 - 0.04 && gap < 1.5 + 0.04, "shots are 1..3 beats: {r}"); }
    assert!(cuts.last().unwrap() - beat0 <= 5.0 + 1e-9, "target_duration caps the walk: {r}");
    let end = r["end"].as_f64().unwrap();
    assert!((((end - beat0) / 2.0).round() * 2.0 - (end - beat0)).abs() < 0.04 && end >= *cuts.last().unwrap(), "ends on a 4-beat phrase: {r}");
    let d = r["duration"].as_f64().unwrap();
    assert!((d - (end - 1.0)).abs() < 1e-9);
    let fade = r["fade_out_s"].as_f64().unwrap();
    assert!((fade - 1.0).abs() < 0.04, "2 beats of ~0.5 s: {r}");
    let mc = ss.project.find_clip(r["music_clip_id"].as_str().unwrap()).unwrap();
    let keys: Vec<(f64, f64)> = mc.audio.volume.keys.iter().map(|k| (k.t, k.v)).collect();
    assert_eq!(keys.len(), 2);
    assert!((keys[0].0 - (d - fade)).abs() < 1e-9 && (keys[0].1 - r["music_gain_applied"].as_f64().unwrap()).abs() < 1e-9 && keys[1] == (d, 0.0), "{keys:?}");
}
