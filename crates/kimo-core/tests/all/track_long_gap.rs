//! track_long_gap (long_gap.rs) through its command, on a tiny lossless clip: 64x48, 10 fps, 9 frames. A 16x16
//! checkerboard (4 px cells, 0/255 on a flat 40 background) sits at x = 8 + 2N, y = 16 on frame N, except frames 4 and 5,
//! which are entirely flat (the object is occluded). Every expected state and box below follows from that by hand.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};

fn clip_media(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("kimo-longgap-{tag}-{}.mkv", std::process::id()));
    let g = "nullsrc=s=64x48:r=10:d=0.9,format=gray,geq=lum='if(between(N,4,5),40,if(between(X,8+2*N,23+2*N)*between(Y,16,31),255*mod(floor((X-8-2*N)/4)+floor((Y-16)/4),2),40))'";
    assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", g, "-c:v", "ffv1", p.to_str().unwrap()]).status().unwrap().success());
    p
}
fn session(p: &std::path::Path) -> (Session, String) {
    let mut s = Session::new();
    dispatch(&mut s, "project_set", &json!({"width": 64, "height": 48, "fps": 10})).unwrap();
    let a = dispatch(&mut s, "import_media", &json!({"path": p})).unwrap()["asset_id"].clone();
    let tr = dispatch(&mut s, "add_track", &json!({})).unwrap()["track_id"].clone();
    let id = dispatch(&mut s, "add_clip", &json!({"track_id": tr, "source_type": "media", "asset_id": a, "start": 0, "duration": 0.9})).unwrap()["clip_id"].as_str().unwrap().to_string();
    (s, id)
}
const SEED: [f64; 4] = [0.125, 16.0 / 48.0, 0.25, 16.0 / 48.0];

#[test]
fn tracks_through_an_occlusion_and_reacquires_after_two_consistent_frames() {
    let p = clip_media("t1");
    let (mut s, id) = session(&p);
    let before = serde_json::to_value(&s.project).unwrap();
    for memory in [false, true] {
        let r = dispatch(&mut s, "track_long_gap", &json!({"clip_id": id, "bbox": SEED, "appearance_memory": memory})).unwrap();
        let samples = r["samples"].as_array().unwrap();
        let states: Vec<&str> = samples.iter().map(|x| x["state"].as_str().unwrap()).collect();
        assert_eq!(states, ["seed", "visible", "visible", "visible", "lost", "lost", "tentative", "reacquired", "visible"], "memory={memory}");
        for (i, smp) in samples.iter().enumerate() {
            assert_eq!(smp["frame"], json!(i));
            match states[i] {
                "lost" | "tentative" => assert!(smp["bbox"].is_null()),
                _ => {
                    let b = &smp["bbox"];
                    assert!((b["x"].as_f64().unwrap() - (8 + 2 * i) as f64).abs() < 1e-6 && (b["y"].as_f64().unwrap() - 16.0).abs() < 1e-6, "frame {i}: {b}");
                    assert!((b["w"].as_f64().unwrap() - 16.0).abs() < 1e-6 && (b["h"].as_f64().unwrap() - 16.0).abs() < 1e-6, "frame {i}: {b}");
                }
            }
        }
        assert_eq!((r["analysis_size"].clone(), r["appearance_memory"].clone(), r["mutated"].clone()), (json!([64, 48]), json!(memory), json!(false)));
        // three stable identical observations: the appearance is not novel enough to learn
        assert_eq!(r["learned_anchors"], json!(0));
    }
    // a sub-range starting mid-clip seeds at that frame
    let r = dispatch(&mut s, "track_long_gap", &json!({"clip_id": id, "bbox": [10.0 / 64.0, SEED[1], 0.25, SEED[3]], "start_frame": 1, "end_frame": 2})).unwrap();
    let st: Vec<(Value, Value)> = r["samples"].as_array().unwrap().iter().map(|x| (x["frame"].clone(), x["state"].clone())).collect();
    assert_eq!(st, vec![(json!(1), json!("seed")), (json!(2), json!("visible"))]);
    // a 34 px tall seed (the 1.5 scale would not fit the 48 px frame and is skipped) still follows the patch exactly
    let r = dispatch(&mut s, "track_long_gap", &json!({"clip_id": id, "bbox": [0.125, 14.0 / 48.0, 0.25, 34.0 / 48.0], "end_frame": 1})).unwrap();
    let b = &r["samples"][1]["bbox"];
    assert_eq!((r["samples"][1]["state"].clone(), b["x"].as_f64().map(|x| x.round()), b["h"].as_f64().map(|x| x.round())), (json!("visible"), Some(10.0), Some(34.0)), "{r}");
    assert_eq!(before, serde_json::to_value(&s.project).unwrap(), "read-only");
    std::fs::remove_file(p).unwrap();
}
