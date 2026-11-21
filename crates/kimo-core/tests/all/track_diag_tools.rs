//! Track-analysis toolkit (ext/track_diag) end to end on a tiny lossless synthetic plate: ffv1, 80x60, 10 fps, 6 frames of a smooth
//! aperiodic texture that translates exactly +1 px in x per frame (frame k at pixel (x, y) shows T(x - k, y)). Every feature
//! position, homography, decomposition, jitter and residual therefore has a hand-computed expectation: the motion at frame k is
//! the pure translation (k, 0).
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};

const W: f64 = 80.0; const N: usize = 6;
const ROI: [f64; 8] = [0.15, 0.15, 0.85, 0.15, 0.85, 0.85, 0.15, 0.85];

fn plate() -> std::path::PathBuf {
    static P: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    P.get_or_init(|| make_plate("main", "80x60", 0.6, "128+45*sin(0.55*(X-N))*sin(0.47*Y+0.3)+30*sin(0.21*(X-N)+0.33*Y)+20*cos(0.13*(X-N)-0.29*Y+1)")).clone()
}
fn make_plate(tag: &str, size: &str, d: f64, t: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("kimo-trackdiag-{tag}-{}.mkv", std::process::id()));
    let g = format!("nullsrc=s={size}:r=10:d={d},format=rgb24,geq=r='{t}':g='{t}':b='{t}'");
    assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", &g, "-c:v", "ffv1", "-pix_fmt", "bgr0", p.to_str().unwrap()]).status().unwrap().success());
    p
}
fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n} {a}: {e}")) }
fn setup() -> (Session, String) {
    let mut s = Session::new(); c(&mut s, "project_set", json!({"width": 80, "height": 60, "fps": 10}));
    let a = c(&mut s, "import_media", json!({"path": plate()}))["asset_id"].clone(); let t = c(&mut s, "add_track", json!({}))["track_id"].clone();
    let id = c(&mut s, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0, "duration": 0.6}))["clip_id"].as_str().unwrap().to_string();
    (s, id)
}
fn track(s: &mut Session, id: &str) -> Value { c(s, "track_features", json!({"clip_id": id, "roi": ROI, "min_distance": 4, "window": 6, "levels": 2})) }
fn f(v: &Value) -> f64 { v.as_f64().unwrap_or_else(|| panic!("not a number: {v}")) }
fn close(v: &Value, want: &[f64]) { let a = v.as_array().unwrap(); assert_eq!(a.len(), want.len(), "{v}"); for (x, w) in a.iter().zip(want) { near(f(x), *w, 1e-9, &v.to_string()); } }
fn near(a: f64, b: f64, tol: f64, what: &str) { assert!((a - b).abs() <= tol, "{what}: {a} vs {b} (tol {tol})"); }
/// The ROI quad shifted by k px in x, normalised, for every frame: an exact "native tracker" answer.
fn native_quads() -> Vec<f64> { (0..N).flat_map(|k| ROI.iter().enumerate().map(move |(i, v)| if i % 2 == 0 { v + k as f64 / W } else { *v })).collect() }

#[test]
fn track_features_follows_the_known_translation_and_stores_raw_tracks() {
    let (mut s, id) = setup();
    let r = track(&mut s, &id);
    assert_eq!(r["track_id"], "tracks1"); assert_eq!(r["first_frame"], 0); assert_eq!(r["frames"], N);
    let n = r["features"].as_u64().unwrap() as usize; assert!(n >= 16, "{r}");
    let held = (0..n).filter(|j| j % 4 == 0).count(); assert_eq!(r["held_out"], held); assert_eq!(r["train"], n - held); assert_eq!(r["alive_first"], n);
    close(&r["roi_px"], &[12.0, 9.0, 68.0, 9.0, 68.0, 51.0, 12.0, 51.0]);
    // the no-argument form lists every set
    let l = c(&mut s, "track_set_info", json!({})); assert_eq!(l["track_sets"].as_array().unwrap().len(), 1); assert_eq!(l["track_sets"][0]["features"], n);
    let info = c(&mut s, "track_set_info", json!({"track_id": "tracks1", "frames": [0, 5], "include_status": true}));
    let feats = info["feature_data"].as_array().unwrap(); assert_eq!(feats.len(), n);
    let (mut alive_end, mut died) = (0, 0);
    for (j, ft) in feats.iter().enumerate() {
        assert_eq!(ft["feature"], j); assert_eq!(ft["role"], if j % 4 == 0 { "held_out" } else { "train" });
        let (x0, y0) = (f(&ft["p0"][0]), f(&ft["p0"][1]));
        assert!(x0 >= 12.0 && x0 <= 68.0 && y0 >= 9.0 && y0 <= 51.0, "feature {j} outside the ROI");
        assert_eq!(ft["positions"][0]["xy"], json!([x0, y0])); assert_eq!(ft["positions"][0]["alive"], true);
        let p5 = &ft["positions"][1];
        if p5["alive"] == true { alive_end += 1; assert!(ft["died_at_frame"].is_null());
            near(f(&p5["xy"][0]), x0 + 5.0, 0.05, &format!("feature {j} x at frame 5")); near(f(&p5["xy"][1]), y0, 0.05, &format!("feature {j} y at frame 5"));
        } else { died += 1; let d = ft["died_at_frame"].as_u64().unwrap(); assert!(d >= 1 && d <= 5); }
    }
    assert_eq!(info["alive_last"], alive_end); assert_eq!(r["survived_all_frames"], alive_end); assert_eq!(died + alive_end, n);
    // status omitted unless asked; feature selection
    let one = c(&mut s, "track_set_info", json!({"features": [1], "frames": [2]}));
    assert_eq!(one["feature_data"].as_array().unwrap().len(), 1); assert!(one["feature_data"][0]["positions"][0]["alive"].is_null());
    assert!(c(&mut s, "track_set_info", json!({"features": [0]}))["feature_data"][0].get("positions").is_none());
}

#[test]
fn homography_fit_recovers_the_translation_and_every_diagnostic_agrees() {
    let (mut s, id) = setup(); track(&mut s, &id);
    let r = c(&mut s, "fit_track_homography", json!({"corners": ROI, "native_quads": native_quads()}));
    assert_eq!(r["fit_id"], "fit1"); assert_eq!(r["track_id"], "tracks1");
    let sm = &r["summary"]; assert_eq!(sm["frames"], N); assert_eq!(sm["carried_frames"], json!([]));
    assert!(f(&sm["fit_pooled_frame_median_px"]) < 0.05 && f(&sm["fit_worst_frame_p95_px"]) < 0.1, "{sm}");
    assert!(f(&sm["native_pooled_frame_median_px"]) < 0.05, "{sm}");
    for k in sm["corner_disagreement_last_frame_px"].as_array().unwrap() { assert!(f(k) < 0.1, "{sm}"); }
    let rows = r["rows"].as_array().unwrap(); assert_eq!(rows.len(), N);
    for (i, row) in rows.iter().enumerate() { assert_eq!(row["rel_frame"], i); near(f(&row["time_s"]), i as f64 / 10.0, 1e-12, "time"); assert!(f(&row["median_vector_drift_px"]) < 0.05, "{row}"); }
    assert!(c(&mut s, "fit_track_homography", json!({"corners": ROI, "include_rows": false})).get("rows").is_none());

    // fit_info: identity at frame 0 and translation (5, 0) at frame 5; listing form
    let fi = c(&mut s, "fit_info", json!({"fit_id": "fit1", "frames": [0, 5]}));
    assert_eq!(fi["kind"], "robust_soft_l1"); assert_eq!(fi["has_native"], true); assert_eq!(fi["has_ecc"], false);
    let h0 = &fi["frames_data"][0]["H_px"]; for i in 0..3 { for j in 0..3 { near(f(&h0[i][j]), if i == j { 1.0 } else { 0.0 }, 1e-9, "H0"); } }
    let h5 = &fi["frames_data"][1]["H_px"]; let want = [[1.0, 0.0, 5.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for i in 0..3 { for j in 0..3 { near(f(&h5[i][j]), want[i][j], if j == 2 && i < 2 { 0.05 } else { 2e-3 }, &format!("H5[{i}][{j}]")); } }
    let cp = fi["frames_data"][1]["corners_px"].as_array().unwrap(); near(f(&cp[0]), 17.0, 0.05, "corner0 x"); near(f(&cp[1]), 9.0, 0.05, "corner0 y");
    assert!(fi["frames_data"][0]["ecc"].is_null());
    assert_eq!(c(&mut s, "fit_info", json!({}))["fits"].as_array().unwrap().len(), 2);

    // decomposition at the quad centre (40, 30): no rotation, unit scales, displacement (k, 0)
    let d = c(&mut s, "decompose_track_homography", json!({"fit_id": "fit1", "every": 5}));
    close(&d["point_px"], &[40.0, 30.0]); let dr = d["rows"].as_array().unwrap(); assert_eq!(dr.len(), 2);
    near(f(&dr[1]["center_dx"]), 5.0, 0.05, "dx"); near(f(&dr[1]["center_dy"]), 0.0, 0.05, "dy"); near(f(&dr[1]["rotation_deg"]), 0.0, 0.1, "rot");
    near(f(&dr[1]["scale_major"]), 1.0, 2e-3, "major"); near(f(&dr[1]["scale_minor"]), 1.0, 2e-3, "minor"); near(f(&dr[1]["det"]), 1.0, 4e-3, "det");
    assert_eq!(d["summary"]["orientation_flip_frames"], json!([]));
    let dp = c(&mut s, "decompose_track_homography", json!({"fit_id": "fit1", "point": [0.25, 0.5], "include_rows": false}));
    close(&dp["point_px"], &[20.0, 30.0]); assert!(dp.get("rows").is_none());

    // jitter: every corner steps 1 px per frame, accelerations vanish; native quads are exact
    let j = c(&mut s, "track_jitter_metrics", json!({"fit_id": "fit1", "include_rows": true}));
    for src in ["fit", "native"] { let st = &j[src]["step_px"]; assert_eq!(st["n"], 4 * (N - 1)); near(f(&st["median"]), 1.0, 0.05, src); assert!(f(&j[src]["accel_px"]["max"]) < 0.1, "{src}: {}", j[src]); }
    near(f(&j["native"]["step_px"]["max"]), 1.0, 1e-9, "native step"); assert_eq!(j["fit"]["rows"].as_array().unwrap().len(), N - 1); assert!(j["fit"]["rows"][0]["accel_px"].is_null());
    let jf = c(&mut s, "track_jitter_metrics", json!({"fit_id": "fit1", "source": "fit"})); assert!(jf.get("native").is_none() && jf["fit"].get("rows").is_none());
    assert!(c(&mut s, "track_jitter_metrics", json!({"fit_id": "fit1", "source": "native"})).get("fit").is_none());

    // corner disagreement between the exact native quads and the fit
    let cd = c(&mut s, "corner_disagreement", json!({"fit_id": "fit1", "every": 2}));
    assert_eq!(cd["rows"].as_array().unwrap().len(), 3); for k in 0..4 { assert!(f(&cd["per_corner"][k]["max"]) < 0.1, "{cd}"); near(f(&cd["per_corner"][k]["median"]), 0.0, 0.05, "median"); }
    assert!(c(&mut s, "corner_disagreement", json!({"fit_id": "fit1", "include_rows": false})).get("rows").is_none());

    // per-feature residuals: train / held-out / all partitions of the alive features at frame 5
    let all = c(&mut s, "track_feature_residuals", json!({"fit_id": "fit1", "frame": 5, "set": "all", "top": 400}));
    let tr = c(&mut s, "track_feature_residuals", json!({"fit_id": "fit1", "frame": 5, "set": "train", "top": 1}));
    let ho = c(&mut s, "track_feature_residuals", json!({"fit_id": "fit1", "frame": 5}));
    assert_eq!(f(&all["compared"]), f(&tr["compared"]) + f(&ho["compared"])); assert_eq!(tr["largest"].as_array().unwrap().len(), 1); assert_eq!(ho["set"], "heldout");
    assert!(f(&all["length_stats_px"]["max"]) < 0.1, "{all}");
    for x in all["largest"].as_array().unwrap() { near(f(&x["tracked"][0]) - f(&x["p0"][0]), 5.0, 0.05, "tracked dx"); near(f(&x["predicted"][0]) - f(&x["p0"][0]), 5.0, 0.05, "predicted dx"); }
}

#[test]
fn lk_audit_ecc_refine_and_error_field_render() {
    let (mut s, id) = setup(); track(&mut s, &id); c(&mut s, "fit_track_homography", json!({"corners": ROI}));
    let dir = std::env::temp_dir().join(format!("kimo-trackdiag-out-{}", std::process::id())); let ds = dir.to_str().unwrap().to_string();
    let a = c(&mut s, "lk_residual_audit", json!({"min_distance": 6, "window": 5, "frames": [5], "render_dir": format!("{ds}/")}));
    assert_eq!(a["fit_id"], "fit1"); assert!(a["features_detected"].as_u64().unwrap() >= 8);
    let rows = a["rows"].as_array().unwrap(); assert_eq!(rows.len(), N);
    for r in rows { assert!(r["valid"].as_u64().unwrap() >= 8, "{r}"); assert!(f(&r["max"]) < 0.1, "{r}"); assert_eq!(r["over_px_count"], 0); }
    assert!(f(&a["worst_p95_px"]) < 0.1); let png = format!("{ds}/residual-5.png"); assert_eq!(a["rendered"], json!([png])); assert!(std::path::Path::new(&png).is_file());
    // a tight over_px counts the sub-0.1 px residuals as over
    let a2 = c(&mut s, "lk_residual_audit", json!({"min_distance": 6, "window": 5, "over_px": 0.05, "roi": [0.2, 0.2, 0.8, 0.2, 0.8, 0.8, 0.2, 0.8]}));
    assert!(a2["rows"][0]["valid"].as_u64().unwrap() >= 8); assert_eq!(a2["rendered"], json!([]));

    // ECC from the exact translation stays on it
    let e = c(&mut s, "ecc_refine_track", json!({"fit_id": "fit1", "max_iter": 20}));
    assert_eq!(e["fit_id"], "fit2"); assert_eq!(e["parent_fit"], "fit1"); assert_eq!(e["frames"], N); assert_eq!(e["failed_frames"], json!([]));
    assert_eq!(e["mask_pixels"], 29 * 22, "ROI x 12..68, y 9..51, every 2nd pixel");
    for r in e["rows"].as_array().unwrap() { assert!(f(&r["rho_init"]) > 0.99, "{r}"); if r["status"] != "kept_initial" { assert!(f(&r["rho"]) >= f(&r["rho_init"]) && f(&r["max_corner_move_px"]) < 0.1, "{r}"); } }
    let fi = c(&mut s, "fit_info", json!({"fit_id": "fit2", "frames": [5]})); assert_eq!(fi["kind"], "ecc_refined"); assert_eq!(fi["parent_fit"], "fit1"); assert_eq!(fi["has_ecc"], true);
    near(f(&fi["frames_data"][0]["H_px"][0][2]), 5.0, 0.05, "ECC tx"); assert!(f(&fi["frames_data"][0]["ecc"]["rho"]) > 0.99);
    assert!(c(&mut s, "ecc_refine_track", json!({"fit_id": "fit1", "mask_step": 1, "include_rows": false})).get("rows").is_none());

    // error field: default frames first / third / two thirds / last, PNG + JSON sidecar each
    let od = format!("{ds}/field"); let r = c(&mut s, "render_error_field", json!({"fit_id": "fit1", "out_dir": od, "set": "all", "magnify": 10}));
    let files = r["files"].as_array().unwrap(); assert_eq!(files.iter().map(|x| x["frame"].as_u64().unwrap()).collect::<Vec<_>>(), vec![0, 2, 4, 5]);
    for x in files { assert!(std::path::Path::new(x["png"].as_str().unwrap()).is_file()); let meta: Value = serde_json::from_str(&std::fs::read_to_string(x["meta"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!(meta["magnify"], 10.0); assert_eq!(meta["features_drawn"], x["features_drawn"]); assert!(x["features_drawn"].as_u64().unwrap() >= 8); assert!(f(&meta["residual_px"]["max"]) < 0.1); }
    let img = image_px(&format!("{od}/errorfield-0.png")); assert!(img.iter().any(|p| p[1] > 200 && p[0] < 60 && p[2] < 60), "no green feature dot drawn");
    let rt = c(&mut s, "render_error_field", json!({"out_dir": od, "frames": [3], "set": "train"})); let rh = c(&mut s, "render_error_field", json!({"out_dir": od, "frames": [3]}));
    assert_eq!(rt["fit_id"], "fit3"); assert!(rt["files"][0]["features_drawn"].as_u64().unwrap() > rh["files"][0]["features_drawn"].as_u64().unwrap());
    std::fs::remove_dir_all(&dir).unwrap();
}
fn image_px(p: &str) -> Vec<[u8; 3]> {
    let o = std::process::Command::new("ffmpeg").args(["-v", "error", "-i", p, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]).output().unwrap(); assert!(o.status.success());
    o.stdout.chunks(3).map(|c| [c[0], c[1], c[2]]).collect()
}
