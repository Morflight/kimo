use kimo_core::ext::undistort::undistort_frame; use kimo_core::sfm::{norm_obs, Intr};
#[test] fn undistort_inverts_the_solver_lens_model() {
    let (w, h, f, k1, k2) = (320usize, 180usize, 300.0, -0.18, 0.05); let ki = Intr { f, cx: w as f64 / 2.0, cy: h as f64 / 2.0, k1, k2, p1: 0.0, p2: 0.0 };
    let ideal = |x: f64, y: f64| -> f32 { (0.5 + 0.25 * (x * 0.09).sin() + 0.25 * (y * 0.07 + x * 0.03).cos()) as f32 };
    // distorted image: pixel p shows the ideal scene at the undistorted position of p
    let mut dist = vec![0f32; w * h * 4]; for y in 0..h { for x in 0..w { let (ux, uy) = { let n = norm_obs((x as f64 + 0.5, y as f64 + 0.5), &ki); (n.0 * f + ki.cx, n.1 * f + ki.cy) }; let v = ideal(ux, uy); for c in 0..3 { dist[(y * w + x) * 4 + c] = v; } dist[(y * w + x) * 4 + 3] = 1.0; } }
    let out = undistort_frame(&dist, w, h, f, ki.cx, ki.cy, k1, k2); let (mut e, mut n, mut cov) = (0.0f64, 0usize, 0usize);
    for y in 0..h { for x in 0..w { let i = (y * w + x) * 4; if out[i + 3] > 0.99 { cov += 1; if x > 4 && y > 4 && x < w - 5 && y < h - 5 { e += (out[i] - ideal(x as f64 + 0.5, y as f64 + 0.5)).abs() as f64; n += 1; } } } }
    let mae = e / n.max(1) as f64; eprintln!("undistort round trip: mean abs error {mae:.5} over {n} px, coverage {:.1}%", cov as f64 / (w * h) as f64 * 100.0); assert!(mae < 0.01 && cov > w * h * 8 / 10);
}

// ---------- undistort_clip on a tiny lossless plate (32x24, 10 fps, 3 frames of a grey ramp texture) ----------
use kimo_core::commands::{dispatch, Session};
use serde_json::json;
fn decode_gray(p: &str) -> Vec<u8> { let o = std::process::Command::new("ffmpeg").args(["-v", "error", "-i", p, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]).output().unwrap(); assert!(o.status.success()); o.stdout.chunks(3).map(|c| c[1]).collect() }
#[test] fn undistort_clip_encodes_the_remapped_frames() {
    let src = std::env::temp_dir().join(format!("kimo-undist-src-{}.mkv", std::process::id())); let t = "60+5*X+2*Y";
    assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", &format!("nullsrc=s=32x24:r=10:d=0.3,format=gray,geq=lum='{t}'"), "-c:v", "ffv1", src.to_str().unwrap()]).status().unwrap().success());
    let mut s = Session::new(); dispatch(&mut s, "project_set", &json!({"width": 32, "height": 24, "fps": 10})).unwrap();
    let a = dispatch(&mut s, "import_media", &json!({"path": src})).unwrap()["asset_id"].clone(); let tr = dispatch(&mut s, "add_track", &json!({})).unwrap()["track_id"].clone();
    let clip = dispatch(&mut s, "add_clip", &json!({"track_id": tr, "source_type": "media", "asset_id": a, "start": 0, "duration": 0.3})).unwrap()["clip_id"].as_str().unwrap().to_string();
    let out = std::env::temp_dir().join(format!("kimo-undist-out-{}.mp4", std::process::id())); let os = out.to_str().unwrap();
    // k1 = 0: the identity remap; the encoded frames are the source (within H.264 at crf 14)
    let r = dispatch(&mut s, "undistort_clip", &json!({"clip_id": clip, "k1": 0, "focal_px": 40, "out_path": os})).unwrap();
    assert_eq!(r["frames"], 3); assert_eq!(r["lens"]["from"], "explicit"); let aid = r["asset_id"].as_str().unwrap(); assert_eq!(s.project.assets[aid].width, 32);
    let g = decode_gray(os); assert_eq!(g.len(), 32 * 24 * 3); let want = |x: usize, y: usize| 60.0 + 5.0 * x as f64 + 2.0 * y as f64;
    let mae = (0..32 * 24).map(|i| (g[i] as f64 - want(i % 32, i / 32)).abs()).sum::<f64>() / (32.0 * 24.0); assert!(mae < 4.0, "identity remap mae {mae}");
    // pincushion removal (k1 > 0) pulls the corners from outside the source: they go black, the centre stays
    let r = dispatch(&mut s, "undistort_clip", &json!({"clip_id": clip, "k1": 0.5, "focal_px": 20, "out_path": os, "import": false, "start": 1, "end": 1})).unwrap();
    assert_eq!(r["frames"], 1); assert!(r.get("asset_id").is_none()); let g = decode_gray(os); assert!(g[0] < 20, "corner {}", g[0]); assert!((g[12 * 32 + 16] as f64 - want(16, 12)).abs() < 8.0, "centre {}", g[12 * 32 + 16]);
    // the lens of a stored camera solve
    let sol = kimo_core::sfm::Solution { mode: "full", k1k2: [0.0; 2], poses: vec![None], intr: Intr { f: 40.0, cx: 16.0, cy: 12.0, k1: 0.0, k2: 0.0, p1: 0.0, p2: 0.0 }, points: vec![], point_track: vec![], rms: 0.0, frame_rms: vec![f64::NAN], registered: 0, notes: vec![] };
    let sid = kimo_core::ext::camera_solve::store_solution(&clip, 0, 10.0, 0.0, 32, 24, sol, 0, false);
    let r = dispatch(&mut s, "undistort_clip", &json!({"clip_id": clip, "out_path": os, "import": false})).unwrap(); assert_eq!(r["lens"]["from"], sid.as_str()); assert_eq!(r["lens"]["focal_px"], 40.0);
    std::fs::remove_file(&out).unwrap(); std::fs::remove_file(&src).unwrap();
}
