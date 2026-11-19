//! roto_propagate (roto.rs / tools_3.rs): rotoscope assist (dense optical flow forward/back between path-mask
//! keyframes + edge snap) -- already shipped pre-v7, uncredited. This is the engine's real "roto brush" (AE's
//! own Roto Brush is also edge/flow based, not a deep neural model, so this IS the non-ML roto brush the
//! checklist asks for; only true AI/ML one-click object segmentation remains a genuine, stated gap).
//!
//! Same trackable-video technique as kimo_motion_v7.rs: a deterministic noise field panned by ffmpeg's crop
//! filter, since optical flow needs real texture, which a flat colour cannot give it.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};
use std::io::Write;

fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n} {a}: {e}")) }
fn id(v: &Value, k: &str) -> String { v[k].as_str().unwrap().to_string() }
fn hash_seed(s: &str) -> u64 { let mut h: u64 = 0xcbf29ce484222325; for b in s.bytes() { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); } h }
fn write_noise_ppm(path: &std::path::Path, w: usize, h: usize, seed: u64) {
    let mut s = seed; let mut next = move || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; s };
    let mut buf = Vec::with_capacity(w * h * 3);
    for _ in 0..w * h { let v = next(); buf.push((v & 0xff) as u8); buf.push(((v >> 8) & 0xff) as u8); buf.push(((v >> 16) & 0xff) as u8); }
    let mut f = std::fs::File::create(path).unwrap(); write!(f, "P6\n{w} {h}\n255\n").unwrap(); f.write_all(&buf).unwrap();
}
fn panning_clip(name: &str, x0: i32, y0: i32, vx: f64, vy: f64, frames: usize, fps: f64) -> std::path::PathBuf {
    let noise = std::env::temp_dir().join(format!("kimo-roto-{}-{name}-noise.ppm", std::process::id()));
    write_noise_ppm(&noise, 400, 300, 0xC0FFEE ^ std::process::id() as u64 ^ hash_seed(name));
    let out = std::env::temp_dir().join(format!("kimo-roto-{}-{name}.mp4", std::process::id()));
    let vf = format!("crop=160:90:x='{x0}+{vx}*t':y='{y0}+{vy}*t'");
    let st = std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-loop", "1", "-i", noise.to_str().unwrap(),
        "-vf", &vf, "-pix_fmt", "yuv444p", "-c:v", "libx264", "-qp", "0", "-r", &fps.to_string(), "-frames:v", &frames.to_string(), out.to_str().unwrap()])
        .status().expect("ffmpeg"); assert!(st.success(), "ffmpeg pan render failed");
    out
}
fn rect_pts(cx: f64, cy: f64, hw: f64, hh: f64) -> Value { json!([{"x": cx - hw, "y": cy - hh}, {"x": cx + hw, "y": cy - hh}, {"x": cx + hw, "y": cy + hh}, {"x": cx - hw, "y": cy + hh}]) }

#[test]
fn roto_propagate_is_deterministic_and_idempotent_on_a_second_identical_run() {
    // not a golden-exactness claim -- just proof the whole pipeline (dense flow + distance blend + snap) is a
    // pure function of the project state, not touched by wall-clock/thread-order nondeterminism.
    let path = panning_clip("determ", 60, 60, 8.0, 0.0, 20, 10.0);
    let build = |path: &std::path::Path| -> Vec<f64> {
        let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 160, "height": 90, "fps": 10.0}));
        let t = id(&c(&mut s, "add_track", json!({})), "track_id");
        let a = id(&c(&mut s, "import_media", json!({"path": path.to_str().unwrap()})), "asset_id");
        let clip = id(&c(&mut s, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0.0, "duration": 2.0})), "clip_id");
        let m = id(&c(&mut s, "add_mask", json!({"clip_id": clip, "kind": "path", "interp": "linear", "points": rect_pts(0.5, 0.5, 0.1, 0.1)})), "mask_id");
        c(&mut s, "set_mask_path_key", json!({"clip_id": clip, "mask_id": m, "time": 0.0, "points": rect_pts(0.4, 0.5, 0.1, 0.1)}));
        c(&mut s, "set_mask_path_key", json!({"clip_id": clip, "mask_id": m, "time": 1.9, "points": rect_pts(0.6, 0.5, 0.1, 0.1)}));
        c(&mut s, "roto_propagate", json!({"clip_id": clip, "mask_id": m, "snap": 0}));
        let p = &s.project; let cl = p.find_clip(&clip).unwrap(); let l = cl.mask.as_ref().unwrap().layers.iter().find(|l| l.id == m).unwrap();
        l.path_keys.iter().flat_map(|k| k.points.iter().flat_map(|pt| [pt.x, pt.y])).collect()
    };
    let a = build(&path); let b = build(&path);
    assert_eq!(a, b, "the same project and footage should propagate to bit-identical path keys on a second run");
    // and it should actually be MOVING the mask, not leaving it dead at its straight-line average (0.5,0.5)
    // across the whole sweep -- the keys were seeded off-centre (0.4 -> 0.6), so some interior frame must differ.
    assert!(a.iter().any(|v| (v - 0.5).abs() > 1e-6), "propagated points should not all sit exactly at 0.5");
}

#[test]
fn roto_propagate_fills_every_intermediate_frame_between_two_keys() {
    let path = panning_clip("fill", 60, 60, 8.0, 0.0, 20, 10.0);
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 160, "height": 90, "fps": 10.0}));
    let t = id(&c(&mut s, "add_track", json!({})), "track_id");
    let a = id(&c(&mut s, "import_media", json!({"path": path.to_str().unwrap()})), "asset_id");
    let clip = id(&c(&mut s, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0.0, "duration": 2.0})), "clip_id");
    let m = id(&c(&mut s, "add_mask", json!({"clip_id": clip, "kind": "path", "interp": "linear", "points": rect_pts(0.5, 0.5, 0.1, 0.1)})), "mask_id");
    c(&mut s, "set_mask_path_key", json!({"clip_id": clip, "mask_id": m, "time": 0.0, "points": rect_pts(0.4, 0.5, 0.1, 0.1)}));
    c(&mut s, "set_mask_path_key", json!({"clip_id": clip, "mask_id": m, "time": 1.9, "points": rect_pts(0.6, 0.5, 0.1, 0.1)}));
    let r = c(&mut s, "roto_propagate", json!({"clip_id": clip, "mask_id": m, "snap": 0}));
    // 20 frames at 10fps spanning 0.0..1.9s = 20 frames total; propagate must have written a key on every one
    assert_eq!(r["keys"].as_u64().unwrap(), 20, "a path key on every frame from 0.0 to 1.9s, got {r}");
    assert!(r["frames_filled"].as_u64().unwrap() >= 18, "nearly all the in-between frames should be newly filled, got {r}");
}

/// Fast tiny-media checks: ffv1 96x80, 10 fps, 8 frames of a smooth texture translating exactly +1 px in x per frame.
fn tex_plate(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("kimo-roto-tex-{tag}-{}.mkv", std::process::id()));
    let t = "128+45*sin(0.55*(X-N))*sin(0.47*Y+0.3)+30*sin(0.21*(X-N)+0.33*Y)+20*cos(0.13*(X-N)-0.29*Y+1)";
    let g = format!("nullsrc=s=96x80:r=10:d=0.8,format=rgb24,geq=r='{t}':g='{t}':b='{t}'");
    assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", &g, "-c:v", "ffv1", "-pix_fmt", "bgr0", p.to_str().unwrap()]).status().unwrap().success()); p
}
fn tex_session(path: &std::path::Path) -> (Session, String, String) {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 96, "height": 80, "fps": 10}));
    let t = id(&c(&mut s, "add_track", json!({})), "track_id"); let a = id(&c(&mut s, "import_media", json!({"path": path.to_str().unwrap()})), "asset_id");
    let clip = id(&c(&mut s, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0.0, "duration": 0.8})), "clip_id");
    let m = id(&c(&mut s, "add_mask", json!({"clip_id": clip, "kind": "path", "interp": "linear", "points": rect_pts(0.5, 0.5, 0.3, 0.3)})), "mask_id");
    c(&mut s, "set_mask_path_key", json!({"clip_id": clip, "mask_id": m, "time": 0.0, "points": rect_pts(0.5, 0.5, 0.3, 0.3)}));
    c(&mut s, "set_mask_path_key", json!({"clip_id": clip, "mask_id": m, "time": 0.7, "points": rect_pts(0.5 + 7.0 / 96.0, 0.5, 0.3, 0.3)}));
    (s, clip, m)
}
fn keys_of(s: &Session, clip: &str, m: &str) -> Vec<kimo_core::masks::PathKey> { s.project.find_clip(clip).unwrap().mask.as_ref().unwrap().layers.iter().find(|l| l.id == m).unwrap().path_keys.clone() }

#[test]
fn tiny_roto_propagate_follows_the_footage_between_keys() {
    let p = tex_plate("follow"); let (mut s, clip, m) = tex_session(&p);
    let r = c(&mut s, "roto_propagate", json!({"clip_id": clip, "mask_id": m, "snap": 0, "quality": 2}));
    assert_eq!(r["keys"], 8); assert_eq!(r["frames_filled"], 6); assert_eq!(r["segments"], 1); assert_eq!(r["mean_snap_px"], 0.0);
    for (i, k) in keys_of(&s, &clip, &m).iter().enumerate() { assert!((k.t - i as f64 / 10.0).abs() < 1e-9);
        for (j, pt) in k.points.iter().enumerate() { let want = [(0.2, 0.2), (0.8, 0.2), (0.8, 0.8), (0.2, 0.8)][j]; assert!((pt.x * 96.0 - (want.0 * 96.0 + i as f64)).abs() < 1.0 && (pt.y * 80.0 - want.1 * 80.0).abs() < 1.0, "frame {i} point {j}: ({}, {})", pt.x * 96.0, pt.y * 80.0); } }
    // snapping moves the outline toward the strongest nearby edge, by at most `snap` px
    let (mut s2, clip2, m2) = tex_session(&p); let r2 = c(&mut s2, "roto_propagate", json!({"clip_id": clip2, "mask_id": m2, "snap": 3}));
    let ms = r2["mean_snap_px"].as_f64().unwrap(); assert!(ms > 0.0 && ms <= 3.0, "{r2}");
    std::fs::remove_file(p).unwrap();
}
