//! Path shape tools (ext/path_tools.rs over paths.rs): add_path_shape / set_path / path_info / add_path_key /
//! clear_path_keys on a 32x24 project, checked on the rendered pixels.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};
fn sess() -> (Session, String) { let mut s = Session::new(); dispatch(&mut s, "project_set", &json!({"width": 32, "height": 24, "fps": 10})).unwrap(); let t = dispatch(&mut s, "add_track", &json!({})).unwrap()["track_id"].as_str().unwrap().to_string(); (s, t) }
fn red_at(s: &Session, t: f64, x: usize, y: usize) -> f32 { let mut c = kimo_core::render::Cache::new(); kimo_core::render::prepare_at(&s.project, &mut c, Some(t)).unwrap(); kimo_core::render::render_frame(&s.project, t, &c).unwrap().d[(y * 32 + x) * 4] }
fn add(s: &mut Session, t: &str, extra: Value) -> Result<Value, String> { let mut a = json!({"track_id": t, "start": 0, "duration": 2, "fill": [1, 0, 0]}); for (k, v) in extra.as_object().unwrap() { a[k] = v.clone(); } dispatch(s, "add_path_shape", &a) }

#[test]
fn path_shape_tools_build_report_and_morph_paths() {
    let (mut s, t) = sess();
    // a closed point list becomes "M.. L.. Z"; the report gives length, bbox and fill rule
    let r = add(&mut s, &t, json!({"points": [[-8, -6], [8, -6], [8, 6], [-8, 6]], "closed": true})).unwrap();
    assert_eq!(r["d"], "M-8 -6 L8 -6 L8 6 L-8 6 Z"); assert_eq!((r["length_px"].as_f64(), r["segments"].as_u64(), r["fill_rule"].as_str()), (Some(56.0), Some(4), Some("nonzero")));
    assert_eq!(r["bbox"], json!({"x0": -8.0, "y0": -6.0, "x1": 8.0, "y1": 6.0}));
    let id = r["clip_id"].as_str().unwrap().to_string();
    assert!(red_at(&s, 0.5, 16, 12) > 0.99, "filled inside"); assert!(red_at(&s, 0.5, 2, 2) < 0.01, "empty outside");
    // path_info reads the same back; set_path swaps the outline (even-odd hole in the middle)
    let i = dispatch(&mut s, "path_info", &json!({"clip_id": id})).unwrap(); assert_eq!((i["length_px"].as_f64(), i["path_keys"].as_u64()), (Some(56.0), Some(0)));
    let r = dispatch(&mut s, "set_path", &json!({"clip_id": id, "d": "M-10 -8 H10 V8 H-10 Z M-3 -3 H3 V3 H-3 Z", "fill_rule": "evenodd"})).unwrap(); assert_eq!((r["subpaths"].as_u64(), r["fill_rule"].as_str()), (Some(2), Some("evenodd")));
    assert!(red_at(&s, 0.5, 16, 12) < 0.01, "even-odd hole"); assert!(red_at(&s, 0.5, 9, 12) > 0.99, "ring");
    // path keys: a key at 1 s adds the base as a key at 0; the shape morphs between them, and clearing returns to the base
    dispatch(&mut s, "set_path", &json!({"clip_id": id, "d": "M-4 -4 H4 V4 H-4 Z"})).unwrap();
    let k = dispatch(&mut s, "add_path_key", &json!({"clip_id": id, "time": 1.0, "d": "M-14 -10 H14 V10 H-14 Z", "ease": "linear"})).unwrap();
    assert_eq!(k["keys"].as_array().unwrap().len(), 2);
    assert!(red_at(&s, 0.0, 16 + 10, 12) < 0.01, "small at 0 s"); assert!(red_at(&s, 1.0, 16 + 10, 12) > 0.99, "grown at 1 s");
    assert_eq!(dispatch(&mut s, "path_info", &json!({"clip_id": id})).unwrap()["path_keys"], 2);
    assert_eq!(dispatch(&mut s, "clear_path_keys", &json!({"clip_id": id})).unwrap()["removed"], true);
    assert_eq!(dispatch(&mut s, "clear_path_keys", &json!({"clip_id": id})).unwrap()["removed"], false);
    assert!(red_at(&s, 1.0, 16 + 10, 12) < 0.01, "base path again");
    // smooth open points: Catmull-Rom curves through every point
    let r = add(&mut s, &t, json!({"points": [[-10, 0], [0, -8], [10, 0]], "smooth": true, "tension": 0.0})).unwrap(); assert_eq!(r["d"], "M-10 0 C-10 0 0 -8 0 -8 C0 -8 10 0 10 0");
}
