//! Observe and expect, functional: a tool call with `observe` reports the touched span, probe changes and a strip image,
//! and `expect` checks brightness, black frames and readability, on a 64x32 canvas at 10 fps where brightness values are
//! exact: an sRGB red solid has luma 0.2126, white 1, black 0.
use kimo_core::commands::{dispatch, Session};
use kimo_core::observe::{self, expect};
use serde_json::{json, Value};

fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n} {a}: {e}")) }
fn session() -> (Session, String) { let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 64, "height": 32, "fps": 10})); let t = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string(); (s, t) }
fn solid(s: &mut Session, t: &str, col: [f64; 3], start: f64, dur: f64) -> String { c(s, "add_clip", json!({"track_id": t, "source_type": "solid", "color": col, "start": start, "duration": dur}))["clip_id"].as_str().unwrap().to_string() }
fn fails(s: &mut Session, e: Value) -> Vec<String> { expect::check(s, &e, &json!({})).iter().flat_map(|f| f["failed"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<_>>()).collect() }

#[test] fn fast_observation_renders_probes_and_a_strip_image() {
    let (mut s, t) = session(); solid(&mut s, &t, [0.0, 0.0, 0.0], 0.0, 2.0); let t2 = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string();
    let r = observe::wrap(&mut s, &json!({"track_id": t2, "source_type": "solid", "color": [1, 1, 1], "start": 0, "duration": 2, "observe": "fast"}), |ss, a| dispatch(ss, "add_clip", a)).unwrap();
    let o = &r["observe"]; assert_eq!(o["mode"], "fast"); assert_eq!(o["touched"]["spans"], json!([[0.0, 2.0]]));
    let pr = o["probes"].as_array().unwrap(); assert_eq!(pr.len(), 3);
    for p in pr { assert_eq!(p["changed_share"], 1.0, "every pixel went from black to white"); assert_eq!(p["changed_box"], json!([0.0, 0.0, 64.0, 32.0])); }
    let img = o["image"]["path"].as_str().unwrap(); assert!(std::path::Path::new(img).is_file());
    assert_eq!(s.media.last().unwrap().path, img);
    // full mode uses more probes; a global change probes the start, middle and end of the timeline
    let r = observe::wrap(&mut s, &json!({"name": "x", "linear_blend": true, "observe": "full"}), |ss, a| dispatch(ss, "project_set", a)).unwrap();
    assert_eq!(r["observe"]["touched"]["global"], true); assert_eq!(r["observe"]["mode"], "full");
    assert_eq!(r["observe"]["probes"].as_array().unwrap().len(), 3);
}

#[test] fn expectations_on_brightness_black_frames_and_text() {
    let (mut s, t) = session(); solid(&mut s, &t, [1.0, 0.0, 0.0], 0.0, 1.0);
    assert!(fails(&mut s, json!({"luma": [0.2, 0.22], "at": 0.5})).is_empty(), "red is 0.2126");
    assert_eq!(fails(&mut s, json!({"luma": [0.5], "at": 0.5})), vec!["brightness at 0.50 s is 0.213, expected 0.5..1"]);
    assert!(fails(&mut s, json!({"luma": [0.2, 0.22], "at": 0.5, "region": [8, 8, 16, 8]})).is_empty());
    assert!(fails(&mut s, json!({"no_black_frames": [0, 1]})).is_empty());
    assert_eq!(fails(&mut s, json!({"no_black_frames": [0, 2]})), vec!["black frame at 1.17 s"]);
    let txt = c(&mut s, "add_text", json!({"track_id": t, "text": "a much longer line of text than anyone can read", "start": 0, "duration": 0.2, "size": 8}))["clip_id"].as_str().unwrap().to_string();
    assert!(!fails(&mut s, json!({"readable": txt})).is_empty(), "0.2 s is too short to read that line");
}
