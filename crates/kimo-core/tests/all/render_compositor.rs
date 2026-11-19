//! Compositor render path (render.rs) on a tiny 40x20 project: the legacy single-shape mask, corner pin, posterize time, echo
//! ghosts, the render_mask inspection PNG, legacy particles, adjustment layers, per-effect masks and layer emitters. A white
//! solid on black makes coverage readable as the red channel; expected values follow the documented formulas.
use kimo_core::commands::{dispatch, Session};
use kimo_core::render;
use serde_json::{json, Value};

const W: usize = 40; const H: usize = 20;
fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n}: {e}")) }
fn white() -> (Session, String) {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": W, "height": H, "fps": 10}));
    let t = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let id = c(&mut s, "add_clip", json!({"track_id": t, "source_type": "solid", "color": [1, 1, 1], "start": 0, "duration": 2}))["clip_id"].as_str().unwrap().to_string(); (s, id)
}
fn frame(s: &Session, t: f64) -> Vec<f32> { let mut k = render::Cache::new(); render::prepare_at(&s.project, &mut k, Some(t)).unwrap(); render::render_frame(&s.project, t, &k).unwrap().d }
fn red(f: &[f32], x: usize, y: usize) -> f32 { f[(y * W + x) * 4] }

#[test]
fn legacy_mask_shapes_cut_the_layer_alpha() {
    let masked = |a: Value| { let (mut s, id) = white(); let mut a = a; a["clip_id"] = json!(id); a["feather"] = json!(0.001); c(&mut s, "set_mask", a); frame(&s, 0.5) };
    // rect 0.5 x 0.5 centred: columns 10..30, rows 5..15 (pixel x samples x/W)
    let f = masked(json!({"shape": "rect"})); assert_eq!((red(&f, 20, 10), red(&f, 12, 10), red(&f, 5, 10), red(&f, 20, 2)), (1.0, 1.0, 0.0, 0.0));
    let f = masked(json!({"shape": "rect", "invert": true})); assert_eq!((red(&f, 20, 10), red(&f, 5, 10)), (0.0, 1.0));
    // rotated 90 degrees in normalised units: |ny| < w/2 and |nx| < h/2
    let f = masked(json!({"shape": "rect", "w": 0.5, "h": 0.1, "rotation": 90})); assert_eq!((red(&f, 20, 6), red(&f, 12, 10)), (1.0, 0.0));
    let f = masked(json!({"shape": "rect", "w": 0.5, "h": 0.1})); assert_eq!((red(&f, 20, 6), red(&f, 12, 10)), (0.0, 1.0));
    let f = masked(json!({"shape": "ellipse"})); assert_eq!((red(&f, 20, 10), red(&f, 12, 4)), (1.0, 0.0));
    // linear: everything left of cx
    let f = masked(json!({"shape": "linear"})); assert_eq!((red(&f, 5, 10), red(&f, 35, 10)), (1.0, 0.0));
}

#[test]
fn corner_pin_warps_the_layer_by_a_homography() {
    let (mut s, id) = white();
    // top corners pulled in by 10 px: a trapezoid, full width at the bottom
    c(&mut s, "set_corner_pin", json!({"clip_id": id, "corners": [[10, 0], [-10, 0], [0, 0], [0, 0]]}));
    let f = frame(&s, 0.5); assert_eq!((red(&f, 2, 1), red(&f, 37, 1)), (0.0, 0.0), "outside the trapezoid"); assert!(red(&f, 20, 10) > 0.999 && red(&f, 8, 15) > 0.999, "inside: bilinear samples of white: {} {}", red(&f, 20, 10), red(&f, 8, 15));
    // all four corners collapsed onto one point is singular: the layer is left unwarped
    c(&mut s, "set_corner_pin", json!({"clip_id": id, "corners": [[0, 0], [-40, 0], [-40, -20], [0, -20]]}));
    let f = frame(&s, 0.5); assert_eq!((red(&f, 0, 0), red(&f, 39, 19)), (1.0, 1.0));
    c(&mut s, "set_corner_pin", json!({"clip_id": id, "corners": null})); assert!(s.project.tracks[0].clips[0].corners.is_none());
}

/// A 4x4 white square moving from x = -12 to x = +12 px (centre offsets) over 1 s, on black.
fn mover() -> (Session, String) {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": W, "height": H, "fps": 10}));
    let t = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let id = c(&mut s, "add_shape", json!({"track_id": t, "shape": "rect", "width": 4, "height": 4, "fill": [1, 1, 1], "start": 0, "duration": 2}))["clip_id"].as_str().unwrap().to_string();
    c(&mut s, "set_keyframe", json!({"clip_id": id, "property": "x", "time": 0, "value": -12, "ease": "linear"})); c(&mut s, "set_keyframe", json!({"clip_id": id, "property": "x", "time": 1, "value": 12}));
    (s, id)
}

#[test]
fn posterize_time_holds_motion_between_steps() {
    let (mut s, id) = mover(); let free = frame(&s, 0.4);
    c(&mut s, "set_posterize_time", json!({"clip_id": id, "fps": 2}));
    assert_eq!(frame(&s, 0.4), frame(&s, 0.0), "at 2 fps, 0.4 s shows the 0 s pose"); assert_ne!(frame(&s, 0.4), free);
    assert_eq!(frame(&s, 0.7), frame(&s, 0.5), "next step at 0.5 s");
}

#[test]
fn echo_ghosts_trail_the_layer() {
    // at 0.5 s the square is at x = 0 (columns 18..22); 0.25 s earlier it was at x = -6 (columns 12..16)
    let (mut s, id) = mover(); let row = 10;
    let base = frame(&s, 0.5); assert_eq!((red(&base, 20, row), red(&base, 14, row)), (1.0, 0.0));
    c(&mut s, "set_echo", json!({"clip_id": id, "count": 1, "spacing": 0.25, "decay": 0.5}));
    let f = frame(&s, 0.5); assert_eq!(red(&f, 20, row), 1.0, "the layer itself"); let g = red(&f, 14, row); assert!(g > 0.2 && g < 0.9, "half-opacity ghost: {g}"); assert_eq!(red(&f, 30, row), 0.0);
    c(&mut s, "set_echo", json!({"clip_id": id, "count": 1, "spacing": 0.25, "decay": 0.5, "mode": "add"}));
    let a = frame(&s, 0.5); assert_eq!(red(&a, 20, row), 1.0); assert!((red(&a, 14, row) - g).abs() < 1e-6, "add over black equals the composite ghost");
    // average: the layer and its ghost each get half weight
    c(&mut s, "set_echo", json!({"clip_id": id, "count": 1, "spacing": 0.25, "mode": "average"}));
    let v = frame(&s, 0.5); let (cur, gh) = (red(&v, 20, row), red(&v, 14, row)); assert!(cur < 1.0 && cur > 0.2 && (cur - gh).abs() < 1e-6, "equal weights: {cur} {gh}");
    c(&mut s, "set_echo", json!({"clip_id": id, "count": 0})); assert_eq!(frame(&s, 0.5), base, "count 0 removes the echo");
}

fn png_rgba(p: &std::path::Path) -> Vec<u8> {
    let o = std::process::Command::new("ffmpeg").args(["-v", "error", "-i", p.to_str().unwrap(), "-f", "rawvideo", "-pix_fmt", "rgba", "-"]).output().unwrap(); assert!(o.status.success()); o.stdout
}

#[test]
fn render_mask_png_shows_matte_overlay_and_cutout() {
    let (mut s, id) = white();
    let m = c(&mut s, "add_mask", json!({"clip_id": id, "kind": "rect", "params": {"w": 0.5, "h": 0.5}}))["mask_id"].as_str().unwrap().to_string();
    let p = std::env::temp_dir().join(format!("kimo-rendermask-{}.png", std::process::id()));
    let px = |b: &[u8], x: usize, y: usize| [b[(y * W + x) * 4], b[(y * W + x) * 4 + 1], b[(y * W + x) * 4 + 2], b[(y * W + x) * 4 + 3]];
    let run = |s: &mut Session, extra: Value| { let mut a = json!({"clip_id": id, "time": 0.5, "path": p}); for (k, v) in extra.as_object().unwrap() { a[k] = v.clone(); } c(s, "render_mask", a); png_rgba(&p) };
    let b = run(&mut s, json!({})); assert_eq!((px(&b, 20, 10), px(&b, 2, 2)), ([255, 255, 255, 255], [0, 0, 0, 255]), "matte");
    let b = run(&mut s, json!({"mask_id": m, "view": "overlay"})); let q = px(&b, 20, 10); assert_eq!((q[0], q[3]), (255, 255)); assert!((q[1] as i32 - 166).abs() <= 1, "65 % of white under the red tint: {q:?}"); assert_eq!(px(&b, 2, 2), [255, 255, 255, 255]);
    let b = run(&mut s, json!({"view": "cutout"})); assert_eq!((px(&b, 20, 10), px(&b, 2, 2)[3]), ([255, 255, 255, 255], 0));
    // a clip without a mask stack shows full coverage; a legacy mask is shown through the cutout alpha
    let (mut s2, id2) = white(); c(&mut s2, "set_mask", json!({"clip_id": id2, "shape": "linear", "feather": 0.001}));
    let b = { c(&mut s2, "render_mask", json!({"clip_id": id2, "time": 0.5, "path": p})); png_rgba(&p) }; assert_eq!(px(&b, 35, 10), [255, 255, 255, 255], "no stack: everything selected");
    let b = { c(&mut s2, "render_mask", json!({"clip_id": id2, "time": 0.5, "path": p, "view": "cutout"})); png_rgba(&p) }; assert_eq!((px(&b, 5, 10)[3], px(&b, 35, 10)[3]), (255, 0));
    std::fs::remove_file(&p).unwrap();
}

#[test]
fn legacy_particle_kinds_draw_seeded_tinted_sprites() {
    let draw = |kind: &str, seed: u64, t: f64| { let mut s = Session::new(); c(&mut s, "project_set", json!({"width": W, "height": H, "fps": 10}));
        let tr = c(&mut s, "add_track", json!({}))["track_id"].clone();
        c(&mut s, "add_particles", json!({"track_id": tr, "kind": kind, "start": 0, "duration": 3, "count": 30, "size": 2, "seed": seed, "color": [1.0, 0.5, 0.25]})); frame(&s, t) };
    let mut seen: Vec<Vec<f32>> = vec![];
    for kind in ["snow", "rain", "embers", "sparks", "dust"] {
        let f = draw(kind, 3, 0.7); let ink: f32 = f.chunks(4).map(|p| p[0]).sum(); assert!(ink > 0.5, "{kind} draws: {ink}");
        // over black every pixel is colour * coverage: g = r / 2, b = r / 4
        for p in f.chunks(4) { assert!((p[1] - p[0] * 0.5).abs() < 1e-5 && (p[2] - p[0] * 0.25).abs() < 1e-5, "{kind} tint {p:?}"); }
        assert_eq!(draw(kind, 3, 0.7), f, "{kind} deterministic"); assert_ne!(draw(kind, 4, 0.7), f, "{kind} seed changes the field");
        assert!(seen.iter().all(|o| *o != f), "{kind} differs from the other kinds"); seen.push(f);
    }
}

#[test]
fn adjustment_layers_apply_through_their_mask_and_opacity() {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": W, "height": H, "fps": 10}));
    let t1 = c(&mut s, "add_track", json!({}))["track_id"].clone(); let t2 = c(&mut s, "add_track", json!({}))["track_id"].clone();
    c(&mut s, "add_clip", json!({"track_id": t1, "source_type": "solid", "color": [0.2, 0.4, 0.6], "start": 0, "duration": 2}));
    let adj = c(&mut s, "add_adjustment_layer", json!({"track_id": t2, "start": 0, "duration": 2}))["clip_id"].as_str().unwrap().to_string();
    c(&mut s, "add_effect", json!({"clip_id": adj, "kind": "invert"}));
    let px = |f: &[f32], x: usize, y: usize| [f[(y * W + x) * 4], f[(y * W + x) * 4 + 1], f[(y * W + x) * 4 + 2]];
    let near3 = |a: [f32; 3], b: [f32; 3], what: &str| for j in 0..3 { assert!((a[j] - b[j]).abs() < 1e-6, "{what}: {a:?} vs {b:?}"); };
    let f = frame(&s, 0.5); near3(px(&f, 20, 10), [0.8, 0.6, 0.4], "full inversion"); near3(px(&f, 2, 2), [0.8, 0.6, 0.4], "everywhere");
    // a centred rect mask and half opacity: inside = mean of the backdrop and its inversion, outside untouched
    c(&mut s, "set_mask", json!({"clip_id": adj, "shape": "rect", "feather": 0.001}));
    c(&mut s, "set_keyframe", json!({"clip_id": adj, "property": "opacity", "time": 0, "value": 0.5}));
    let f = frame(&s, 0.5); near3(px(&f, 20, 10), [0.5, 0.5, 0.5], "half-mixed inside"); near3(px(&f, 2, 2), [0.2, 0.4, 0.6], "outside the mask");
}

#[test]
fn per_effect_mask_layers_limit_an_effect() {
    // white solid; a mode-none rect layer over the left half selects where the first invert applies; the second invert is global
    let (mut s, id) = white();
    let m = c(&mut s, "add_mask", json!({"clip_id": id, "kind": "rect", "mode": "none", "params": {"cx": 0.25, "w": 0.5, "h": 1.0}}))["mask_id"].as_str().unwrap().to_string();
    c(&mut s, "add_effect", json!({"clip_id": id, "kind": "invert", "strings": {"mask_layer": m}}));
    let f = frame(&s, 0.5); assert_eq!((red(&f, 5, 10), red(&f, 35, 10)), (0.0, 1.0), "masked invert: left only");
    c(&mut s, "add_effect", json!({"clip_id": id, "kind": "invert"}));
    let f = frame(&s, 0.5); assert_eq!((red(&f, 5, 10), red(&f, 35, 10)), (1.0, 0.0), "then a global invert");
}

#[test]
fn layer_emitters_spawn_on_the_emit_clip_silhouette() {
    // a black 8x8 square centred at column 10 (invisible on black) is the emit shape; motionless white particles show where they spawn
    let run = |extra: Value| { let mut s = Session::new(); c(&mut s, "project_set", json!({"width": W, "height": H, "fps": 10}));
        let t1 = c(&mut s, "add_track", json!({}))["track_id"].clone(); let t2 = c(&mut s, "add_track", json!({}))["track_id"].clone();
        let sq = c(&mut s, "add_shape", json!({"track_id": t1, "shape": "rect", "width": 8, "height": 8, "fill": [0, 0, 0], "start": 0, "duration": 2}))["clip_id"].as_str().unwrap().to_string();
        c(&mut s, "set_property", json!({"clip_id": sq, "property": "x", "value": -10}));
        let mut cfg = json!({"shape": "layer", "emit_clip": sq, "rate": 300, "life": 3, "speed": 0, "gravity": 0, "size0": 1, "size1": 1, "alpha0": 1, "alpha1": 1, "color0": [1, 1, 1], "color1": [1, 1, 1], "seed": 3});
        for (k, v) in extra.as_object().unwrap() { cfg[k] = v.clone(); } let cfg = serde_json::to_string(&cfg).unwrap().replace("SQ", &sq); let cfg: Value = serde_json::from_str(&cfg).unwrap();
        c(&mut s, "add_emitter", json!({"track_id": t2, "start": 0, "duration": 2, "config": cfg})); frame(&s, 0.5) };
    for extra in [json!({}), json!({"emit_follow": true, "prewarm": true, "colliders": [{"kind": "layer", "clip_id": "SQ", "follow": true}]}), json!({"emit_edge": true, "colliders": [{"kind": "layer", "clip_id": "SQ"}]})] {
        let f = run(extra.clone()); let lit: Vec<(usize, usize)> = (0..H).flat_map(|y| (0..W).map(move |x| (x, y))).filter(|&(x, y)| red(&f, x, y) > 0.05).collect();
        assert!(!lit.is_empty(), "{extra}: particles drawn"); assert!(lit.iter().all(|&(x, y)| (3..18).contains(&x) && (3..18).contains(&y)), "{extra}: only around the square: {lit:?}");
    }
}
