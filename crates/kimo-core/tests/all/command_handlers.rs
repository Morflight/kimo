//! Functional tests of editing tools that had no behavioural test of their own, driven through `dispatch` on tiny
//! canvases (64x32, 10 fps) with values computed by hand. Media-driven trackers use a 160x120, 10 fps, 1 s FFmpeg lavfi
//! plate. Argument refusals of these tools belong in unit tests next to their handlers, not here.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};

fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n} {a}: {e}")) }
fn session() ->(Session, String) { let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 64, "height": 32, "fps": 10})); let t = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string(); (s, t) }
fn shape(s: &mut Session, t: &str, start: f64, dur: f64) -> String { c(s, "add_shape", json!({"track_id": t, "shape": "rect", "width": 8, "height": 8, "start": start, "duration": dur}))["clip_id"].as_str().unwrap().to_string() }
fn text(s: &mut Session, t: &str) -> String { c(s, "add_text", json!({"track_id": t, "text": "ab", "start": 0, "duration": 2}))["clip_id"].as_str().unwrap().to_string() }
fn keys(s: &Session, id: &str, prop: &str) -> Vec<(f64, f64)> { s.project.find_clip(id).unwrap().props.get(prop).map(|p| p.keys.iter().map(|k| (k.t, k.v)).collect()).unwrap_or_default() }
fn close(a: f64, b: f64, tol: f64, what: &str) { assert!((a - b).abs() <= tol, "{what}: {a} vs {b}"); }
fn gray(png: &str, w: usize, h: usize) -> Vec<u8> { let o = std::process::Command::new("ffmpeg").args(["-v", "error", "-i", png, "-f", "rawvideo", "-pix_fmt", "gray", "-"]).stdin(std::process::Stdio::null()).output().unwrap(); assert_eq!(o.stdout.len(), w * h, "decode {png}"); o.stdout }
fn cj(s: &Session, id: &str) -> Value { serde_json::to_value(s.project.find_clip(id).unwrap()).unwrap() }

// ---------------------------------------------------------------- text path, mesh pin, physics, motion presets, exports

#[test] fn set_text_path_shapes_points_animation_and_removal() {
    let (mut s, t) = session(); let id = text(&mut s, &t);
    // remove on a clip that never had text effects is a no-op success
    assert_eq!(c(&mut s, "set_text_path", json!({"clip_id": id, "remove": true})), json!({"ok": true}));
    assert!(cj(&s, &id)["text_fx"].is_null());
    c(&mut s, "set_text_path", json!({"clip_id": id, "shape": "circle", "cx": 0.4, "cy": 0.6, "rx": 0.2, "ry": 0.1, "start": 0.25, "flip": true}));
    let fx = &cj(&s, &id)["text_fx"];
    assert_eq!(fx["path"]["closed"], true); assert_eq!(fx["path"]["points"].as_array().unwrap().len(), 12);
    assert_eq!(fx["path_flip"], true); assert_eq!(fx["path_start"]["base"], 0.25);
    // first circle point is (cx + rx, cy)
    close(fx["path"]["points"][0]["x"].as_f64().unwrap(), 0.6, 1e-12, "circle point 0 x"); close(fx["path"]["points"][0]["y"].as_f64().unwrap(), 0.6, 1e-12, "circle point 0 y");
    c(&mut s, "set_text_path", json!({"clip_id": id, "points": [[0.1, 0.5], [0.9, 0.5], [0.5, 0.2]], "animate": {"start": 0.5, "end": 1.5, "from": 0.2, "to": 0.8, "ease": "linear"}}));
    let fx = cj(&s, &id)["text_fx"].clone();
    assert_eq!(fx["path"]["closed"], false); assert_eq!(fx["path"]["points"].as_array().unwrap().len(), 3); assert_eq!(fx["path_flip"], false);
    let ks: Vec<(f64, f64)> = fx["path_start"]["keys"].as_array().unwrap().iter().map(|k| (k["t"].as_f64().unwrap(), k["v"].as_f64().unwrap())).collect();
    assert_eq!(ks, vec![(0.5, 0.2), (1.5, 0.8)]);
    // animate defaults: start 0 -> end = clip duration, from 0 -> to 1
    c(&mut s, "set_text_path", json!({"clip_id": id, "shape": "arc", "animate": {}}));
    let ks: Vec<(f64, f64)> = cj(&s, &id)["text_fx"]["path_start"]["keys"].as_array().unwrap().iter().map(|k| (k["t"].as_f64().unwrap(), k["v"].as_f64().unwrap())).collect();
    assert_eq!(ks, vec![(0.0, 0.0), (2.0, 1.0)]);
    c(&mut s, "set_text_path", json!({"clip_id": id, "remove": true}));
    assert!(cj(&s, &id)["text_fx"]["path"].is_null());
}

#[test] fn set_mesh_pin_keys_every_vertex() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
    let v0 = [0.0, 0.0, 64.0, 0.0, 0.0, 32.0, 64.0, 32.0]; let v1 = [2.0, 1.0, 60.0, 3.0, 4.0, 30.0, 62.0, 29.0];
    assert_eq!(c(&mut s, "set_mesh_pin", json!({"clip_id": id, "nx": 2, "ny": 2, "keys": [{"time": 0, "verts": v0}, {"time": 1, "verts": v1}]})), json!({"ok": true}));
    for (p, v) in [("pin_on", 1.0), ("mesh_nx", 2.0), ("mesh_ny", 2.0)] { assert_eq!(s.project.find_clip(&id).unwrap().props[p].base, v, "{p}"); }
    for k in 0..4 { assert_eq!(keys(&s, &id, &format!("mx{k}")), vec![(0.0, v0[k * 2]), (1.0, v1[k * 2])]); assert_eq!(keys(&s, &id, &format!("my{k}")), vec![(0.0, v0[k * 2 + 1]), (1.0, v1[k * 2 + 1])]); }
}

#[test] fn simulate_physics_bakes_free_flight_and_floor_rest() {
    let (mut s, t) = session(); let a = shape(&mut s, &t, 0.0, 1.0); let b = shape(&mut s, &t, 0.0, 1.0);
    // free flight: no gravity, no drag, no walls, no collisions -> x = x0 + vx t, rotation = spin t
    let r = c(&mut s, "simulate_physics", json!({"bodies": [{"clip_id": a, "x": -50, "vx": 100, "spin_deg_s": 90, "radius_px": 4}], "gravity": 0, "drag": 0, "walls": "", "collide": false, "start": 0.5}));
    assert_eq!(r, json!({"bodies": 1, "frames": 10}));
    let kx = keys(&s, &a, "x"); let kr = keys(&s, &a, "rotation"); assert_eq!(kx.len(), 10);
    for (i, (k, r)) in kx.iter().zip(&kr).enumerate() { let t = i as f64 / 10.0; close(k.0, 0.5 + t, 1e-9, "key time"); close(k.1, -50.0 + 100.0 * t, 1e-6, "x"); close(r.1, 90.0 * t, 1e-6, "rotation"); }
    // gravity onto the floor wall with restitution 0: both bodies end resting on it (y = half height - radius = 16 - 10)
    let r = c(&mut s, "simulate_physics", json!({"bodies": [{"clip_id": a, "x": -20, "radius_px": 10}, {"clip_id": b, "x": 20, "radius_px": 10, "mass": 2}], "gravity": 2000, "restitution": 0, "walls": "b", "duration": 1.5}));
    assert_eq!(r, json!({"bodies": 2, "frames": 15}));
    for id in [&a, &b] { let ky = keys(&s, id, "y"); assert_eq!(ky.len(), 15); close(ky[14].1, 6.0, 1e-9, "resting on the floor"); close(ky[0].1, 0.0, 0.0, "start"); }
    close(keys(&s, &a, "x")[14].1, -20.0, 1e-9, "no sideways motion");
}

fn preset(s: &mut Session, id: &str, a: Value) -> Value { let mut a = a; a["clip_id"] = json!(id); c(s, "apply_motion_preset", a) }
#[test] fn apply_motion_preset_names_prompts_and_composition() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 2.0);
    // a spelled name is normalised ("Fade-In" -> fade_in); opacity ramps 0..1 over 0.5 s from at=0.2
    let r = preset(&mut s, &id, json!({"preset": " Fade-In ", "at": 0.2, "duration": 0.5}));
    assert_eq!(r["applied"], json!([{"preset": "fade_in", "at": 0.2, "duration": 0.5, "frames": 6}]));
    let ko = keys(&s, &id, "opacity"); assert_eq!(ko.len(), 6);
    for (i, k) in ko.iter().enumerate() { close(k.0, 0.2 + i as f64 * 0.1, 1e-9, "t"); close(k.1, i as f64 * 0.2, 1e-9, "opacity"); }
    // a name that is not a preset is read as a prompt: only the first entrance is kept
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 2.0);
    assert_eq!(preset(&mut s, &id, json!({"preset": "bounce from above"}))["applied"][0]["preset"], "bounce_in");
    // a prompt composes presets one after the other
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 2.0);
    let r = preset(&mut s, &id, json!({"prompt": "fade in, then a slow zoom", "at": 0.2, "duration": 0.5, "intensity": 2}));
    assert_eq!(r["applied"], json!([{"preset": "fade_in", "at": 0.2, "duration": 0.5, "frames": 6}, {"preset": "drift_zoom", "at": 0.7, "duration": 0.5, "frames": 6}]));
    let ks = keys(&s, &id, "scale"); close(ks.last().unwrap().0, 1.2, 1e-9, "end"); close(ks.last().unwrap().1, 1.3, 1e-9, "drift zoom 1 + 0.15*k");
    // until_end: a looping preset runs to the end of the clip
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 2.0);
    let r = preset(&mut s, &id, json!({"preset": "float", "at": 0.5, "until_end": true, "distance_px": 100, "bpm": 90}));
    assert_eq!(r["applied"], json!([{"preset": "float", "at": 0.5, "duration": 1.5, "frames": 16}]));
    let ky = keys(&s, &id, "y"); close(ky[5].1, -100.0 * 0.04 * (2.0 * std::f64::consts::PI * 0.5 / 2.5).sin(), 1e-9, "float y at s=0.5");
}

#[test] fn export_variants_and_join_and_alpha_sequences() {
    let (mut s, t) = session(); c(&mut s, "add_clip", json!({"track_id": t, "source_type": "solid", "color": [0.2, 0.4, 0.6], "start": 0, "duration": 0.4}));
    let dir = std::env::temp_dir().join(format!("kimo_ch_export_{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
    let f = |n: &str| dir.join(n).to_str().unwrap().to_string();
    let r = c(&mut s, "export", json!({"path": f("full.mp4")}));
    assert_eq!(r["duration"], 0.4); assert!(r["bytes"].as_u64().unwrap() > 0); assert_eq!(r["bytes"].as_u64().unwrap(), std::fs::metadata(f("full.mp4")).unwrap().len());
    let r = c(&mut s, "export", json!({"path": f("pic.mp4"), "audio": false}));
    assert_eq!(r["duration"], 0.4);
    let a = c(&mut s, "export", json!({"path": f("a.mp4"), "audio": false, "end": 0.2})); let b = c(&mut s, "export", json!({"path": f("b.mp4"), "audio": false, "start": 0.2}));
    close(a["duration"].as_f64().unwrap() + b["duration"].as_f64().unwrap(), 0.4, 1e-9, "two halves");
    let r = c(&mut s, "join_export", json!({"path": f("j.mp4"), "parts": [f("a.mp4"), f("b.mp4")]}));
    assert_eq!(r["path"], f("j.mp4")); close(r["duration"].as_f64().unwrap(), 0.4, 0.05, "joined duration");
    let r = c(&mut s, "export_alpha", json!({"path": f("f_%03d.png"), "format": "png", "start": 0, "end": 0.2}));
    assert_eq!(r["path"], f("f_%03d.png"));
    let n = std::fs::read_dir(&dir).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_str().unwrap().starts_with("f_")).count();
    assert_eq!(n, 2, "0.2 s at 10 fps is two PNG frames");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test] fn export_gif_writes_a_gif_and_removes_its_temporary_mp4() {
    let (mut s, t) = session(); c(&mut s, "add_clip", json!({"track_id": t, "source_type": "solid", "color": [1, 0, 0], "start": 0, "duration": 0.3}));
    let dir = std::env::temp_dir().join(format!("kimo_ch_gif_{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
    let g = dir.join("a.gif").to_str().unwrap().to_string(); assert_eq!(c(&mut s, "export_gif", json!({"path": g, "fps": 5, "end": 0.2})), json!({"path": g}));
    assert!(std::fs::metadata(&g).unwrap().len() > 6); assert_eq!(&std::fs::read(&g).unwrap()[..3], b"GIF");
    assert!(!std::path::Path::new(&format!("{g}.tmp.mp4")).exists(), "temporary mp4 removed");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- mask stack, motion paths, pins, curves

fn mask(s: &mut Session, id: &str, a: Value) -> String { let mut a = a; a["clip_id"] = json!(id); c(s, "add_mask", a)["mask_id"].as_str().unwrap().to_string() }
fn layer(s: &Session, id: &str, mid: &str) -> Value { cj(s, id)["mask"]["layers"].as_array().unwrap().iter().find(|l| l["id"] == mid).cloned().unwrap() }

#[test] fn mask_layers_add_update_reorder_remove_and_list() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0); let other = shape(&mut s, &t, 0.0, 1.0);
    let a = mask(&mut s, &id, json!({"kind": "rect", "interp": "linear", "params": {"cx": 0.25, "label": "ignored"}}));
    let b = mask(&mut s, &id, json!({"kind": "path", "points": [{"x": 0, "y": 0}, {"x": 1, "y": 0}, {"x": 1, "y": 1}]}));
    assert_eq!(layer(&s, &id, &a)["props"]["cx"]["base"], 0.25, "a numeric param replaces the default, a non-number is skipped");
    c(&mut s, "set_mask_prop_key", json!({"clip_id": id, "mask_id": a, "property": "cx", "time": 0.5, "value": 0.75}));
    assert_eq!(c(&mut s, "update_mask", json!({"clip_id": id, "mask_id": a, "mode": "subtract", "invert": true, "enabled": false, "closed": false, "name": "hole", "ref_clip": other,
        "feather": 3, "expand": -2, "opacity": 0.5, "params": {"cx": 0.4, "tol": 0.2}, "fg": [[0.1, 0.2], [0.3]], "bg": [[0.9, 0.8]]})), json!({"ok": true}));
    let l = layer(&s, &id, &a);
    assert_eq!((l["mode"].as_str(), l["invert"].as_bool(), l["enabled"].as_bool(), l["closed"].as_bool(), l["name"].as_str()), (Some("subtract"), Some(true), Some(false), Some(false), Some("hole")));
    assert_eq!(l["strings"], json!({"interp": "linear", "ref_clip": other}));
    assert_eq!((l["props"]["feather"]["base"].as_f64(), l["props"]["expand"]["base"].as_f64(), l["props"]["opacity"]["base"].as_f64()), (Some(3.0), Some(-2.0), Some(0.5)));
    assert_eq!(l["props"]["cx"], json!({"base": 0.4, "keys": [], "expr": []}), "params reset the base and drop the keys");
    assert_eq!(l["props"]["tol"]["base"], 0.2); assert_eq!(l["fg"], json!([[0.1, 0.2]]), "malformed points are dropped"); assert_eq!(l["bg"], json!([[0.9, 0.8]]));
    c(&mut s, "update_mask", json!({"clip_id": id, "mask_id": b, "ngon": {"sides": 3, "r": 0.2}}));
    let l = layer(&s, &id, &b); assert_eq!(l["points"].as_array().unwrap().len(), 3); assert_eq!(l["closed"], true);
    c(&mut s, "update_mask", json!({"clip_id": id, "mask_id": b, "ngon": {}}));
    assert_eq!(layer(&s, &id, &b)["points"].as_array().unwrap().len(), 5, "default ngon has 5 sides");
    c(&mut s, "reorder_mask", json!({"clip_id": id, "mask_id": a, "index": 9}));
    let order: Vec<String> = cj(&s, &id)["mask"]["layers"].as_array().unwrap().iter().map(|l| l["id"].as_str().unwrap().to_string()).collect(); assert_eq!(order, vec![b.clone(), a.clone()]);
    c(&mut s, "remove_mask_layer", json!({"clip_id": id, "mask_id": b}));
    // a legacy single mask and a track matte both show in list_masks
    c(&mut s, "set_mask", json!({"clip_id": id, "shape": "ellipse"}));
    c(&mut s, "set_track_matte", json!({"clip_id": id, "matte_clip_id": other, "mode": "luma_inverted", "luma_space": "display", "hide_source": false}));
    let r = c(&mut s, "list_masks", json!({"clip_id": id}));
    assert_eq!(r["legacy_shape"], "ellipse"); assert_eq!(r["matte"], json!({"source": other, "mode": "inv_luma", "hide_source": false, "luma_space": "display"}));
    assert_eq!(r["layers"].as_array().unwrap().len(), 1, "set_mask keeps the layer stack");
    let r = c(&mut s, "list_masks", json!({"clip_id": other})); assert_eq!(r, json!({"layers": [], "legacy_shape": null, "matte": null}));
    c(&mut s, "remove_track_matte", json!({"clip_id": id})); assert!(cj(&s, &id)["mask"]["matte"].is_null());
}

#[test] fn render_mask_writes_the_matte_png() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0); let m = mask(&mut s, &id, json!({"kind": "rect", "params": {"cx": 0.25, "w": 0.5, "h": 1.0}}));
    let p = std::env::temp_dir().join(format!("kimo_ch_mask_{}.png", std::process::id())); let ps = p.to_str().unwrap();
    assert_eq!(c(&mut s, "render_mask", json!({"clip_id": id, "mask_id": m, "time": 0.5, "path": ps, "view": "matte"})), json!({"path": ps}));
    let g = gray(ps, 64, 32);
    assert!(g[16 * 64 + 8] > 200 && g[16 * 64 + 56] < 50, "left half selected, right half not");
    let _ = std::fs::remove_file(&p);
}

#[test] fn motion_path_points_shapes_bezier_and_animation() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 2.0);
    c(&mut s, "set_motion_path", json!({"clip_id": id, "points": [[0.1, 0.2], [0.9, 0.8]], "smooth": false, "closed": true, "auto_orient": true, "orient_offset": 90, "constant_speed": false, "orient_smooth": 0.5}));
    let mp = cj(&s, &id)["motion_path"].clone();
    assert_eq!((mp["closed"].as_bool(), mp["auto_orient"].as_bool(), mp["orient_offset"].as_f64(), mp["constant_speed"].as_bool(), mp["orient_smooth"].as_f64()), (Some(true), Some(true), Some(90.0), Some(false), Some(0.5)));
    assert_eq!((mp["points"][1]["x"].as_f64(), mp["points"][1]["y"].as_f64(), mp["points"][1]["ix"].as_f64()), (Some(0.9), Some(0.8), Some(0.0)), "smooth=false keeps corner points");
    c(&mut s, "set_motion_path", json!({"clip_id": id, "points": [[0.1, 0.5], [0.5, 0.5], [0.9, 0.5]], "tension": 1}));
    let mp = cj(&s, &id)["motion_path"].clone(); assert_eq!(mp["constant_speed"], true);
    assert!(mp["points"][1]["ox"].as_f64().unwrap() > 0.0, "smooth through-points get tangents");
    c(&mut s, "set_motion_path", json!({"clip_id": id, "points": [{"x": 0, "y": 0, "ox": 0.1}, {"x": 1, "y": 1}], "animate": {"from": 0.25, "ease": "linear"}}));
    assert_eq!(cj(&s, &id)["motion_path"]["points"][0]["ox"], 0.1);
    assert_eq!(keys(&s, &id, "path"), vec![(0.0, 0.25), (2.0, 1.0)]);
    c(&mut s, "set_motion_path", json!({"clip_id": id, "shape": "figure8", "cx": 0.4}));
    assert_eq!(cj(&s, &id)["motion_path"]["closed"], true);
    c(&mut s, "remove_motion_path", json!({"clip_id": id}));
    assert!(cj(&s, &id)["motion_path"].is_null()); assert!(s.project.find_clip(&id).unwrap().props.get("path").is_none());
}

#[test] fn pin_layer_quads_batches_and_removal() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
    let q0 = [0.0, 0.0, 64.0, 0.0, 64.0, 32.0, 0.0, 32.0]; let q1 = [4.0, 2.0, 60.0, 1.0, 62.0, 30.0, 3.0, 31.0];
    assert_eq!(c(&mut s, "pin_layer", json!({"clip_id": id, "time": 0.25, "quad": q0})), json!({"ok": true, "keys": 1}));
    assert_eq!(c(&mut s, "pin_layer", json!({"clip_id": id, "keys": [{"time": 0.5, "quad": q1}, {"time": 0.75, "quad": q0}]})), json!({"ok": true, "keys": 2}));
    assert_eq!(keys(&s, &id, "pin2x"), vec![(0.25, 64.0), (0.5, 62.0), (0.75, 64.0)]); assert_eq!(keys(&s, &id, "pin3y"), vec![(0.25, 32.0), (0.5, 31.0), (0.75, 32.0)]);
    assert_eq!(s.project.find_clip(&id).unwrap().props["pin_on"].base, 1.0);
    c(&mut s, "pin_layer", json!({"clip_id": id, "off": true}));
    assert!(s.project.find_clip(&id).unwrap().props.get("pin_on").is_none()); assert_eq!(keys(&s, &id, "pin2x").len(), 3, "off keeps the keys");
}

#[test] fn copy_curve_remaps_time_and_value_and_simplify_keeps_a_straight_line() {
    let (mut s, t) = session(); let a = shape(&mut s, &t, 0.0, 2.0); let b = shape(&mut s, &t, 0.0, 2.0);
    for (tt, v) in [(0.0, 0.0), (1.0, 10.0)] { c(&mut s, "set_keyframe", json!({"clip_id": a, "property": "x", "time": tt, "value": v, "ease": "linear"})); }
    c(&mut s, "set_property", json!({"clip_id": a, "property": "x", "value": 2}));
    assert_eq!(c(&mut s, "copy_curve", json!({"from_clip": a, "from_property": "x", "to_clip": b, "to_property": "y", "time_offset": 0.5, "time_scale": 2, "value_scale": 3, "value_offset": 1, "reverse": true, "invert": true})), json!({"ok": true, "keys": 2}));
    // reverse: t -> 1 - t, then *2 + 0.5; invert then scale/offset: v -> -v * 3 + 1
    assert_eq!(keys(&s, &b, "y"), vec![(0.5, -29.0), (2.5, 1.0)]);
    assert_eq!(s.project.find_clip(&b).unwrap().props["y"].base, -2.0 * 3.0 + 1.0);
    c(&mut s, "copy_curve", json!({"from_clip": a, "from_property": "x", "to_clip": b, "to_property": "rotation"}));
    assert_eq!(keys(&s, &b, "rotation"), vec![(0.0, 0.0), (1.0, 10.0)]);
    assert_eq!(c(&mut s, "simplify_curve", json!({"clip_id": a, "property": "x"})), json!({"before": 2, "after": 2, "max_error": 0.0}));
}

// ---------------------------------------------------------------- groups, camera shake, text animator presets

fn clip_on(s: &Session, track: usize, id: &str) -> bool { s.project.tracks[track].clips.iter().any(|c| c.id == id) }

#[test] fn group_and_ungroup_keep_timeline_times() {
    let (mut s, t) = session(); let t2 = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string();
    let a = shape(&mut s, &t, 1.0, 1.0); let b = shape(&mut s, &t2, 2.0, 1.5); let lone = shape(&mut s, &t, 5.0, 1.0);
    let r = c(&mut s, "group_clips", json!({"clip_ids": [a, b, 7], "track_id": t}));
    let g = r["clip_id"].as_str().unwrap().to_string(); assert_eq!((r["start"].as_f64(), r["duration"].as_f64()), (Some(1.0), Some(2.5)));
    assert!(clip_on(&s, 0, &g), "placed on the given track"); assert!(s.project.find_clip(&a).is_none());
    let src = cj(&s, &g)["source"].clone(); assert_eq!(src["type"], "precomp"); assert_eq!(src["tracks"].as_array().unwrap().len(), 2);
    assert_eq!((src["tracks"][0]["clips"][0]["start"].as_f64(), src["tracks"][1]["clips"][0]["start"].as_f64()), (Some(0.0), Some(1.0)), "local clock starts at the group start");
    assert_eq!(c(&mut s, "ungroup_clip", json!({"clip_id": g})), json!({"ungrouped": 2}));
    assert_eq!((s.project.find_clip(&a).unwrap().start, s.project.find_clip(&b).unwrap().start), (1.0, 2.0));
    // without track_id the group lands on the topmost source track
    let r = c(&mut s, "group_clips", json!({"clip_ids": [lone]})); assert!(clip_on(&s, 0, r["clip_id"].as_str().unwrap()));
}

#[test] fn camera_shake_snaps_hits_and_writes_the_kick_profile() {
    let (mut s, t) = session();
    let id = c(&mut s, "add_camera_shake", json!({"track_id": t, "start": 0, "duration": 1, "hits": [0.2, 0.31, "x"], "amplitude": 10, "zoom": 0.05}))["clip_id"].as_str().unwrap().to_string();
    let cl = cj(&s, &id); let fx = &cl["effects"][0]; assert_eq!(cl["source"], json!({"type": "adjustment"}));
    assert_eq!(fx["kind"], "impact_shake");
    assert_eq!(fx["strings"], json!({"hits": "0.2,0.4", "profile": "-19,-10;-11,-6;-4,-2", "fps": "10"}), "a hit snaps up to the frame it lands on");
    let p: Vec<(&str, f64)> = fx["params"].as_object().unwrap().iter().map(|(k, v)| (k.as_str(), v["base"].as_f64().unwrap())).collect();
    assert_eq!(p, vec![("amount", 10.0), ("amount_y", 9.0), ("decay", 27.0), ("frequency", 12.5), ("rotation", 0.0), ("scale", 1.0), ("zoom", 0.05)]);
    let id = c(&mut s, "add_camera_shake", json!({"track_id": t, "start": 0.5, "duration": 1, "hits": [0.7], "profile": [[1, 2], [3]], "scale": 2}))["clip_id"].as_str().unwrap().to_string();
    let fx = cj(&s, &id)["effects"][0].clone(); assert_eq!(fx["strings"]["profile"], "1,2"); assert_eq!(fx["params"]["scale"]["base"], 2.0);
    close(fx["strings"]["hits"].as_str().unwrap().parse::<f64>().unwrap(), 0.2, 1e-9, "hit relative to the layer start");
    let id = c(&mut s, "add_camera_shake", json!({"track_id": t, "start": 0, "duration": 1, "hits": [0.5], "profile": "spring"}))["clip_id"].as_str().unwrap().to_string();
    let fx = cj(&s, &id)["effects"][0].clone(); assert_eq!(fx["strings"], json!({"hits": "0.5"}), "spring: no kick table"); assert!(fx["params"].get("scale").is_none());
}

fn anim(s: &mut Session, id: &str, a: Value) -> Value { let mut a = a; a["clip_id"] = json!(id); let r = c(s, "add_text_animator", a); let n = r["animator_id"].as_str().unwrap()[2..].parse::<usize>().unwrap(); cj(s, id)["text_fx"]["animators"][n - 1].clone() }
fn pkeys(p: &Value) -> Vec<(f64, f64)> { p["keys"].as_array().unwrap().iter().map(|k| (k["t"].as_f64().unwrap(), k["v"].as_f64().unwrap())).collect() }
#[test] fn text_animator_presets_key_the_selector_offset() {
    let (mut s, t) = session(); let id = text(&mut s, &t);
    let a = anim(&mut s, &id, json!({"preset": "rise", "delay": 0.2, "duration": 0.5, "random": true, "seed": 4}));
    assert_eq!((a["y"]["base"].as_f64(), a["opacity"]["base"].as_f64(), a["unit"].as_str(), a["random"].as_bool(), a["seed"].as_u64()), (Some(70.0), Some(0.0), Some("char"), Some(true), Some(4)));
    assert_eq!(pkeys(&a["offset"]), vec![(0.2, 0.0), (0.7, 1.6)], "in: 0 -> 1 + 2*smoothness");
    let a = anim(&mut s, &id, json!({"preset": "drop", "out": true, "duration": 1, "smoothness": 0}));
    assert_eq!(a["y"]["base"], -90.0); assert_eq!(pkeys(&a["offset"]), vec![(0.0, -1.1), (1.0, 0.0)], "out: -(1 + 2*0.05) -> 0");
    let a = anim(&mut s, &id, json!({"preset": "pop"})); assert_eq!((a["scale"]["base"].as_f64(), a["unit"].as_str()), (Some(0.0), Some("word")));
    let a = anim(&mut s, &id, json!({"preset": "blur_in"})); assert_eq!(a["blur"]["base"], 14.0);
    let a = anim(&mut s, &id, json!({"preset": "cascade_rotate"})); assert_eq!((a["rotation"]["base"].as_f64(), a["y"]["base"].as_f64()), (Some(-35.0), Some(50.0)));
    let a = anim(&mut s, &id, json!({"preset": "tracking_in"})); assert_eq!((a["tracking"]["base"].as_f64(), a["unit"].as_str()), (Some(40.0), Some("word")));
    let a = anim(&mut s, &id, json!({"preset": "wave", "duration": 2}));
    assert_eq!((a["shape"].as_str(), a["end"]["base"].as_f64(), a["smoothness"]["base"].as_f64()), (Some("triangle"), Some(0.35), Some(1.0))); assert_eq!(pkeys(&a["offset"]), vec![(0.0, -0.35), (2.0, 1.35)]);
    let a = anim(&mut s, &id, json!({"preset": "typewriter", "duration": 1})); assert_eq!(a["smoothness"]["base"], 0.0); assert_eq!(pkeys(&a["offset"]), vec![(0.0, 0.0), (1.0, 1.0)]);
    assert_eq!(cj(&s, &id)["text_fx"]["animators"].as_array().unwrap().len(), 8);
}

// ---------------------------------------------------------------- media swaps and timeline edits

fn rand_tag() -> u128 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() }
/// Assets of known length (files absent): a1 2 s, a5 5 s, a0 length unknown (a still); clip9 is a 2-second media clip of a1 with speed 0 (corrupt).
fn media_session() -> Session {
    let p = std::env::temp_dir().join(format!("kimo_ch_media_{}_{}.kimo", std::process::id(), rand_tag())); let ps = p.to_str().unwrap();
    let asset = |id: &str, d: f64| json!({"id": id, "path": format!("/nonexistent/{id}.mkv"), "width": 64, "height": 32, "duration": d, "fps": 10.0});
    std::fs::write(&p, json!({"name": "t", "markers": [], "width": 64, "height": 32, "fps": 10, "assets": {"a1": asset("a1", 2.0), "a5": asset("a5", 5.0), "a0": asset("a0", 0.0)},
        "tracks": [{"id": "track1", "clips": [{"id": "clip9", "source": {"type": "media", "asset": "a1"}, "start": 0, "duration": 2, "in_point": 0, "speed": 0, "effects": [], "blend": "normal", "props": {}}]}]}).to_string()).unwrap();
    let mut s = Session::new(); c(&mut s, "load_project", json!({"path": ps})); let _ = std::fs::remove_file(&p); s
}

#[test] fn replace_clip_media_reports_length_differences_and_can_match_them() {
    let mut s = media_session();
    // speed 0 is treated as 1: the 5 s file from in_point 1 fills 4 s; keep leaves 2 s of it unused
    let r = c(&mut s, "replace_clip_media", json!({"clip_id": "clip9", "asset_id": "a5", "in_point": 1}));
    assert_eq!((r["duration"].as_f64(), r["in_point"].as_f64(), r["old_file_s"].as_f64(), r["new_file_s"].as_f64()), (Some(2.0), Some(1.0), Some(2.0), Some(5.0)));
    assert_eq!(r["length_note"], "the new file is 5.00 s (the old one was 2.00 s); the clip kept its 2.00 s, so 2.00 s of the new file is not used. Call again with length: \"match\" to use all of it");
    let r = c(&mut s, "replace_clip_media", json!({"clip_id": "clip9", "asset_id": "a1", "in_point": 1.5}));
    assert_eq!(r["length_note"], "the new file is 2.00 s, shorter than this 2.00 s clip needs from in_point 1.50: the last 1.50 s would have no picture. Call again with length: \"match\" to end the clip with the file");
    let r = c(&mut s, "replace_clip_media", json!({"clip_id": "clip9", "asset_id": "a5", "in_point": 0, "length": "match"}));
    assert_eq!(r["duration"], 5.0); assert_eq!(r["length_note"], "the new file is 5.00 s (the old one was 2.00 s); the clip now runs 5.00 s (was 2.00 s)");
    let r = c(&mut s, "replace_clip_media", json!({"clip_id": "clip9", "asset_id": "a0", "length": "match"}));
    assert_eq!(r["duration"], 5.0, "unknown length: the duration is kept"); assert!(r.get("length_note").is_none());
}

fn starts(s: &Session, t: usize) -> Vec<(String, f64, f64)> { let mut v: Vec<(String, f64, f64)> = s.project.tracks[t].clips.iter().map(|c| (c.id.clone(), c.start, c.duration)).collect(); v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap()); v }

#[test] fn ripple_trim_slide_and_rate_stretch() {
    let (mut s, t) = session(); let a = shape(&mut s, &t, 0.0, 1.0); let b = shape(&mut s, &t, 1.0, 1.0); let x = shape(&mut s, &t, 2.0, 1.0);
    assert_eq!(c(&mut s, "ripple_trim", json!({"clip_id": b, "edge": "head", "delta": 0.5})), json!({"duration": 1.5}));
    assert_eq!(starts(&s, 0), vec![(a.clone(), 0.0, 1.0), (b.clone(), 1.0, 1.5), (x.clone(), 2.5, 1.0)], "later clips follow the tail");
    assert_eq!(cj(&s, &b)["in_point"], 0.0, "head growth clamps the in-point at 0");
    // the first clip has no previous neighbour, the last no next one
    c(&mut s, "slide_clip", json!({"clip_id": a, "delta": 0.2})); c(&mut s, "slide_clip", json!({"clip_id": x, "delta": 0.3}));
    assert_eq!(starts(&s, 0), vec![(a.clone(), 0.2, 1.0), (b.clone(), 1.2, 1.6), (x.clone(), 2.8, 1.0)], "the neighbours absorb each slide");
    assert_eq!(c(&mut s, "rate_stretch", json!({"clip_id": a, "duration": 2})), json!({"speed": 0.5}));
}

#[test] fn swaps_duplicates_distribution_and_silence_cuts() {
    let (mut s, t) = session(); let a = shape(&mut s, &t, 0.0, 1.0); let b = shape(&mut s, &t, 3.0, 1.0);
    let t2 = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string(); let o = shape(&mut s, &t2, 0.0, 1.0);
    assert_eq!(c(&mut s, "swap_clips", json!({"clip_a": b, "clip_b": a})), json!({"first": b, "second": a}));
    assert_eq!(starts(&s, 0), vec![(b.clone(), 0.0, 1.0), (a.clone(), 3.0, 1.0)], "same span, same gap");
    let d = c(&mut s, "duplicate_clip", json!({"clip_id": a, "start": 5, "track_id": t2}))["clip_id"].as_str().unwrap().to_string();
    assert_eq!(starts(&s, 1), vec![(o.clone(), 0.0, 1.0), (d.clone(), 5.0, 1.0)]);
    assert_eq!(c(&mut s, "distribute_clips", json!({"track_id": t, "gap": 0.5, "start": 1})), json!({"moved": 2}));
    assert_eq!(starts(&s, 0), vec![(b.clone(), 1.0, 1.0), (a.clone(), 2.5, 1.0)]);
    // silence cuts: well-formed ranges are ripple-extracted, latest first; malformed ones are skipped
    let r = c(&mut s, "remove_silence_cuts", json!({"track_id": t, "ranges": [[1.5, 2.6], "x", [3], [1.2, 1.3]]}));
    assert_eq!(r["removed"], 2); let cl = r["clips"].as_array().unwrap();
    let mut spans: Vec<(f64, f64)> = cl.iter().map(|x| ((x["start"].as_f64().unwrap() * 1e6).round() / 1e6, (x["end"].as_f64().unwrap() * 1e6).round() / 1e6)).collect();
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    assert_eq!(spans, vec![(1.0, 1.2), (1.2, 1.4), (1.4, 2.3)]);
}

#[test] fn markers_snapping_edit_points_tracks_and_razor() {
    let (mut s, t) = session(); let a = shape(&mut s, &t, 1.0, 1.0);
    let t2 = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string(); shape(&mut s, &t2, 0.5, 2.0);
    let m = c(&mut s, "add_marker", json!({"time": 3.05, "name": "beat"}))["marker_id"].as_str().unwrap().to_string();
    assert_eq!(c(&mut s, "snap_time", json!({"time": 3.0})), json!({"snapped": true, "time": 3.05}));
    assert_eq!(c(&mut s, "snap_time", json!({"time": 1.96, "tolerance": 0.05})), json!({"snapped": true, "time": 2.0}));
    assert_eq!(c(&mut s, "snap_time", json!({"time": 5.0})), json!({"snapped": false, "time": 5.0}));
    assert_eq!(c(&mut s, "list_edit_points", json!({})), json!({"points": [0.5, 1.0, 2.0, 2.5]}));
    c(&mut s, "remove_marker", json!({"marker_id": m}));
    c(&mut s, "set_track", json!({"track_id": t2, "locked": true}));
    assert_eq!(c(&mut s, "split_all_at", json!({"at": 1.5})), json!({"split": 1}), "the locked track is skipped");
    assert_eq!(starts(&s, 0).len(), 2); assert_eq!(starts(&s, 0)[0], (a.clone(), 1.0, 0.5));
    assert_eq!(starts(&s, 1).len(), 1);
    c(&mut s, "set_track", json!({"track_id": t2, "locked": false})); c(&mut s, "delete_track", json!({"track_id": t2})); assert_eq!(s.project.tracks.len(), 1);
}

#[test] fn cutting_a_remapped_clip_keeps_its_source_curve() {
    let mut s = media_session();
    c(&mut s, "set_time_remap", json!({"clip_id": "clip9", "keys": [[0, 0], [2, 1]]}));
    assert_eq!(c(&mut s, "split_all_at", json!({"at": 1.0})), json!({"split": 1}));
    let right = s.project.tracks[0].clips.iter().find(|c| c.id != "clip9").unwrap();
    assert_eq!((right.start, right.in_point), (1.0, 0.0)); assert_eq!(serde_json::to_value(right).unwrap()["time_remap"]["offset"], 1.0, "the remap curve is read 1 s further in");
}

// ---------------------------------------------------------------- typed modifiers, shape modifiers, layer styles

fn kinds(r: &Value) -> Vec<String> { r["modifiers"].as_array().unwrap().iter().map(|m| m.as_str().map(String::from).unwrap_or_else(|| m["kind"].as_str().unwrap().to_string())).collect() }
#[test] fn typed_modifiers_add_replace_list_and_remove() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 2.0);
    let m = |a: Value| { let mut a = a; a["clip_id"] = json!(id); a };
    for (tt, v) in [(0.0, 0.0), (1.0, 10.0)] { c(&mut s, "set_keyframe", json!({"clip_id": id, "property": "x", "time": tt, "value": v})); }
    assert_eq!(kinds(&c(&mut s, "add_modifier", m(json!({"property": "x", "kind": "wiggle", "seed": 7})))), vec!["wiggle"]);
    assert_eq!(kinds(&c(&mut s, "add_modifier", m(json!({"property": "x", "kind": "wiggle", "amp": 3})))), vec!["wiggle"], "same kind replaces");
    assert_eq!(kinds(&c(&mut s, "add_modifier", m(json!({"property": "x", "kind": "loop", "mode": "pingpong", "direction": "both"})))), vec!["wiggle", "loop", "loop"]);
    assert_eq!(kinds(&c(&mut s, "add_modifier", m(json!({"property": "x", "kind": "loop", "direction": "in"})))), vec!["wiggle", "loop"]);
    assert_eq!(kinds(&c(&mut s, "add_modifier", m(json!({"property": "x", "kind": "bounce"})))), vec!["wiggle", "loop", "bounce"]);
    assert_eq!(kinds(&c(&mut s, "add_modifier", m(json!({"property": "x", "kind": "overshoot", "amp": 0.5})))), vec!["wiggle", "loop", "bounce", "overshoot"]);
    // the default wiggle seed is the FNV-1a hash of "<clip>/<property>"
    let r = c(&mut s, "add_modifier", m(json!({"property": "y", "kind": "wiggle"}))); assert_eq!(kinds(&r), vec!["wiggle"]);
    let l = c(&mut s, "list_modifiers", m(json!({"property": "y"})));
    assert_eq!(l["modifiers"][0]["op"], serde_json::to_value(kimo_core::keyframe::Op::Wiggle { freq: 2.0, amp: 10.0, octaves: 1, seed: kimo_core::commands::fnv64(format!("{id}/y").as_bytes()), shift: 0.0 }).unwrap());
    assert_eq!(l["keyframes"], 0);
    let l = c(&mut s, "list_modifiers", m(json!({"property": "x"}))); assert_eq!(kinds(&l), vec!["wiggle", "loop", "bounce", "overshoot"]); assert_eq!(l["keyframes"], 2);
    let fx = c(&mut s, "add_effect", json!({"clip_id": id, "kind": "blur", "params": {"radius": 2}}))["effect_id"].as_str().unwrap().to_string();
    assert_eq!(kinds(&c(&mut s, "add_modifier", m(json!({"property": "radius", "effect_id": fx, "kind": "wiggle"})))), vec!["wiggle"]);
    assert_eq!(c(&mut s, "list_modifiers", m(json!({"property": "radius", "effect_id": fx})))["modifiers"].as_array().unwrap().len(), 1);
    assert_eq!(c(&mut s, "remove_modifier", m(json!({"property": "x", "kind": "loop"}))), json!({"removed": 1, "remaining": 3}));
    assert_eq!(c(&mut s, "remove_modifier", m(json!({"property": "x"}))), json!({"removed": 3, "remaining": 0}));
}

#[test] fn shape_modifiers_add_and_update_in_place() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
    let l = c(&mut s, "list_shape_modifiers", json!({})); assert_eq!(l["modifiers"].as_array().unwrap().len(), kimo_core::shapemods::KINDS.len());
    let r = c(&mut s, "add_shape_modifier", json!({"clip_id": id, "kind": "trim", "params": {"end": 0.5}})); assert_eq!(r["effect_id"], "shape_trim-1");
    let r = c(&mut s, "add_shape_modifier", json!({"clip_id": id, "kind": "merge", "strings": {"shape": "path", "d": "M0 0 L10 0 L10 10 Z", "mode": "union"}})); assert_eq!(r["effect_id"], "shape_merge-2");
    c(&mut s, "add_shape_modifier", json!({"clip_id": id, "kind": "repeater", "params": {"copies": 4}}));
    c(&mut s, "add_shape_modifier", json!({"clip_id": id, "kind": "trim", "effect_id": "shape_trim-1", "params": {"start": 0.25}}));
    let e = cj(&s, &id)["effects"][0].clone(); assert_eq!((e["params"]["start"]["base"].as_f64(), e["params"]["end"]["base"].as_f64()), (Some(0.25), Some(1.0)), "updating resets unspecified params to defaults");
    assert_eq!(cj(&s, &id)["effects"].as_array().unwrap().len(), 3);
    assert_eq!(c(&mut s, "remove_shape_modifier", json!({"clip_id": id, "effect_id": "shape_trim-1"})), json!({"removed": "shape_trim-1"}));
}

#[test] fn layer_styles_one_per_kind_unless_an_id_is_given() {
    let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
    assert_eq!(c(&mut s, "list_layer_styles", json!({}))["styles"].as_array().unwrap().len(), kimo_core::layerstyles::STYLES.len());
    assert_eq!(c(&mut s, "set_layer_style", json!({"clip_id": id, "style": "stroke", "strings": {"position": "outside"}, "params": {"width": 2}})), json!({"effect_id": "ls_stroke", "kind": "ls_stroke"}));
    c(&mut s, "set_layer_style", json!({"clip_id": id, "style": "stroke", "params": {"width": 6}}));
    let fx = cj(&s, &id)["effects"].clone(); assert_eq!(fx.as_array().unwrap().len(), 1, "the same style kind is updated");
    assert_eq!(fx[0]["params"]["width"]["base"], 6.0); assert_eq!(fx[0]["strings"], json!({}));
    assert_eq!(c(&mut s, "set_layer_style", json!({"clip_id": id, "style": "bevel", "effect_id": "b2"}))["effect_id"], "b2");
    assert_eq!(c(&mut s, "remove_layer_style", json!({"clip_id": id, "effect_id": "b2"})), json!({"removed": "b2"}));
}

// ---------------------------------------------------------------- trackers on a 160x120, 10 fps, 1 s plate: a static sinusoidal texture that slides 1 px left per frame

fn plate() -> &'static std::path::PathBuf {
    static P: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let p = std::env::temp_dir().join(format!("kimo-ch-plate-{}.mkv", std::process::id()));
        assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", "nullsrc=s=320x240:r=10:d=1,geq=lum='128+60*sin(X/5)*cos(Y/7)+60*sin((X+Y)/11)':cb=128:cr=128,crop=160:120:x='40+n':y=40,format=rgb24", "-c:v", "ffv1", p.to_str().unwrap()]).status().unwrap().success());
        p
    })
}
fn plate_session() -> (Session, String, String) {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 160, "height": 120, "fps": 10}));
    let a = c(&mut s, "import_media", json!({"path": plate()}))["asset_id"].as_str().unwrap().to_string();
    let t = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string();
    let m = c(&mut s, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0, "duration": 1}))["clip_id"].as_str().unwrap().to_string();
    let g = shape(&mut s, &t, 0.0, 1.0);
    (s, m, g)
}

#[test] fn motion_extract_and_motion_diff_on_a_sliding_plate() {
    let (mut s, m, g) = plate_session();
    let r = c(&mut s, "motion_extract", json!({"clip_id": m, "target_clip_id": g, "return_curves": true}));
    assert_eq!(r["frames"], 10); assert_eq!(r["curves"].as_array().unwrap().len(), 10);
    let (lo, hi) = (r["range"]["dx"][0].as_f64().unwrap(), r["range"]["dx"][1].as_f64().unwrap());
    assert!((-11.0..=-7.0).contains(&lo) && hi.abs() < 0.6, "content slides ~9 px left over 9 frames: {r}");
    let kx = keys(&s, &g, "x"); assert_eq!(kx.len(), 10); close(kx[9].1, lo, 0.6, "the target follows the footage");
    c(&mut s, "motion_extract", json!({"clip_id": m, "target_clip_id": g, "invert": true, "gain": 0.5, "start": 2, "end": 5}));
    let kx = keys(&s, &g, "x"); assert_eq!(kx.len(), 4); close(kx[0].0, 0.2, 1e-9, "keys start at the first measured frame"); assert!(kx[3].1 > 0.0, "inverted half-gain moves the other way");
    let d = c(&mut s, "motion_diff", json!({"clip_a": m, "clip_b": m}));
    assert_eq!(d["frames"], 10); assert_eq!(d["worst_translation_px"], 0.0);
    for ch in ["dx_px", "dy_px", "scale", "rot_deg"] { assert_eq!(d["channels"][ch]["rms_diff"], 0.0, "{ch}"); }
    close(d["channels"]["dx_px"]["correlation"].as_f64().unwrap(), 1.0, 1e-6, "identical motion");
}

#[test] fn point_tracking_and_classic_stabilize() {
    let (mut s, m, g) = plate_session();
    let r = c(&mut s, "track_point", json!({"clip_id": m, "target_clip_id": g, "x": 0.5, "y": 0.5, "patch": 16, "search": 6, "step": 3}));
    assert_eq!(r["frames"], 10); close(r["end_px"][0].as_f64().unwrap(), 80.0 - 9.0, 1.5, "the point slides 9 px left");
    let kx = keys(&s, &g, "x"); assert_eq!(kx.len(), 4, "every 3rd frame plus the last"); assert!(keys(&s, &g, "rotation").len() == 4, "rigid tracks rotation too");
    let (mut s, m, g) = plate_session();
    c(&mut s, "track_point", json!({"clip_id": m, "target_clip_id": g, "x": 0.5, "y": 0.5, "patch": 16, "search": 6, "rigid": false}));
    assert_eq!(keys(&s, &g, "x").len(), 10); assert!(keys(&s, &g, "rotation").is_empty(), "translation only");
    let r = c(&mut s, "stabilize", json!({"clip_id": m, "smooth": 0, "lock": true}));
    assert_eq!(r["frames"], 10); assert!(r["max_translation_px"].as_f64().unwrap() > 5.0, "a locked camera undoes the 9 px slide: {r}");
    assert_eq!(keys(&s, &m, "x").len(), 10);
}

#[test] fn planar_and_mesh_tracking_pin_targets() {
    let (mut s, m, g) = plate_session();
    let q = [0.25, 0.25, 0.75, 0.25, 0.75, 0.75, 0.25, 0.75];
    let r = c(&mut s, "track_planar", json!({"clip_id": m, "corners": q, "target_clip_id": g, "expand": 0.25, "light": 0.5, "return_quads": true, "occluder_matte": true, "occluder_mode": "adaptive", "context_corners": [0.1, 0.1, 0.9, 0.1, 0.9, 0.9, 0.1, 0.9]}));
    assert_eq!(r["frames"], 10); assert_eq!(r["quads"].as_array().unwrap().len(), 10); assert_eq!(r["quads"][0].as_array().unwrap().len(), 8);
    for k in ["pin0x", "pin0y", "pin3x", "pin3y", "pin_gain"] { assert_eq!(keys(&s, &g, k).len(), 10, "{k}"); }
    assert_eq!(s.project.find_clip(&g).unwrap().props["pin_light"].base, 0.5);
    assert!(s.project.occluders.contains_key(&g), "the adaptive occluder is registered for the target");
    let x0 = keys(&s, &g, "pin0x")[0].1; close(x0, 40.0 - 0.25 * 80.0, 1.0, "expand 0.25 grows the quad by a quarter of its 80 px side");
    let r = c(&mut s, "track_planar", json!({"clip_id": m, "corners": q, "start": 2, "end": 4, "scale_damp": 0.5, "shape_lock": 1, "reanchor": true, "levels": 2}));
    assert_eq!(r["frames"], 3);
    let r = c(&mut s, "track_mesh", json!({"clip_id": m, "corners": q, "nx": 2, "ny": 2, "target_clip_id": g, "return_verts": true, "verts0": [0.25, 0.25, 0.75, 0.25, 0.25, 0.75, 0.75, 0.75], "end": 4}));
    assert_eq!(r["frames"], 5); assert_eq!(r["verts"][0].as_array().unwrap().len(), 8);
    assert_eq!(s.project.find_clip(&g).unwrap().props["mesh_nx"].base, 2.0); assert_eq!(keys(&s, &g, "mx3").len(), 5);
    close(keys(&s, &g, "mx0")[0].1, 40.0, 0.5, "frame 0 starts at verts0");
}

// ---------------------------------------------------------------- look: clip outlines, kinds, safe areas, scopes

#[test] fn look_outlines_each_clip_kind_and_flags_titles_outside_safe() {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 320, "height": 180, "fps": 10}));
    let tr: Vec<String> = (0..3).map(|_| c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string()).collect();
    shape(&mut s, &tr[0], 0.0, 2.0); // lowest: left out by max_clips
    let tx = c(&mut s, "add_text", json!({"track_id": tr[1], "text": "WIDE TITLE", "size": 30, "x": 120, "start": 0, "duration": 2}))["clip_id"].as_str().unwrap().to_string();
    let mv = shape(&mut s, &tr[1], 0.0, 2.0); for (tt, v) in [(0.0, -50.0), (1.0, 50.0)] { c(&mut s, "set_keyframe", json!({"clip_id": mv, "property": "x", "time": tt, "value": v})); }
    let off = shape(&mut s, &tr[1], 0.0, 2.0); c(&mut s, "set_property", json!({"clip_id": off, "property": "x", "value": 5000}));
    let pa = c(&mut s, "add_particles", json!({"track_id": tr[1], "kind": "snow", "count": 80, "start": 0, "duration": 2}))["clip_id"].as_str().unwrap().to_string();
    let inner = shape(&mut s, &tr[1], 0.0, 2.0); let grp = c(&mut s, "group_clips", json!({"clip_ids": [inner]}))["clip_id"].as_str().unwrap().to_string();
    let bg = c(&mut s, "add_clip", json!({"track_id": tr[2], "source_type": "gradient", "color": [0, 0, 0], "color2": [1, 1, 1], "start": 0, "duration": 2}))["clip_id"].as_str().unwrap().to_string();
    c(&mut s, "set_meta", json!({"id": bg, "name": "bg"}));
    let p = std::env::temp_dir().join(format!("kimo_ch_look_{}.png", std::process::id())); let ps = p.to_str().unwrap();
    let r = c(&mut s, "look", json!({"time": 0.5, "overlays": ["ids", "safe", "motion_paths", "alpha", "scopes"], "max_clips": 6, "path": ps}));
    assert_eq!(r["image"], ps); assert!(p.is_file()); assert_eq!(r["image_size"], json!([320 + 135, 180]), "the scopes panel is 3/4 of the image height");
    let clips = r["clips"].as_array().unwrap(); assert_eq!(clips.len(), 6);
    let by = |id: &str| clips.iter().find(|e| e["id"] == id).cloned().unwrap();
    assert_eq!(by(&bg)["kind"], "gradient"); assert_eq!(by(&bg)["fills_frame"], true); assert_eq!(by(&bg)["box"], json!([0.0, 0.0, 320.0, 180.0]));
    assert_eq!(by(&tx)["kind"], "text"); assert_eq!(by(&pa)["kind"], "particles"); assert_eq!(by(&grp)["kind"], "precomp");
    assert_eq!(by(&off)["box"], Value::Null); assert_eq!(by(&off)["note"], "nothing drawn at this time (fully transparent or off-screen)");
    assert_eq!(by(&mv)["keyframes"], json!({"x": [0.0, 1.0]}));
    let w: Vec<&str> = r["warnings"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert!(w.contains(&format!("{tx} is outside title safe").as_str()), "{w:?}"); assert!(w.contains(&"1 lower clip(s) not outlined (max_clips)"), "{w:?}");
    assert_eq!(clips[0]["id"], bg.as_str(), "top of the stack first");
    let _ = std::fs::remove_file(&p);
}
