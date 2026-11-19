//! HUD / info-graphic one-call tools (hud.rs): detection box and number counter.
//! Each tool builds ordinary clips; the tests check the clip layout, timing and keyframes the contracts describe, on a tiny
//! 64x36 project (k = height / 720 = 0.05 scales every px@720p size).
use kimo_core::commands::{dispatch, Session};
use kimo_core::model::{Clip, Source};
use serde_json::{json, Value};
use std::sync::Arc;

fn session(w: u32, h: u32, fps: f64) -> (Session, String) {
    let mut s = Session::new(); dispatch(&mut s, "project_set", &json!({"width": w, "height": h, "fps": fps})).unwrap();
    let t = dispatch(&mut s, "add_track", &json!({})).unwrap()["track_id"].as_str().unwrap().to_string(); (s, t)
}
fn run(s: &mut Session, tool: &str, t: &str, mut a: Value) -> Result<Value, String> { a["track_id"] = json!(t); dispatch(s, tool, &a) }
fn clips(s: &Session) -> Vec<Arc<Clip>> { s.project.tracks[0].clips.clone() }
fn keys(c: &Clip, p: &str) -> Vec<(f64, f64)> { c.props[p].keys.iter().map(|k| (k.t, k.v)).collect() }
fn base(c: &Clip, p: &str) -> f64 { c.props[p].base }
fn approx(a: &[(f64, f64)], b: &[(f64, f64)], what: &str) { assert_eq!(a.len(), b.len(), "{what}: {a:?} vs {b:?}"); for (x, y) in a.iter().zip(b) { assert!((x.0 - y.0).abs() < 1e-9 && (x.1 - y.1).abs() < 1e-9, "{what}: {a:?} vs {b:?}"); } }
fn shape_of(c: &Clip) -> (f64, f64, [f64; 3], f64) { match &c.source { Source::Shape { width, height, fill, stroke_width, .. } => (*width, *height, *fill, *stroke_width), s => panic!("not a shape: {s:?}") } }
fn text_of(c: &Clip) -> (String, [f64; 3]) { match &c.source { Source::Text { text, color, .. } => (text.clone(), *color), s => panic!("not text: {s:?}") } }

#[test]
fn detection_box_builds_fill_outline_ticks_and_label() {
    let (mut s, t) = session(64, 36, 25.0);
    let r = run(&mut s, "add_detection_box", &t, json!({"x": 0.25, "y": 0.25, "w": 0.5, "h": 0.5, "label": "car", "confidence": 0.92, "start": 1.0, "duration": 2.0})).unwrap();
    let cl = clips(&s); assert_eq!(r["clip_ids"].as_array().unwrap().len(), 12); assert_eq!(cl.len(), 12, "fill + outline + 8 ticks + plate + text");
    // soft fill: 32x18 px box centred on the frame, fades in to fill_opacity 0.05 over 0.35 s
    let (pw, ph, fill, _) = shape_of(&cl[0]); assert_eq!((pw, ph, fill), (32.0, 18.0, [0.2, 0.9, 0.5]));
    assert_eq!((base(&cl[0], "x"), base(&cl[0], "y"), cl[0].start, cl[0].duration), (0.0, 0.0, 1.0, 2.0));
    approx(&keys(&cl[0], "opacity"), &[(0.0, 0.0), (0.35, 0.05), (1.85, 0.05), (2.0, 0.0)], "fill opacity");
    // hollow outline (fill -1), stroke max(2k, 1.5), pops from 1.12 to 1 with motion blur
    let (_, _, ofill, sw) = shape_of(&cl[1]); assert_eq!((ofill[0], sw, cl[1].motion_blur), (-1.0, 1.5, 180.0));
    approx(&keys(&cl[1], "scale"), &[(0.0, 1.12), (0.35, 1.0)], "outline scale"); approx(&keys(&cl[1], "opacity")[..2], &[(0.0, 0.0), (0.15, 1.0)], "outline in");
    // corner ticks: length clamp(min(32,18) * 0.14, 8k, 36k) = 1.8 (capped), thickness 2.2 * 1.5 = 3.3
    let (tw, th, _, _) = shape_of(&cl[2]); assert!((tw - 1.8).abs() < 1e-9 && (th - 3.3).abs() < 1e-9, "tick {tw}x{th}");
    assert!((base(&cl[2], "x") - -(16.0 - 0.9)).abs() < 1e-9 && (base(&cl[2], "y") - -(9.0 - 1.65)).abs() < 1e-9, "top-left horizontal tick");
    // label text: "label conf" with dark ink on a light colour, revealed from 0.05 to 0.35 s, starting 0.18 s after the box
    let (txt, ink) = text_of(&cl[11]); assert_eq!((txt.as_str(), ink), ("car 0.92", [0.04, 0.05, 0.05]));
    assert!((cl[11].start - 1.18).abs() < 1e-9); approx(&keys(&cl[11], "reveal"), &[(0.05, 0.0), (0.35, 1.0)], "label reveal");
    // a dark colour gets white ink; a caption adds one more text clip
    let (mut s2, t2) = session(64, 36, 25.0);
    run(&mut s2, "add_detection_box", &t2, json!({"color": [0.1, 0.1, 0.8], "caption": "tracked", "font": "Inter"})).unwrap();
    let cl = clips(&s2); assert_eq!(cl.len(), 13); assert_eq!(text_of(&cl[11]).1, [1.0; 3]);
    let (cap, _) = text_of(&cl[12]); assert_eq!(cap, "tracked"); assert!((cl[12].start - 0.3).abs() < 1e-9 && (cl[12].duration - 2.7).abs() < 1e-9);
    assert_eq!(text_of(&cl[11]).0, "object 0.92", "default label and confidence");
}

#[test]
fn counter_rolls_with_quart_ease_grouping_and_settle_pop() {
    // 10 fps, 0.5 s roll -> 5 frames; v = 1000 * (1 - (1-u)^4) at u = 0, .25, .5, .75, 1
    let (mut s, t) = session(64, 36, 10.0);
    let r = run(&mut s, "add_counter", &t, json!({"from": 0, "to": 1000, "duration": 0.5, "hold": 1.0, "prefix": "$", "suffix": " pts", "grouping": "western", "font": "Inter", "x": 3, "y": -4})).unwrap();
    assert_eq!(r["frames"], 5); let cl = clips(&s);
    let texts: Vec<String> = cl.iter().map(|c| text_of(c).0).collect(); assert_eq!(texts, ["$0 pts", "$684 pts", "$938 pts", "$996 pts", "$1,000 pts"]);
    assert!(cl[..4].iter().all(|c| (c.duration - 0.1).abs() < 1e-12), "one frame per step");
    assert!((cl[4].start - 0.4).abs() < 1e-12 && (cl[4].duration - 1.1).abs() < 1e-12, "last value holds to start + duration + hold");
    approx(&keys(&cl[4], "scale"), &[(0.0, 1.0), (0.12, 1.06), (0.4, 1.0)], "settle pop"); assert!(keys(&cl[0], "scale").is_empty());
    assert_eq!((base(&cl[2], "x"), base(&cl[2], "y")), (3.0, -4.0));
    if let Source::Text { font, .. } = &cl[0].source { assert_eq!(font, "Inter"); }
    // decimals and Devanagari digits
    let (mut s, t) = session(64, 36, 10.0);
    run(&mut s, "add_counter", &t, json!({"from": 1, "to": 2, "duration": 0.1, "decimals": 1, "devanagari_digits": true})).unwrap();
    let texts: Vec<String> = clips(&s).iter().map(|c| text_of(c).0).collect(); assert_eq!(texts, ["\u{0967}.\u{0966}", "\u{0968}.\u{0966}"], "at least 2 frames (Devanagari 1.0 and 2.0)");
}
