//! The MCP surface of the shared light / material model (ext/lighting_tools.rs over lighting.rs): lights, rigs, materials,
//! the relight effect and the material preview, on an empty project (the preview is a 64 px PNG).
use kimo_core::commands::{dispatch, Session};
use serde_json::json;
fn ok(s: &mut Session, n: &str, a: serde_json::Value) -> serde_json::Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n}: {e}")) }
fn no(s: &mut Session, n: &str, a: serde_json::Value) -> String { dispatch(s, n, &a).unwrap_err() }

#[test]
fn light_tools_create_update_rig_list_and_remove() {
    let mut s = Session::new();
    let l = ok(&mut s, "set_light", json!({"id": "k", "kind": "point", "pos": [0.1, 0.2, 0.3], "intensity": 2.0}));
    assert_eq!((l["id"].as_str(), l["intensity"].as_f64(), l["pos"].clone()), (Some("k"), Some(2.0), json!([0.1, 0.2, 0.3])));
    let l = ok(&mut s, "set_light", json!({"id": "k", "intensity": 5.0})); assert_eq!((l["intensity"].as_f64(), l["pos"].clone()), (Some(5.0), json!([0.1, 0.2, 0.3])), "update keeps the other fields");
    ok(&mut s, "set_light", json!({"id": "k", "keys": [{"t": 0.0, "intensity": 1.0}, {"t": 1.0, "pos": [0.5, 0.5, 0.5]}]}));
    assert!(no(&mut s, "set_light", json!({"id": ""})).contains("1..64 chars"));
    assert!(no(&mut s, "set_light", json!({"id": "x", "intensity": "bright"})).starts_with("invalid value"));
    assert!(no(&mut s, "set_light", json!({"id": "x", "kind": "area"})).contains("kind must be"));
    let many: Vec<serde_json::Value> = (0..257).map(|i| json!({"t": i as f64})).collect(); assert!(no(&mut s, "set_light", json!({"id": "x", "keys": many})).contains("max 256"));
    assert!(no(&mut s, "set_light", json!({"id": "x", "keys": [{"t": 0.0, "intensity": 2000.0}]})).contains("key intensity must be in 0..1000"));
    for i in 0..15 { ok(&mut s, "set_light", json!({"id": format!("l{i}")})); }
    assert!(no(&mut s, "set_light", json!({"id": "one_too_many"})).contains("max 16 lights")); ok(&mut s, "set_light", json!({"id": "l0", "intensity": 3.0}));
    assert_eq!(ok(&mut s, "list_lights", json!({}))["lights"].as_array().unwrap().len(), 16);
    assert!(no(&mut s, "light_rig", json!({"preset": "three_point"})).contains("max 16 lights"), "a rig needs three free slots");
    for i in 0..15 { ok(&mut s, "remove_light", json!({"id": format!("l{i}")})); }
    assert!(no(&mut s, "remove_light", json!({"id": "l0"})).contains("no such light 'l0'"));
    // three-point rig: key moved with the subject, warmed by +0.15 red / -0.3 blue per unit warmth, fill = key / ratio, rim = key * 0.9
    let r = ok(&mut s, "light_rig", json!({"preset": "three_point", "prefix": "r", "intensity": 2.0, "warmth": 0.5, "subject": [0.6, 0.4]}));
    assert_eq!(r["ratio"], 3.0); let ls = r["lights"].as_array().unwrap(); assert_eq!(ls.len(), 3);
    let p = ls[0]["pos"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>(); assert!((p[0] - 0.3).abs() < 1e-12 && (p[1] - 0.1).abs() < 1e-12 && p[2] == 0.9, "{p:?}");
    let c = ls[0]["color"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>(); assert!(c[0] == 1.0 && c[1] == 0.93 && (c[2] - 0.67).abs() < 1e-12, "{c:?}");
    assert!((ls[1]["intensity"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-12 && (ls[2]["intensity"].as_f64().unwrap() - 1.8).abs() < 1e-12);
    assert_eq!((ls[0]["id"].as_str(), ls[1]["id"].as_str(), ls[2]["id"].as_str()), (Some("r_key"), Some("r_fill"), Some("r_rim")));
    // every recipe has its own default ratio; re-running a prefix replaces its three lights
    for (preset, ratio) in [("rembrandt", 6.0), ("butterfly", 4.0), ("split", 16.0), ("golden_hour", 3.5), ("neon_night", 1.5), ("three_point", 3.0)] { assert_eq!(ok(&mut s, "light_rig", json!({"preset": preset, "prefix": "r"}))["ratio"], ratio, "{preset}"); }
    assert_eq!(ok(&mut s, "list_lights", json!({}))["lights"].as_array().unwrap().len(), 4, "k + one rig");
    assert_eq!(ok(&mut s, "light_rig", json!({"preset": "split", "prefix": "q", "ratio": 2.0}))["ratio"], 2.0);
    for (a, msg) in [(json!({"preset": "disco"}), "unknown preset 'disco'"), (json!({"preset": "split", "prefix": ""}), "prefix must be 1..40"), (json!({"preset": "split", "intensity": -1}), "intensity must be 0..1000"),
        (json!({"preset": "split", "warmth": 2}), "warmth must be -1..1"), (json!({"preset": "split", "subject": [0.5]}), "subject must be [x,y]"), (json!({"preset": "split", "ratio": 64}), "ratio must be 1..32")] {
        assert!(no(&mut s, "light_rig", a).contains(msg), "{msg}"); }
}

#[test]
fn material_tools_relight_effect_and_preview() {
    let mut s = Session::new();
    // a preset name starts from the preset: only roughness changes on chrome
    let m = ok(&mut s, "set_material", json!({"name": "chrome", "roughness": 0.3})); assert_eq!((m["roughness"].as_f64(), m["metal"].as_f64(), m["name"].as_str()), (Some(0.3), Some(1.0), Some("chrome")));
    let m = ok(&mut s, "set_material", json!({"name": "mine", "albedo": [0.2, 0.3, 0.4]})); assert_eq!((m["roughness"].as_f64(), m["ior"].as_f64()), (Some(0.5), Some(1.5)), "a new name starts from the defaults");
    assert!(no(&mut s, "set_material", json!({"name": ""})).contains("1..64 chars"));
    assert!(no(&mut s, "set_material", json!({"name": "bad", "ior": 9})).contains("ior must be in 1..4"));
    for i in 0..62 { ok(&mut s, "set_material", json!({"name": format!("m{i}")})); }
    assert!(no(&mut s, "set_material", json!({"name": "overflow"})).contains("max 64 materials"));
    let lm = ok(&mut s, "list_materials", json!({})); assert_eq!((lm["defined"].as_object().unwrap().len(), lm["presets"].as_array().unwrap().len()), (64, 10));
    assert_eq!(ok(&mut s, "remove_material", json!({"name": "m0"}))["removed"], "m0"); assert!(no(&mut s, "remove_material", json!({"name": "m0"})).contains("no such defined material"));
    // set_relight validates every string and param, then adds or updates the clip's relight
    ok(&mut s, "set_light", json!({"id": "key", "kind": "point"}));
    let t = ok(&mut s, "add_track", json!({}))["track_id"].clone();
    let c = ok(&mut s, "add_clip", json!({"track_id": t, "source_type": "solid", "color": [0.5, 0.5, 0.5], "start": 0, "duration": 1}))["clip_id"].as_str().unwrap().to_string();
    for (a, msg) in [(json!({"strings": {"tint": "red"}}), "unknown string 'tint'"), (json!({"strings": {"material": 3}}), "must be strings"), (json!({"params": {"glow": 1}}), "unknown param 'glow'"),
        (json!({"params": {"amount": "x"}}), "must be a finite number"), (json!({"strings": {"height_source": "z"}}), "height_source must be"), (json!({"strings": {"material": "lava"}}), "unknown material 'lava'"),
        (json!({"strings": {"lights": "key, ghost"}}), "unknown light 'ghost'"), (json!({"effect_id": "relight9"}), "no relight 'relight9'")] {
        let mut a = a; a["clip_id"] = json!(c); assert!(no(&mut s, "set_relight", a).contains(msg), "{msg}"); }
    assert!(no(&mut s, "set_relight", json!({"clip_id": "nope"})).contains("no such clip"));
    let r = ok(&mut s, "set_relight", json!({"clip_id": c, "params": {"amount": 0.5}, "strings": {"material": "mine", "lights": "key", "height_source": "luma"}})); assert_eq!(r["effect_id"], "relight");
    ok(&mut s, "set_relight", json!({"clip_id": c, "effect_id": "relight", "params": {"bump": 2.0}}));
    let fx = &s.project.tracks[0].clips[0].effects; assert_eq!(fx.len(), 1, "updated in place");
    assert_eq!((fx[0].params["bump"].base, fx[0].params["amount"].base, fx[0].strings.len()), (2.0, 1.0, 0), "an update replaces params and strings");
    // preview: a lit sphere PNG with its luminance stats
    let path = std::env::temp_dir().join(format!("kimo-matprev-{}.png", std::process::id()));
    let r = ok(&mut s, "preview_material", json!({"path": path, "material": "mine", "size": 64}));
    assert_eq!(r["lights"], 1); let (mean, max) = (r["mean_luma"].as_f64().unwrap(), r["max_luma"].as_f64().unwrap()); assert!(mean > 0.0 && max >= mean && max <= 1.0, "{mean} {max}");
    assert!(std::fs::read(&path).unwrap().starts_with(b"\x89PNG")); std::fs::remove_file(&path).unwrap();
    assert!(no(&mut s, "preview_material", json!({"path": path, "material": "mine", "size": 10})).contains("size must be 64..1024"));
    assert!(no(&mut s, "preview_material", json!({"path": path, "material": "lava"})).contains("unknown material"));
}
