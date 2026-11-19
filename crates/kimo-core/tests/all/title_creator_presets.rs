//! add_title creator presets: hook_box (hook line on a rounded box) and subscribe_pop (pill badge + dot + text) on a 9:16 frame.
use kimo_core::commands::{dispatch, Session};
use kimo_core::render;
use serde_json::{json, Value};
fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n}: {e}")) }
fn e(s: &mut Session, n: &str, a: Value) -> String { dispatch(s, n, &a).unwrap_err() }
fn scene() -> (Session, String) { let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 1080, "height": 1920, "fps": 30})); let t = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string(); (s, t) }

#[test]
fn hook_box_pops_in_two_clips() {
    let (mut s, t) = scene();
    let before = s.project.tracks[0].clips.len();
    let r = c(&mut s, "add_title", json!({"track_id": t, "style": "hook_box", "text": "WAIT FOR IT", "start": 0.0, "duration": 2.5}));
    let ids = r["clip_ids"].as_array().expect("clip_ids");
    assert!(ids.len() >= 2, "{r}");
    assert_eq!(s.project.tracks[0].clips.len(), before + ids.len(), "{r}");
}

#[test]
fn subscribe_pop_three_clips_and_bounces() {
    let (mut s, t) = scene();
    let before = s.project.tracks[0].clips.len();
    let r = c(&mut s, "add_title", json!({"track_id": t, "style": "subscribe_pop", "text": "FOLLOW", "start": 0.0, "duration": 2.5}));
    let ids = r["clip_ids"].as_array().expect("clip_ids");
    assert!(ids.len() >= 3, "{r}");
    assert_eq!(s.project.tracks[0].clips.len(), before + ids.len(), "{r}");
}

#[test]
fn hook_box_and_subscribe_pop_render_without_panicking() {
    for (style, text) in [("hook_box", "WAIT FOR IT"), ("subscribe_pop", "FOLLOW")] {
        let (mut s, t) = scene();
        c(&mut s, "add_title", json!({"track_id": t, "style": style, "text": text, "start": 0.0, "duration": 2.5}));
        let tm = 1.25; let mut cache = render::Cache::new();
        render::prepare_at(&s.project, &mut cache, Some(tm)).unwrap_or_else(|e| panic!("{style} prepare: {e}"));
        let f = render::render_frame(&s.project, tm, &cache);
        assert!(f.is_ok(), "{style}: {:?}", f.err());
    }
}

#[test]
fn title_unknown_style_still_errors() {
    let (mut s, t) = scene();
    let err = e(&mut s, "add_title", json!({"track_id": t, "style": "not_a_real_style", "text": "x"}));
    assert!(err.contains("unknown style"), "{err}");
}

// ---- the other add_title styles with subtitles, width fitting, font fallback warnings and refusals (titles.rs) ----
fn yb(s: &Session, i: usize) -> f64 { s.project.tracks[0].clips[i].props["y"].base }
fn sized(w: u32, h: u32) -> (Session, String) { let mut s = Session::new(); c(&mut s, "project_set", json!({"width": w, "height": h, "fps": 30})); let t = c(&mut s, "add_track", json!({}))["track_id"].as_str().unwrap().to_string(); (s, t) }

#[test]
fn subtitles_sit_below_the_title_in_every_style() {
    // 1920x1080: k = 1 (short side / 1080)
    let (mut s, t) = sized(1920, 1080);
    let r = c(&mut s, "add_title", json!({"track_id": t, "style": "cinematic", "text": "DUNE", "subtitle": "part two", "accent": [1, 0, 0]}));
    assert_eq!(r["clip_ids"].as_array().unwrap().len(), 4, "title + subtitle + two rules");
    assert_eq!((yb(&s, 0), yb(&s, 1), yb(&s, 2), yb(&s, 3)), (-20.0, 105.0, -125.0, 150.0), "rules move below the subtitle");
    match &s.project.tracks[0].clips[2].source { kimo_core::model::Source::Shape { fill, width, .. } => assert_eq!((*fill, *width), ([1.0, 0.0, 0.0], 260.0)), o => panic!("{o:?}") }
    let (mut s, t) = sized(1920, 1080);
    c(&mut s, "add_title", json!({"track_id": t, "style": "impact", "text": "BIG\\nNEWS", "subtitle": "today"}));
    assert_eq!(s.project.tracks[0].clips.len(), 2); assert_eq!(yb(&s, 1), 380.0, "190 px per title line");
    match &s.project.tracks[0].clips[0].source { kimo_core::model::Source::Text { text, .. } => assert_eq!(text, "BIG\nNEWS", "\\n becomes a line break"), o => panic!("{o:?}") }
    let (mut s, t) = sized(1920, 1080);
    c(&mut s, "add_title", json!({"track_id": t, "style": "elegant", "text": "a\\nb", "subtitle": "by night"}));
    assert_eq!(yb(&s, 1), 260.0, "130 px per title line");
    // lower third: accent bar at the left margin, name and subtitle boxed to 900 px, both left aligned at the same x
    let (mut s, t) = sized(1920, 1080);
    c(&mut s, "add_title", json!({"track_id": t, "style": "lower_third", "text": "Ada Lovelace", "subtitle": "mathematician", "duration": 3.0}));
    let cl = &s.project.tracks[0].clips; assert_eq!(cl.len(), 3);
    assert_eq!((cl[0].props["x"].base, cl[0].props["y"].base), (-848.0, 300.0));
    assert_eq!((cl[1].props["x"].base, cl[1].props["y"].base, cl[2].props["x"].base, cl[2].props["y"].base), (-390.0, 272.0, -390.0, 332.0));
    let sc: Vec<(f64, f64)> = cl[0].props["scale"].keys.iter().map(|k| (k.t, k.v)).collect(); assert_eq!(sc, [(0.0, 0.0), (0.45, 1.0), (2.55, 1.0), (3.0, 0.0)]);
    let fx = cl[2].text_fx.as_ref().unwrap(); assert_eq!((fx.box_width, fx.animators.len(), fx.animators[0].tracking.base), (900.0, 2, 4.5));
}
