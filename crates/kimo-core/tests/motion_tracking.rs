//! Classic block-matching tracker and stabilizer (tracking.rs) on tiny lossless synthetic plates: ffv1, 10 fps, a smooth aperiodic
//! texture translating exactly +1 px in x per frame (frame k at pixel (x, y) shows T(x - k, y)), so the tracked point moves (k, 0)
//! and the stabilizer's locked correction is (-k, 0) at frame k. Also the motion critic on keyframed solids.
use kimo_core::commands::{dispatch, Session};
use kimo_core::tracking;
use serde_json::{json, Value};

const TEX: &str = "128+45*sin(0.55*(X-N))*sin(0.47*Y+0.3)+30*sin(0.21*(X-N)+0.33*Y)+20*cos(0.13*(X-N)-0.29*Y+1)";
fn plate(w: u32, h: u32) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("kimo-motiontrack-{w}x{h}-{}-{}.mkv", TEX.len(), std::process::id()));
    let g = format!("nullsrc=s={w}x{h}:r=10:d=0.5,format=rgb24,geq=r='{TEX}':g='{TEX}':b='{TEX}'");
    assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", &g, "-c:v", "ffv1", "-pix_fmt", "bgr0", p.to_str().unwrap()]).status().unwrap().success()); p
}
fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n} {a}: {e}")) }
fn setup(w: u32, h: u32) -> (Session, String, std::path::PathBuf) {
    let p = plate(w, h); let mut s = Session::new(); c(&mut s, "project_set", json!({"width": w, "height": h, "fps": 10}));
    let a = c(&mut s, "import_media", json!({"path": p}))["asset_id"].clone(); let t = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let id = c(&mut s, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0, "duration": 0.5}))["clip_id"].as_str().unwrap().to_string(); (s, id, p)
}
fn near(a: f64, b: f64, tol: f64, what: &str) { assert!((a - b).abs() <= tol, "{what}: {a} vs {b} (tol {tol})"); }

#[test]
fn track_point_follows_the_translation_with_the_patch_ring() {
    let (s, id, p) = setup(80, 60);
    let ring = tracking::track_point(&s.project, &id, 0.5, 0.5, 12, 4, true, 0.12).unwrap();
    assert_eq!(ring.len(), 5); for (k, t) in ring.iter().enumerate() { near(t.t, k as f64 / 10.0, 1e-12, "t"); near(t.x, 40.0 + k as f64, 0.25, &format!("ring x at {k}")); near(t.y, 30.0, 0.25, "ring y"); near(t.scale, 1.0, 0.02, "ring scale"); near(t.rot_deg, 0.0, 1.0, "ring rotation"); }
    std::fs::remove_file(p).unwrap();
}

#[test]
fn classic_stabilize_locks_the_known_camera_path() {
    let (s, id, p) = setup(160, 120);
    let r = tracking::stabilize(&s.project, &id, 4.0, true, true).unwrap(); assert_eq!(r.len(), 5);
    for (k, v) in r.iter().enumerate() { near(v.t, k as f64 / 10.0, 1e-12, "t"); near(v.dx, -(k as f64), 0.25, &format!("locked dx at {k}")); near(v.dy, 0.0, 0.25, "dy"); near(v.rot_deg, 0.0, 0.2, "rot");
        // the zoom must cover the canvas after shifting by |dx| px: at least (80 + |dx|) / 80
        assert!(v.scale >= (80.0 + k as f64 - 0.25) / 80.0 * 1.003 - 1e-9, "frame {k}: zoom {} too small", v.scale); }
    // rotation off: the estimated rotation and scale are discarded
    let nr = tracking::stabilize(&s.project, &id, 4.0, true, false).unwrap(); for v in &nr { assert_eq!(v.rot_deg, 0.0); }
    // smoothing a straight pan keeps the middle of the path (the correction is small there) while lock removes all motion
    let sm = tracking::stabilize(&s.project, &id, 0.2, false, true).unwrap(); for v in &sm { near(v.dx, 0.0, 1e-9, "sigma < 0.3 keeps the path: no correction"); }
    std::fs::remove_file(p).unwrap();
}

// ---------- motion critic (critic/motion.rs): pops, off-screen excursions, sub-frame and small moves ----------
#[test]
fn motion_critic_flags_pops_and_offscreen_excursions_only() {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 200, "height": 100, "fps": 10})); let t = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let solid = |s: &mut Session, start: f64| c(s, "add_clip", json!({"track_id": t, "source_type": "solid", "color": [1, 1, 1], "start": start, "duration": 3}))["clip_id"].as_str().unwrap().to_string();
    let key = |s: &mut Session, id: &str, p: &str, tt: f64, v: f64| { c(s, "set_keyframe", json!({"clip_id": id, "property": p, "time": tt, "value": v, "ease": "hold"})); };
    let keyl = |s: &mut Session, id: &str, p: &str, tt: f64, v: f64| { c(s, "set_keyframe", json!({"clip_id": id, "property": p, "time": tt, "value": v, "ease": "in_out"})); };
    // x holds at 0 for a second, then jumps 60 px in a single frame: a pop
    let a = solid(&mut s, 0.0); key(&mut s, &a, "x", 0.0, 0.0); key(&mut s, &a, "x", 1.05, 60.0); key(&mut s, &a, "x", 2.0, 60.0);
    // y eases from 0 out past the 50 px half height and back to 0: it leaves the frame mid-move
    let b = solid(&mut s, 3.0); keyl(&mut s, &b, "y", 0.0, 0.0); keyl(&mut s, &b, "y", 1.0, 120.0); keyl(&mut s, &b, "y", 2.0, 0.0);
    // too short (under 2 frames), too small (rotation span under 1 deg) or a single key: nothing to say
    let q = solid(&mut s, 6.0); keyl(&mut s, &q, "x", 0.0, 0.0); keyl(&mut s, &q, "x", 0.1, 50.0); keyl(&mut s, &q, "rotation", 0.0, 0.0); keyl(&mut s, &q, "rotation", 2.0, 0.6); keyl(&mut s, &q, "scale", 0.5, 2.0);
    // a move that ends off-screen is not an excursion; a smooth zoom is fine
    let e = solid(&mut s, 9.0); keyl(&mut s, &e, "x", 0.0, 0.0); keyl(&mut s, &e, "x", 2.0, 300.0); keyl(&mut s, &e, "scale", 0.0, 1.0); keyl(&mut s, &e, "scale", 2.0, 1.5);
    let f = kimo_core::critic::timeline(&s.project); let mine = |id: &str| f.iter().filter(|x| x.clip_id.as_deref() == Some(id) && x.domain == "motion").map(|x| x.code).collect::<Vec<_>>();
    assert_eq!(mine(&a), vec!["motion_pop"]); let pop = f.iter().find(|x| x.code == "motion_pop").unwrap(); assert!((pop.at.unwrap() - 1.0).abs() < 0.11, "pop at {:?}", pop.at);
    assert_eq!(mine(&b), vec!["leaves_frame_mid_move"]); assert!(mine(&q).is_empty(), "{:?}", mine(&q)); assert!(mine(&e).is_empty(), "{:?}", mine(&e));
}
