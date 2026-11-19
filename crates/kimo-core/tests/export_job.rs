//! RV-6: chunked export job: incremental reuse, status, cancel, resume, join. Real ffmpeg, tiny 160x90 25 fps projects.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};
fn c(ss: &mut Session, n: &str, v: Value) -> Value { dispatch(ss, n, &v).unwrap_or_else(|e| panic!("{n}: {e}")) }
fn tmp(tag: &str) -> std::path::PathBuf { let d = std::env::temp_dir().join(format!("kimo_rv6_{tag}_{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
/// three 2 s shape clips back to back = 6 s, 3 chunks of 2 s
fn scene(n: usize) -> (Session, Vec<String>) {
    let mut ss = Session::new(); c(&mut ss, "project_set", json!({"width": 160, "height": 90, "fps": 25})); let t = c(&mut ss, "add_track", json!({}))["track_id"].as_str().unwrap().to_string();
    let ids = (0..n).map(|i| c(&mut ss, "add_shape", json!({"track_id": t, "shape": "rect", "width": 20 + i * 3, "height": 20, "start": i as f64 * 2.0, "duration": 2.0}))["clip_id"].as_str().unwrap().to_string()).collect(); (ss, ids)
}
fn run(ss: &mut Session, d: &std::path::Path, extra: Value) -> Value { let mut a = json!({"path": d.join("out.mp4").to_str().unwrap(), "chunk_s": 2, "wait": true}); for (k, v) in extra.as_object().unwrap() { a[k] = v.clone(); } c(ss, "export_job_start", a)["status"].clone() }
fn frames(p: &std::path::Path) -> u64 { let o = std::process::Command::new("ffprobe").args(["-v", "error", "-count_frames", "-select_streams", "v:0", "-show_entries", "stream=nb_read_frames", "-of", "csv=p=0"]).arg(p).output().unwrap(); String::from_utf8_lossy(&o.stdout).trim().parse().unwrap_or(0) }
fn chunk(d: &std::path::Path, i: u64) -> std::path::PathBuf { d.join("out.mp4.chunks").join(format!("chunk_{i:05}.mp4")) }

#[test] fn first_run_renders_every_chunk_and_the_join_has_the_right_frame_count() {
    let d = tmp("first"); let (mut ss, _) = scene(3); let s = run(&mut ss, &d, json!({}));
    assert_eq!(s["state"], "done", "{s}"); assert_eq!(s["chunks"], json!({"done": 3, "total": 3, "reused": 0})); assert_eq!(s["frames_done"], 150); assert_eq!(s["frames_total"], 150);
    assert_eq!(s["result"]["rendered"].as_array().unwrap().len(), 3); assert_eq!(s["result"]["rendered"][0]["reason"], "no_previous"); assert_eq!(frames(&d.join("out.mp4")), 150);
    assert!(d.join("out.mp4.chunks/manifest.json").is_file() && d.join("out.mp4.chunks/index.json").is_file());
    // the joined picture equals an ffmpeg concat of the same chunks (frame count)
    let list = d.join("l.txt"); std::fs::write(&list, (0..3).map(|i| format!("file '{}'\n", chunk(&d, i).display())).collect::<String>()).unwrap();
    let cat = d.join("cat.mp4"); assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "concat", "-safe", "0", "-i"]).arg(&list).args(["-c", "copy", "-an"]).arg(&cat).status().unwrap().success());
    assert_eq!(frames(&cat), frames(&d.join("out.mp4"))); std::fs::remove_dir_all(&d).ok();
}
#[test] fn unchanged_rerun_reuses_everything_and_an_edit_renders_only_its_chunk() {
    let d = tmp("incr"); let (mut ss, ids) = scene(3); run(&mut ss, &d, json!({})); let before: Vec<Vec<u8>> = (0..3).map(|i| std::fs::read(chunk(&d, i)).unwrap()).collect();
    let s = run(&mut ss, &d, json!({})); assert_eq!(s["chunks"]["reused"], 3); assert_eq!(s["result"]["rendered"].as_array().unwrap().len(), 0);
    c(&mut ss, "set_keyframes", json!({"clip_id": ids[1], "property": "opacity", "keys": [{"t": 0.0, "v": 1.0}, {"t": 1.0, "v": 0.4}]}));
    let s = run(&mut ss, &d, json!({})); assert_eq!(s["chunks"]["reused"], 2, "{s}"); assert_eq!(s["result"]["rendered"], json!([{"index": 1, "reason": "key_changed"}]));
    assert_eq!(before[0], std::fs::read(chunk(&d, 0)).unwrap(), "a reused chunk is untouched"); assert_eq!(before[2], std::fs::read(chunk(&d, 2)).unwrap()); assert_ne!(before[1], std::fs::read(chunk(&d, 1)).unwrap());
    // forced: everything re-renders and says so; reused chunk bytes equal a fresh full re-render (deterministic encode)
    let s = run(&mut ss, &d, json!({"force": true})); assert_eq!(s["chunks"]["reused"], 0); assert_eq!(s["result"]["rendered"][0]["reason"], "forced");
    assert_eq!(before[0], std::fs::read(chunk(&d, 0)).unwrap(), "re-rendering an unchanged chunk gives the same bytes"); std::fs::remove_dir_all(&d).ok();
}
#[test] fn a_corrupted_or_missing_chunk_file_is_never_reused() {
    let d = tmp("hash"); let (mut ss, _) = scene(3); run(&mut ss, &d, json!({}));
    let f = chunk(&d, 0); let mut b = std::fs::read(&f).unwrap(); let n = b.len() - 3; b[n] ^= 0xff; std::fs::write(&f, b).unwrap(); std::fs::remove_file(chunk(&d, 2)).unwrap();
    let s = run(&mut ss, &d, json!({})); let r = s["result"]["rendered"].as_array().unwrap().clone(); assert_eq!(r.len(), 2, "{s}");
    assert!(r.contains(&json!({"index": 0, "reason": "file_hash_mismatch"})) && r.contains(&json!({"index": 2, "reason": "file_missing"}))); assert_eq!(frames(&d.join("out.mp4")), 150); std::fs::remove_dir_all(&d).ok();
}
#[test] fn cancel_keeps_finished_chunks_marks_the_unfinished_one_and_resume_finishes() {
    let d = tmp("cancel"); let (mut ss, _) = scene(3);
    // render chunk 0 only by cancelling right away after it lands: start without wait and cancel when one chunk is done
    let a = json!({"path": d.join("out.mp4").to_str().unwrap(), "chunk_s": 2}); let id = c(&mut ss, "export_job_start", a)["job_id"].as_str().unwrap().to_string();
    let t0 = std::time::Instant::now(); loop { let s = c(&mut ss, "job_status", json!({"job_id": id})); if s["chunks"]["done"].as_u64().unwrap() >= 1 || s["state"] == "done" || t0.elapsed().as_secs() > 60 { break; } std::thread::sleep(std::time::Duration::from_millis(5)); }
    let s = c(&mut ss, "cancel_job", json!({"job_id": id}));
    if s["state"] == "cancelled" { assert!(!s["completed_ranges"].as_array().unwrap().is_empty()); assert!(s["chunks"]["done"].as_u64().unwrap() < 3);
        let r = c(&mut ss, "resume_job", json!({"path": d.join("out.mp4").to_str().unwrap(), "chunk_s": 2})); let id2 = r["job_id"].as_str().unwrap().to_string();
        let t0 = std::time::Instant::now(); let fin = loop { let s = c(&mut ss, "job_status", json!({"job_id": id2})); if s["state"] != "running" && s["state"] != "queued" || t0.elapsed().as_secs() > 120 { break s; } std::thread::sleep(std::time::Duration::from_millis(50)); };
        assert_eq!(fin["state"], "done", "{fin}"); assert!(fin["chunks"]["reused"].as_u64().unwrap() >= 1, "finished chunks are kept: {fin}"); assert_eq!(frames(&d.join("out.mp4")), 150);
    } else { assert_eq!(s["state"], "done", "the job finished before the cancel landed: {s}"); }
    std::fs::remove_dir_all(&d).ok();
}
#[test] fn resume_refuses_when_the_project_changed_and_names_the_dependency() {
    let d = tmp("mismatch"); let (mut ss, ids) = scene(3); run(&mut ss, &d, json!({}));
    c(&mut ss, "set_keyframes", json!({"clip_id": ids[2], "property": "opacity", "keys": [{"t": 0.0, "v": 1.0}, {"t": 1.0, "v": 0.2}]}));
    let e = dispatch(&mut ss, "resume_job", &json!({"path": d.join("out.mp4").to_str().unwrap(), "chunk_s": 2})).unwrap_err(); assert!(e.contains("resume_mismatch") && e.contains(&ids[2]) && e.contains("clip_changed"), "{e}");
    let e = dispatch(&mut ss, "resume_job", &json!({"path": d.join("nothing.mp4").to_str().unwrap()})).unwrap_err(); assert!(e.contains("resume_mismatch"), "{e}"); std::fs::remove_dir_all(&d).ok();
}
#[test] fn eta_is_null_until_three_chunks_are_measured_and_reports_uncertainty() {
    use kimo_core::ext::export_job::{eta, MIN_MEASURED}; assert_eq!(MIN_MEASURED, 3);
    assert!(eta(&[1.0, 1.0], 5).is_null()); let e = eta(&[1.0, 1.0, 1.0], 4); assert_eq!(e["seconds"], 4.0); assert_eq!(e["uncertainty"], "low");
    assert_eq!(eta(&[0.2, 1.0, 3.0], 2)["uncertainty"], "high");
}
#[test] fn bad_arguments_are_named_errors() {
    let (mut ss, _) = scene(1); assert!(dispatch(&mut ss, "job_status", &json!({"job_id": "nope"})).unwrap_err().contains("no such job"));
    assert!(dispatch(&mut ss, "export_job_start", &json!({"path": "/tmp/x.mp4", "crf": 99})).unwrap_err().contains("crf"));
    assert!(dispatch(&mut ss, "export_job_start", &json!({"path": "/tmp/x.mp4", "chunk_s": 0})).unwrap_err().contains("chunk_s"));
}

/// Deterministic cancel: wait until the SECOND chunk is being rendered (its .partial.mp4 exists), then cancel. Retries because a chunk can finish before the cancel lands.
#[test] fn cancel_mid_chunk_leaves_a_marked_partial_and_resume_reuses_the_finished_chunks() {
    for attempt in 0..4 {
        let d = tmp(&format!("midcancel{attempt}")); let (mut ss, _) = scene(3); c(&mut ss, "project_set", json!({"width": 480, "height": 270, "fps": 25}));
        let out = d.join("out.mp4"); let dir = d.join("out.mp4.chunks"); let id = c(&mut ss, "export_job_start", json!({"path": out.to_str().unwrap(), "chunk_s": 2}))["job_id"].as_str().unwrap().to_string();
        let t0 = std::time::Instant::now(); while !dir.join("chunk_00001.partial.mp4").exists() && t0.elapsed().as_secs() < 60 { std::thread::sleep(std::time::Duration::from_millis(1)); }
        let s = c(&mut ss, "cancel_job", json!({"job_id": id}));
        if s["state"] != "cancelled" { std::fs::remove_dir_all(&d).ok(); continue; }
        assert!(dir.join("chunk_00000.mp4").is_file(), "the finished chunk stays"); assert_eq!(s["completed_ranges"], json!([[0, 50]]), "{s}");
        assert!(dir.join("chunk_00001.mp4.partial").is_file() && dir.join("chunk_00001.mp4.partial.marker").is_file(), "unfinished chunk is marked: {:?}", std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect::<Vec<_>>());
        assert!(!dir.join("chunk_00001.mp4").exists() && !dir.join("chunk_00001.partial.mp4").exists(), "never a finished-looking file for an unfinished chunk");
        let r = c(&mut ss, "resume_job", json!({"path": out.to_str().unwrap(), "chunk_s": 2})); let id2 = r["job_id"].as_str().unwrap().to_string();
        let t0 = std::time::Instant::now(); let fin = loop { let s = c(&mut ss, "job_status", json!({"job_id": id2})); if s["state"] != "running" && s["state"] != "queued" || t0.elapsed().as_secs() > 120 { break s; } std::thread::sleep(std::time::Duration::from_millis(20)); };
        assert_eq!(fin["state"], "done", "{fin}"); assert_eq!(fin["chunks"]["reused"], 1, "{fin}"); assert_eq!(frames(&out), 150); std::fs::remove_dir_all(&d).ok(); return;
    }
    panic!("the cancel never landed mid-chunk in 4 attempts");
}

// ---- stream-copy fast path (export_fast), on 32x24 10 fps media ----
fn ff(args: &[&str]) { assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y"]).args(args).status().unwrap().success(), "{args:?}"); }
/// 1 s of h264 (32x24, 10 fps, solid colour) with an aac tone, and a 1 s wav tone.
fn fast_media(d: &std::path::Path) -> (String, String) {
    let v = d.join("v.mp4"); let w = d.join("a.wav");
    ff(&["-f", "lavfi", "-i", "color=c=0x336699:s=32x24:r=10:d=1", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000:duration=1", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", v.to_str().unwrap()]);
    ff(&["-f", "lavfi", "-i", "sine=frequency=220:sample_rate=48000:duration=1", w.to_str().unwrap()]);
    (v.to_string_lossy().into_owned(), w.to_string_lossy().into_owned())
}
fn media_proj(w: u32, h: u32, fps: f64) -> Session { let mut ss = Session::new(); c(&mut ss, "project_set", json!({"width": w, "height": h, "fps": fps})); ss }
fn put(ss: &mut Session, path: &str, start: f64, dur: f64) -> (String, String) {
    let a = c(ss, "import_media", json!({"path": path}))["asset_id"].as_str().unwrap().to_string();
    let t = c(ss, "add_track", json!({}))["track_id"].as_str().unwrap().to_string();
    let id = c(ss, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": start, "duration": dur}))["clip_id"].as_str().unwrap().to_string();
    (t, id)
}

#[test] fn one_untouched_video_plus_audio_is_stream_copied_and_verified() {
    let d = tmp("fastpath"); let (v, w) = fast_media(&d);
    let mut ss = media_proj(32, 24, 10.0);
    put(&mut ss, &v, 0.0, 1.0);
    let (_, wid) = put(&mut ss, &w, 0.5, 0.5);
    c(&mut ss, "set_audio", json!({"clip_id": wid, "fade_in": 0.1, "fade_out": 0.1}));
    let out = d.join("fast.mp4");
    let r = c(&mut ss, "export_job_start", json!({"path": out.to_str().unwrap(), "wait": true}));
    assert_eq!(r["path_used"]["mode"], "stream_copy", "{r}");
    assert_eq!(r["path_used"]["audio_clips"], 2, "the video's own audio plus the wav: {r}");
    let s = &r["status"];
    assert_eq!(s["state"], "done", "{s}");
    assert_eq!((s["frames_done"].clone(), s["frames_total"].clone(), s["chunks"]["done"].clone()), (json!(10), json!(10), json!(1)), "{s}");
    assert_eq!(s["result"]["duration"], 1.0);
    assert_eq!(frames(&out), 10, "every source frame copied");
    assert!(!d.join("fast.mp4.partial.mp4").exists());
    // fast:false forces the chunked render of the same project
    let r2 = c(&mut ss, "export_job_start", json!({"path": d.join("slow.mp4").to_str().unwrap(), "wait": true, "fast": false}));
    assert_eq!((r2["path_used"]["mode"].clone(), r2["path_used"]["why_not_stream_copy"].clone(), r2["status"]["state"].clone()), (json!("render"), json!("off"), json!("done")), "{r2}");
    std::fs::remove_dir_all(&d).ok();
}
