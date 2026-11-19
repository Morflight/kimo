//! Planar tracker, corner pin, mesh pin and occluder mattes (planar.rs) on tiny lossless synthetic plates: ffv1, 96x80, 10 fps,
//! 6 frames of a smooth aperiodic texture translating exactly +1 px in x per frame, so every tracked quad is the frame-0 quad
//! shifted by (k, 0). The occluder plate adds a black 12x12 square at x 44..55, y 34..45 from frame 3 on.
use kimo_core::commands::{dispatch, Session};
use kimo_core::planar;
use kimo_core::render;
use serde_json::{json, Value};

const TEX: &str = "128+45*sin(0.55*(X-N))*sin(0.47*Y+0.3)+30*sin(0.21*(X-N)+0.33*Y)+20*cos(0.13*(X-N)-0.29*Y+1)";
fn plate(tag: &str, occluder: bool) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("kimo-planar-{tag}-{}.mkv", std::process::id()));
    let t = if occluder { format!("if(gte(N,3)*between(X,44,55)*between(Y,34,45),0,{TEX})") } else { TEX.to_string() };
    let g = format!("nullsrc=s=96x80:r=10:d=0.6,format=rgb24,geq=r='{t}':g='{t}':b='{t}'");
    assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", &g, "-c:v", "ffv1", "-pix_fmt", "bgr0", p.to_str().unwrap()]).status().unwrap().success()); p
}
fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n} {a}: {e}")) }
/// Media clip on track 1, red solid target on track 2 (drawn over it).
fn setup(p: &std::path::Path) -> (Session, String, String) {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 96, "height": 80, "fps": 10}));
    let a = c(&mut s, "import_media", json!({"path": p}))["asset_id"].clone(); let t1 = c(&mut s, "add_track", json!({}))["track_id"].clone(); let t2 = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let m = c(&mut s, "add_clip", json!({"track_id": t1, "source_type": "media", "asset_id": a, "start": 0, "duration": 0.6}))["clip_id"].as_str().unwrap().to_string();
    let r = c(&mut s, "add_clip", json!({"track_id": t2, "source_type": "solid", "color": [1, 0, 0], "start": 0, "duration": 0.6}))["clip_id"].as_str().unwrap().to_string();
    (s, m, r)
}
const CORNERS: [f64; 8] = [0.25, 0.25, 0.75, 0.25, 0.75, 0.75, 0.25, 0.75]; // (24,20) (72,20) (72,60) (24,60)
fn frame(s: &Session, t: f64) -> Vec<f32> { let mut cc = render::Cache::new(); render::prepare_at(&s.project, &mut cc, Some(t)).unwrap(); render::render_frame(&s.project, t, &cc).unwrap().d }
fn px(d: &[f32], x: usize, y: usize) -> [f32; 3] { let i = (y * 96 + x) * 4; [d[i], d[i + 1], d[i + 2]] }
fn is_red(p: [f32; 3]) -> bool { p[0] > 0.95 && p[1] < 0.05 && p[2] < 0.05 }
fn near(a: f64, b: f64, tol: f64, what: &str) { assert!((a - b).abs() <= tol, "{what}: {a} vs {b} (tol {tol})"); }

#[test]
fn track_planar_follows_the_plane_and_pins_the_target_layer() {
    let p = plate("plain", false); let (mut s, m, r) = setup(&p);
    let res = c(&mut s, "track_planar", json!({"clip_id": m, "corners": CORNERS, "target_clip_id": r, "return_quads": true}));
    assert_eq!(res["frames"], 6); assert_eq!(res["lost_frames"], 0); assert!(res["min_conf"].as_f64().unwrap() > 0.8, "{res}");
    let base = [24.0, 20.0, 72.0, 20.0, 72.0, 60.0, 24.0, 60.0];
    for (k, q) in res["quads"].as_array().unwrap().iter().enumerate() { for i in 0..8 { near(q[i].as_f64().unwrap(), base[i] + if i % 2 == 0 { k as f64 } else { 0.0 }, 0.1, &format!("frame {k} coord {i}")); } }
    let cl = s.project.find_clip(&r).unwrap(); let q3 = planar::pin_quad(cl, 0.3).unwrap(); near(q3[0][0], 27.0, 0.1, "pin0x at 0.3"); near(q3[2][1], 60.0, 0.1, "pin2y at 0.3");
    // rendered: red inside the tracked quad at frame 3, the footage outside it
    let f = frame(&s, 0.3); assert!(is_red(px(&f, 50, 40)) && is_red(px(&f, 29, 22)), "{:?}", px(&f, 50, 40)); assert!(!is_red(px(&f, 10, 10)) && !is_red(px(&f, 25, 40)));
    std::fs::remove_file(p).unwrap();
}

#[test]
fn occluder_mattes_cut_the_pinned_layer_where_something_covers_the_plane() {
    let p = plate("occ", true);
    for mode in ["anchor", "adaptive"] {
        let (mut s, m, r) = setup(&p);
        c(&mut s, "track_planar", json!({"clip_id": m, "corners": CORNERS, "target_clip_id": r, "occluder_matte": true, "occluder_mode": mode}));
        let f = frame(&s, 0.4); let o = px(&f, 50, 40); assert!(o[0] < 0.2 && o[1] < 0.2, "{mode}: occluder pixel {o:?} not cut");
        assert!(is_red(px(&f, 32, 26)) && is_red(px(&f, 66, 54)), "{mode}: visible plane not red"); assert!(is_red(px(&frame(&s, 0.1), 50, 40)), "{mode}: frame 1 has no occluder");
    }
    std::fs::remove_file(p).unwrap();
}

#[test]
fn mesh_pin_and_mesh_tracker() {
    let p = plate("mesh", false); let (mut s, m, r) = setup(&p);
    let res = c(&mut s, "track_mesh", json!({"clip_id": m, "corners": CORNERS, "nx": 3, "ny": 3, "target_clip_id": r, "return_verts": true}));
    assert_eq!(res["frames"], 6); let v = res["verts"].as_array().unwrap();
    for (k, fv) in v.iter().enumerate() { for j in 0..9 { let (x0, y0) = (24.0 + 24.0 * (j % 3) as f64, 20.0 + 20.0 * (j / 3) as f64);
        near(fv[2 * j].as_f64().unwrap(), x0 + k as f64, 0.3, &format!("frame {k} vertex {j} x")); near(fv[2 * j + 1].as_f64().unwrap(), y0, 0.3, &format!("frame {k} vertex {j} y")); } }
    let f = frame(&s, 0.3); assert!(is_red(px(&f, 50, 40)) && !is_red(px(&f, 10, 10)));
    // a hand-set 2x2 mesh is a corner pin
    let t = s.project.tracks[1].id.clone(); let r2 = c(&mut s, "add_clip", json!({"track_id": t, "source_type": "solid", "color": [0, 0, 1], "start": 0.6, "duration": 0.4}))["clip_id"].as_str().unwrap().to_string();
    c(&mut s, "set_mesh_pin", json!({"clip_id": r2, "nx": 2, "ny": 2, "keys": [{"time": 0, "verts": [10, 10, 50, 10, 10, 50, 50, 50]}]}));
    let f = frame(&s, 0.7); assert!(px(&f, 30, 30)[2] > 0.95 && px(&f, 60, 60)[2] < 0.05);
    std::fs::remove_file(p).unwrap();
}

fn q_at(k: f64) -> Vec<f64> { (0..8).map(|i| CORNERS[i] + if i % 2 == 0 { k / 96.0 } else { 0.0 }).collect() }

#[test]
fn validate_planar_track_finds_no_disagreement_on_a_clean_plane() {
    let p = plate("validate", false); let (mut s, m, _) = setup(&p);
    let r = c(&mut s, "validate_planar_track", json!({"clip_id": m, "corners": CORNERS, "anchors": 2, "backward": true, "context_corners": [0.15, 0.15, 0.85, 0.15, 0.85, 0.85, 0.15, 0.85]}));
    assert_eq!(r["frames"], 6); assert_eq!(r["anchors_used"], json!([1, 3])); assert_eq!(r["backward_anchors_used"], json!([5, 3]));
    assert_eq!(r["tested_frames"], 4); assert_eq!(r["untested_frames"], 2); assert_eq!(r["flagged_frames"], 0); assert!(r["first_flagged_frame"].is_null());
    assert!(r["disagreement_px"]["max"].as_f64().unwrap() < 0.2, "{}", r["disagreement_px"]); assert!(r["measured_on"].as_str().unwrap().starts_with("target quad"));
    std::fs::remove_file(p).unwrap();
}

#[test]
fn anchored_planar_track_passes_through_every_anchor_and_flags_bad_gaps() {
    let p = plate("anchored", false); let (mut s, m, r) = setup(&p);
    let res = c(&mut s, "track_planar_anchored", json!({"clip_id": m, "anchors": [{"frame": 5, "corners": q_at(5.0)}, {"frame": 0, "corners": q_at(0.0)}], "return_quads": true, "target_clip_id": r, "expand": 0.25}));
    assert_eq!((res["frames"].as_u64(), res["first_frame"].as_u64(), res["last_frame"].as_u64()), (Some(6), Some(0), Some(5))); assert_eq!(res["gaps_needing_anchor"], 0);
    let g = &res["gaps"][0]; assert!(g["forward_closure_px"].as_f64().unwrap() < 0.2 && g["backward_closure_px"].as_f64().unwrap() < 0.2, "{g}"); assert_eq!(g["suggest_anchor_frame"], 2);
    for (k, q) in res["quads"].as_array().unwrap().iter().enumerate() { near(q[0].as_f64().unwrap(), 24.0 + k as f64, 0.15, &format!("frame {k}")); near(q[7].as_f64().unwrap(), 60.0, 0.15, "y"); }
    assert_eq!(res["conf"][0], 1.0); assert_eq!(res["conf"][5], 1.0);
    // pinned with a 25% margin: the target canvas covers the quad grown by a quarter of its size on every side
    let cl = s.project.find_clip(&r).unwrap(); let q0 = planar::pin_quad(cl, 0.0).unwrap(); near(q0[0][0], 12.0, 1e-6, "expanded x"); near(q0[0][1], 10.0, 1e-6, "expanded y"); near(q0[2][0], 84.0, 1e-6, "expanded x2");
    // a wrong last anchor (10 px off): the result still passes through it, the gap is flagged
    let bad = c(&mut s, "track_planar_anchored", json!({"clip_id": m, "anchors": [{"frame": 0, "corners": q_at(0.0)}, {"frame": 3, "corners": q_at(13.0)}], "return_quads": true, "threshold_px": 2}));
    assert_eq!(bad["gaps_needing_anchor"], 1); assert_eq!(bad["gaps"][0]["needs_anchor"], true); near(bad["quads"][3][0].as_f64().unwrap(), 37.0, 1e-9, "passes through the anchor");
    std::fs::remove_file(p).unwrap();
}
