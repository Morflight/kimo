//! contact_sheet on a 64x36 project: grid geometry, the 5x7 time labels (counted pixel by pixel from the PNG), range and cut
//! sampling with the 16-tile cap, and keyframe-derived tiles with their keyed props.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};

fn c(ss: &mut Session, n: &str, v: Value) -> Value { dispatch(ss, n, &v).unwrap_or_else(|e| panic!("{n}: {e}")) }
fn out(tag: &str) -> String { std::env::temp_dir().join(format!("kimo-sheet-{tag}-{}.png", std::process::id())).to_string_lossy().into_owned() }
/// 10 s red solid on one track; a green solid at 2..5 s on another with x keyed at 0.5 s and 1.0 s and opacity at 1.0 s.
fn scene() -> (Session, String) {
    let mut ss = Session::new(); c(&mut ss, "project_set", json!({"width": 64, "height": 36, "fps": 10}));
    let t = c(&mut ss, "add_track", json!({}))["track_id"].clone();
    c(&mut ss, "add_clip", json!({"track_id": t, "source_type": "solid", "color": [1, 0, 0], "start": 0, "duration": 10}));
    let t2 = c(&mut ss, "add_track", json!({}))["track_id"].clone();
    let g = c(&mut ss, "add_clip", json!({"track_id": t2, "source_type": "solid", "color": [0, 1, 0], "start": 2, "duration": 3}))["clip_id"].as_str().unwrap().to_string();
    for (p, t, v) in [("x", 0.5, 10.0), ("x", 1.0, 20.0), ("opacity", 1.0, 0.5)] { c(&mut ss, "set_keyframe", json!({"clip_id": g, "property": p, "time": t, "value": v})); }
    (ss, g)
}
/// The PNG decoded to RGBA8 by ffmpeg.
fn rgba(path: &str) -> Vec<u8> {
    let o = std::process::Command::new("ffmpeg").args(["-v", "error", "-i", path, "-f", "rawvideo", "-pix_fmt", "rgba", "-"]).output().unwrap();
    assert!(o.status.success()); o.stdout
}
fn white_in(px: &[u8], w: usize, x0: usize, y0: usize, bw: usize, bh: usize) -> usize {
    let mut n = 0; for y in y0..y0 + bh { for x in x0..x0 + bw { let i = (y * w + x) * 4; if px[i] == 255 && px[i + 1] == 255 && px[i + 2] == 255 { n += 1; } } } n
}

#[test]
fn labelled_grid_geometry_and_every_digit_glyph() {
    let (mut ss, _) = scene(); let p = out("grid");
    let r = c(&mut ss, "contact_sheet", json!({"path": p, "times": [9.09, 1.234, 5.678], "tile_px": 64}));
    // 3 tiles of 64x36: 2 columns, 2 rows, 4 px gaps
    assert_eq!((r["width"].clone(), r["height"].clone(), r["cols"].clone(), r["rows"].clone(), r["tile"].clone(), r["tile_count"].clone()),
        (json!(140), json!(84), json!(2), json!(2), json!({"w": 64, "h": 36}), json!(3)));
    let pos: Vec<(f64, u64, u64)> = r["tiles"].as_array().unwrap().iter().map(|t| (t["timeline_time"].as_f64().unwrap(), t["x"].as_u64().unwrap(), t["y"].as_u64().unwrap())).collect();
    assert_eq!(pos, vec![(1.234, 4, 4), (5.678, 72, 4), (9.09, 4, 44)], "sorted by time, row-major");
    assert!(r.get("omitted").is_none() && r.get("all_times").is_none());
    let px = rgba(&p);
    assert_eq!(px.len(), 140 * 84 * 4);
    // label box (6 px per char + 2) x 9 at tile corner + 2; lit glyph bits, counted from the 5x7 font:
    // "t=1.234s" = 12+10+10+4+14+15+14+13, "t=5.678s" = 12+10+17+4+15+11+17+13, "t=9.090s" = 12+10+15+4+19+15+19+13
    for ((x, y), want) in [((6, 6), 92), ((74, 6), 99), ((6, 46), 107)] { assert_eq!(white_in(&px, 140, x, y, 50, 9), want, "label at {x},{y}"); }
    // away from the label the tile is the red solid
    let i = ((4 + 30) * 140 + 4 + 40) * 4; assert_eq!(&px[i..i + 4], &[255, 0, 0, 255]);
    // labels none: the same tile with no label at all
    let p2 = out("nolabel");
    c(&mut ss, "contact_sheet", json!({"path": p2, "times": [1.234], "tile_px": 64, "labels": "none"}));
    assert_eq!(white_in(&rgba(&p2), 72, 4, 4, 64, 36), 0);
    std::fs::remove_file(p).ok(); std::fs::remove_file(p2).ok();
}

#[test]
fn ranges_and_cuts_are_capped_at_sixteen_even_picks_and_keyframes_carry_their_props() {
    let (mut ss, g) = scene(); let p = out("cap");
    // 16 range times (n clamps 20 to 16) plus the cut at 2 s: 17 distinct times, 16 shown, first and last kept
    // (range step 9.5/15 s; the cut at 0 coincides with the first range time, the cut at 2 s does not)
    let r = c(&mut ss, "contact_sheet", json!({"path": p, "start": 0, "end": 9.5, "n": 20, "at_cuts": true, "tile_px": 64}));
    assert_eq!((r["tile_count"].clone(), r["omitted"].clone(), r["cols"].clone()), (json!(16), json!(1), json!(4)));
    let all: Vec<f64> = r["all_times"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert_eq!(all.len(), 17);
    assert!(all.iter().any(|t| (t - 2.0).abs() < 1e-9) && all[0] == 0.0 && all[16] == 9.5);
    let shown: Vec<f64> = r["tiles"].as_array().unwrap().iter().map(|t| t["timeline_time"].as_f64().unwrap()).collect();
    assert_eq!((shown[0], shown[15]), (0.0, 9.5));
    assert!(r["note"].as_str().unwrap().starts_with("17 distinct times, showing 16"));
    // keyframe tiles: x at clip time 0.5 (timeline 2.5), x + opacity at 1.0 (3.0); time+props lists what is keyed there
    let r = c(&mut ss, "contact_sheet", json!({"path": p, "keyframes_of": g, "labels": "time+props", "tile_px": 64}));
    let tiles = r["tiles"].as_array().unwrap();
    assert_eq!((r["tile_count"].clone(), r["keyframe_candidates"].clone()), (json!(2), json!(2)));
    assert_eq!((tiles[0]["clip_id"].clone(), tiles[0]["clip_time"].as_f64(), tiles[0]["props"].clone()), (json!(g), Some(0.5), json!([{"prop": "x", "value": 10.0}])));
    assert_eq!(tiles[1]["props"], json!([{"prop": "opacity", "value": 0.5}, {"prop": "x", "value": 20.0}]));
    // plain time labels still name the keyed clip but list no props; a cap of 1 keeps the first keyframe time only
    let r = c(&mut ss, "contact_sheet", json!({"path": p, "keyframes_of": [g], "keyframe_cap": 1, "tile_px": 64}));
    assert_eq!((r["tile_count"].clone(), r["keyframe_cap_applied"].clone(), r["all_times"].clone()), (json!(1), json!(1), json!([2.5])));
    assert!(r["tiles"][0]["props"].is_null() && r["tiles"][0]["clip_id"] == json!(g));
    // explicit times outside the keyed clip, or inside it but off its keys, name no clip
    let r = c(&mut ss, "contact_sheet", json!({"path": p, "times": [0.5, 4.0], "keyframes_of": g, "tile_px": 64}));
    let named: Vec<(f64, bool)> = r["tiles"].as_array().unwrap().iter().map(|t| (t["timeline_time"].as_f64().unwrap(), t.get("clip_id").is_some())).collect();
    assert_eq!(named, vec![(0.5, false), (2.5, true), (3.0, true), (4.0, false)]);
    // a range of one tile is its start
    let r = c(&mut ss, "contact_sheet", json!({"path": p, "start": 1.0, "end": 2.0, "n": 1, "tile_px": 64}));
    assert_eq!((r["tile_count"].clone(), r["tiles"][0]["timeline_time"].clone()), (json!(1), json!(1.0)));
    std::fs::remove_file(p).ok();
}
