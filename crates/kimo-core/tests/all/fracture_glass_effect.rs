//! fracture_glass (src/fracture.rs) through the renderer on a tiny 32x32 two-layer project: holes and shard transmission
//! read the layers below. The "simple" layout used below (impact at the centre, 5 radials, r0 = 0.2 heights,
//! ring_growth 2, no debris) has 11 Voronoi sites: the impact and two rings of 5 sites at 6.4 and 12.8 px (+-1.6 px
//! jitter). The impact cell therefore contains every point within 2.4 px of (16,16), and its release time is at most
//! release_delay + release_spread * 0.1 + 1.3 * release_jitter = 0.22 s after impact.
use kimo_core::commands::{dispatch, Session};
use kimo_core::effects::Frame;
use serde_json::{json, Value};

fn px(f: &Frame, x: usize, y: usize) -> [f32; 4] { let o = (y * f.w + x) * 4; [f.d[o], f.d[o + 1], f.d[o + 2], f.d[o + 3]] }
fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n}: {e}")) }
/// Blue solid below, red solid above carrying fracture_glass (still shards, simple layout); centre pixel at time t.
fn layered(fx: Value, t: f64) -> [f32; 4] {
    let mut s = Session::new();
    c(&mut s, "project_set", json!({"width": 32, "height": 32, "fps": 25}));
    let t1 = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let t2 = c(&mut s, "add_track", json!({}))["track_id"].clone();
    c(&mut s, "add_clip", json!({"track_id": t1, "source_type": "solid", "color": [0, 0, 1], "start": 0, "duration": 3}));
    let id = c(&mut s, "add_clip", json!({"track_id": t2, "source_type": "solid", "color": [1, 0, 0], "start": 0, "duration": 3}))["clip_id"].clone();
    let mut p = json!({"radials": 5, "ring_growth": 2, "r0": 0.2, "debris": 0, "max_cells": 40, "gravity": 0, "push": 0, "tumble": 0, "toward_camera": 0, "vel_noise": 0, "thickness": 0, "blur_samples": 1,
        "face_refraction": 0, "dispersion": 0, "fresnel": 0, "sheen": 0, "bevel": 0, "glint": 0, "face_opacity": 1, "fade_start": 5, "clear_time": 6, "hole_dim": 1, "hole_blur": 0});
    for (k, v) in fx.as_object().unwrap() { p[k] = v.clone(); }
    c(&mut s, "add_effect", json!({"clip_id": id, "kind": "fracture_glass", "params": p}));
    let f = kimo_core::render::render_frame(&s.project, t, &s.cache).unwrap();
    px(&f, 16, 16)
}

#[test]
fn holes_and_transmission_read_the_layers_below() {
    let near = |a: [f32; 4], b: [f32; 3], what: &str| assert!((0..3).all(|j| (a[j] - b[j]).abs() < 0.02), "{what}: {a:?} vs {b:?}");
    // Shards pushed off-frame by gravity; hole_dim 1 makes the bare hole opaque black.
    near(layered(json!({"gravity": 100}), 1.0), [0.0, 0.0, 0.0], "opaque hole");
    near(layered(json!({"gravity": 100, "hole_fill": 1}), 1.0), [0.45, 0.0, 0.0], "pane copy, dimmed");
    near(layered(json!({"gravity": 100, "hole_fill": 1, "hole_source": 1}), 1.0), [0.0, 0.0, 1.0], "layers below");
    // A shard still in place shows its own texture, or with transmission 1 the backdrop through its face.
    let own = layered(json!({}), 1.0);
    assert!(own[0] > 0.9 && own[2] < 0.02, "shard face carries the red source: {own:?}");
    let through = layered(json!({"transmission": 1}), 1.0);
    assert!(through[2] > 0.9 && through[0] < 0.02, "transmission 1 shows the blue layer below: {through:?}");
}
