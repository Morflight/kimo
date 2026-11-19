//! Post-transform effects that read the composite below a layer: frosted_glass (frosted.rs) through the renderer, and
//! cast_shadow (castshadow.rs) through the renderer. Tiny 32x32 scenes.
use kimo_core::commands::{dispatch, Session};
use kimo_core::effects::Frame;
use serde_json::{json, Value};

fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n}: {e}")) }
fn at(f: &Frame, x: usize, y: usize) -> [f32; 4] { let o = (y * f.w + x) * 4; [f.d[o], f.d[o + 1], f.d[o + 2], f.d[o + 3]] }

/// Bottom: a full-frame solid carrying a vertical gradient_ramp, so the backdrop at row y is (y/32, y/64, 0).
/// Top: a red 24x24 rect (pane) carrying `kind` with `params`; optional clip properties (rot_x ...).
fn scene(kind: &str, params: Value, props: &[(&str, f64)]) -> Frame {
    let mut s = Session::new();
    c(&mut s, "project_set", json!({"width": 32, "height": 32, "fps": 25}));
    let t1 = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let t2 = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let bg = c(&mut s, "add_clip", json!({"track_id": t1, "source_type": "solid", "color": [0, 0, 0], "start": 0, "duration": 2}))["clip_id"].clone();
    c(&mut s, "add_effect", json!({"clip_id": bg, "kind": "gradient_ramp"}));
    let top = c(&mut s, "add_shape", json!({"track_id": t2, "shape": "rect", "width": 24, "height": 24, "fill": [1.0, 0.0, 0.0, 1.0], "start": 0, "duration": 2}))["clip_id"].clone();
    for (k, v) in props { c(&mut s, "set_property", json!({"clip_id": top, "property": k, "value": v})); }
    c(&mut s, "add_effect", json!({"clip_id": top, "kind": kind, "params": params}));
    kimo_core::render::render_frame(&s.project, 0.5, &s.cache).unwrap()
}
fn clear_glass(extra: Value) -> Value { let mut p = json!({"blur": 0, "refraction": 0, "dispersion": 0, "tint_amount": 0, "saturation": 1, "edge_light": 0}); for (k, v) in extra.as_object().unwrap() { p[k] = v.clone(); } p }
fn near(a: [f32; 4], b: [f32; 3], tol: f32, what: &str) { assert!((0..3).all(|j| (a[j] - b[j]).abs() <= tol), "{what}: {a:?} vs {b:?}"); }

#[test]
fn frosted_glass_shows_the_backdrop_through_the_pane() {
    // Clear glass: the pane shows exactly the backdrop under it.
    let f = scene("frosted_glass", clear_glass(json!({})), &[]);
    near(at(&f, 16, 16), [0.5, 0.25, 0.0], 1e-5, "clear pane = backdrop"); near(at(&f, 16, 8), [0.25, 0.125, 0.0], 1e-5, "row 8");
    near(at(&scene("frosted_glass", clear_glass(json!({"layer_mix": 1})), &[]), 16, 16), [1.0, 0.0, 0.0], 1e-5, "layer_mix 1 = the layer");
    near(at(&scene("frosted_glass", clear_glass(json!({"tint_amount": 1, "tint_r": 0, "tint_g": 1, "tint_b": 0})), &[]), 16, 16), [0.0, 1.0, 0.0], 1e-5, "full tint");
    let l = 0.2126 * 0.5 + 0.7152 * 0.25; near(at(&scene("frosted_glass", clear_glass(json!({"saturation": 0})), &[]), 16, 16), [l, l, l], 1e-5, "saturation 0 = luma");
    // magnify 2 about the pane centroid (15.5): row 8 samples row 15.5 + (8 - 15.5)/2 = 11.75; about magnify_cy = 0 it samples row 4.
    near(at(&scene("frosted_glass", clear_glass(json!({"magnify": 2})), &[]), 16, 8), [11.75 / 32.0, 11.75 / 64.0, 0.0], 1e-4, "magnify");
    near(at(&scene("frosted_glass", clear_glass(json!({"magnify": 2, "magnify_cx": 0.5, "magnify_cy": 0.0})), &[]), 16, 8), [4.0 / 32.0, 4.0 / 64.0, 0.0], 1e-4, "magnify about a point");
    // Frost blur on a linear ramp keeps the interior value; the bevel refracts and lights the pane edge only.
    let fr = scene("frosted_glass", json!({"tint_amount": 0, "saturation": 1}), &[]);
    near(at(&fr, 16, 16), [0.5, 0.25, 0.0], 0.06, "blurred ramp centre"); assert_ne!(at(&fr, 5, 16), at(&f, 5, 16), "edge refraction / rim light");
    // Fresnel adds the environment reflection; reflect_scene mixes in the (blurred) backdrop along the reflected ray.
    let fz = scene("frosted_glass", clear_glass(json!({"fresnel": 1})), &[]); assert_ne!(at(&fz, 16, 16), at(&f, 16, 16));
    let rs = scene("frosted_glass", clear_glass(json!({"fresnel": 1, "reflect_scene": 1})), &[]); assert_ne!(at(&rs, 16, 16), at(&fz, 16, 16));
    // A fully transparent pane (no alpha anywhere) has no centroid to magnify about and frosts nothing.
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 32, "height": 32, "fps": 25})); let t1 = c(&mut s, "add_track", json!({}))["track_id"].clone(); let t2 = c(&mut s, "add_track", json!({}))["track_id"].clone();
    c(&mut s, "add_clip", json!({"track_id": t1, "source_type": "solid", "color": [0.2, 0.4, 0.6], "start": 0, "duration": 2}));
    let top = c(&mut s, "add_shape", json!({"track_id": t2, "shape": "rect", "width": 24, "height": 24, "fill": [1.0, 0.0, 0.0, 0.0], "start": 0, "duration": 2}))["clip_id"].clone();
    c(&mut s, "add_effect", json!({"clip_id": top, "kind": "frosted_glass", "params": {"magnify": 2}}));
    let e = kimo_core::render::render_frame(&s.project, 0.5, &s.cache).unwrap(); near(at(&e, 16, 16), [0.2, 0.4, 0.6], 1e-5, "nothing frosted");
    // Outside the pane nothing changes.
    near(at(&fz, 0, 16), [0.5, 0.25, 0.0], 1e-5, "outside the pane");
}

#[test]
fn cast_shadow_runs_under_the_layer_in_the_render() {
    let f = scene("cast_shadow", json!({"light_size": 0}), &[]);
    let clear = scene("frosted_glass", clear_glass(json!({})), &[]);
    // The default light is up-left (shift ~ +1.0, +1.4 px), so the shadow falls down-right of the pane, darkening the backdrop there.
    assert!(at(&f, 28, 28)[0] < at(&clear, 28, 28)[0] - 0.05, "darker below-right: {:?} vs {:?}", at(&f, 28, 28), at(&clear, 28, 28));
    assert_eq!(at(&f, 1, 1), at(&clear, 1, 1), "no shadow up-left");
}
