//! track_mask: the OpenCV-ported robust estimators (exact hand-computed values) and the tool's refusals.
use kimo_core::commands::{dispatch, Session};
use kimo_core::robust_est::{self as re, Model};
use serde_json::json;

fn close(a: f64, b: f64, tol: f64, what: &str) { assert!((a - b).abs() <= tol, "{what}: got {a}, want {b} (tol {tol})"); }

#[test] fn ransac_iteration_count_matches_opencv_formula() {
    // ln(1-0.99) / ln(1-(1-0.5)^4) = -4.605170 / -0.064539 = 71.355 -> 71
    assert_eq!(re::ransac_update_num_iters(0.99, 0.5, 4, 2000), 71);
    // ln(0.005) / ln(1-0.7^2) = -5.298317 / -0.673345 = 7.869 -> 8
    assert_eq!(re::ransac_update_num_iters(0.995, 0.3, 2, 2000), 8);
    // no outliers: 1-(1-0)^4 = 0 < DBL_MIN -> 0 (upstream returns 0)
    assert_eq!(re::ransac_update_num_iters(0.99, 0.0, 4, 2000), 0);
    // all outliers: denom = ln(1) = 0 >= 0 -> maxIters
    assert_eq!(re::ransac_update_num_iters(0.99, 1.0, 4, 2000), 2000);
    // p = 1: num = ln(DBL_MIN) = -708.4; 708.4 >= 2000 * 0.0645 -> maxIters
    assert_eq!(re::ransac_update_num_iters(1.0, 0.5, 4, 2000), 2000);
    // clamped inputs behave like the clamp
    assert_eq!(re::ransac_update_num_iters(0.99, 1.7, 4, 500), 500);
}

#[test] fn minimal_kernels_are_exact() {
    // similarity s=2, theta=90 deg, t=(3,-1): q = (-2y+3, 2x-1); (1,2)->(-1,1), (4,-1)->(5,7)
    let h = re::run_kernel(Model::Similarity, &[(1.0, 2.0), (4.0, -1.0)], &[(-1.0, 1.0), (5.0, 7.0)]).unwrap();
    let want = [[0.0, -2.0, 3.0], [2.0, 0.0, -1.0]];
    for i in 0..2 { for j in 0..3 { close(h[i][j], want[i][j], 1e-12, &format!("similarity m[{i}][{j}]")); } }
    // affine [[1.5,0.2,-3],[0.1,0.8,4]]: (0,0)->(-3,4), (10,0)->(12,5), (0,10)->(-1,12)
    let h = re::run_kernel(Model::Affine, &[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)], &[(-3.0, 4.0), (12.0, 5.0), (-1.0, 12.0)]).unwrap();
    let want = [[1.5, 0.2, -3.0], [0.1, 0.8, 4.0]];
    for i in 0..2 { for j in 0..3 { close(h[i][j], want[i][j], 1e-12, &format!("affine m[{i}][{j}]")); } }
    let h = re::run_kernel(Model::Translation, &[(2.0, 3.0)], &[(5.5, -1.25)]).unwrap();
    close(h[0][2], 3.5, 0.0, "tx"); close(h[1][2], -4.25, 0.0, "ty");
    // homography from a unit square to a known quad maps all 4 corners exactly
    let (s4, d4) = ([(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)], [(10.0, 5.0), (120.0, 12.0), (115.0, 130.0), (2.0, 110.0)]);
    let h = re::run_kernel(Model::Perspective, &s4, &d4).unwrap();
    for i in 0..4 { let p = kimo_core::trackmath::apply(&h, s4[i]).unwrap(); close(p.0, d4[i].0, 1e-9, "H x"); close(p.1, d4[i].1, 1e-9, "H y"); }
}

#[test] fn degenerate_subsets_are_rejected_like_opencv() {
    assert!(re::have_collinear_points(&[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]));
    assert!(!re::have_collinear_points(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]));
    assert!(!re::check_subset(Model::Affine, &[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)], &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]));
    let sq = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    assert!(re::check_subset(Model::Perspective, &sq, &sq));
    // a bow-tie (orientation flips on 2 of the 4 triangles): OpenCV's Marquez-Neila consistency test rejects it
    assert!(!re::check_subset(Model::Perspective, &sq, &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]));
    // a mirror (all 4 flip) is geometrically consistent and passes, as upstream
    assert!(re::check_subset(Model::Perspective, &sq, &[(0.0, 0.0), (-1.0, 0.0), (-1.0, 1.0), (0.0, 1.0)]));
}

#[test] fn ransac_recovers_similarity_through_gross_outliers() {
    let (a, b, tx, ty) = (1.1 * 10f64.to_radians().cos(), 1.1 * 10f64.to_radians().sin(), 5.0, -3.0);
    let mut s = vec![]; let mut d = vec![];
    for i in 0..40 { let (x, y) = ((i % 8) as f64 * 13.0 + 7.0, (i / 8) as f64 * 17.0 + 3.0); s.push((x, y)); d.push((a * x - b * y + tx, b * x + a * y + ty)); }
    for i in 0..25 { let (x, y) = (i as f64 * 9.0 + 1.0, 200.0 - i as f64 * 5.0); s.push((x, y)); d.push((x + 40.0 + (i * 7 % 11) as f64 * 9.0, y - 60.0 - (i * 5 % 13) as f64 * 7.0)); }
    let r = re::ransac(Model::Similarity, &s, &d, 1.0, 0.995, 2000, 42).unwrap();
    assert_eq!(r.n_inliers, 40); for i in 0..65 { assert_eq!(r.inliers[i], i < 40, "inlier flag {i}"); }
    close(r.model[0][0], a, 1e-9, "a"); close(r.model[1][0], b, 1e-9, "b"); close(r.model[0][1], -b, 1e-9, "-b"); close(r.model[0][2], tx, 1e-9, "tx"); close(r.model[1][2], ty, 1e-9, "ty");
    assert!(r.iterations < 2000, "adaptive termination should stop early, ran {}", r.iterations);
}

fn session_with_masks() -> (Session, String, String, String) {
    let mut s = Session::new(); dispatch(&mut s, "project_set", &json!({"width": 64, "height": 36, "fps": 24})).unwrap();
    let t = dispatch(&mut s, "add_track", &json!({})).unwrap()["track_id"].clone();
    let cl = dispatch(&mut s, "add_clip", &json!({"track_id": t, "source_type": "solid", "color": [0.5, 0.5, 0.5], "start": 0, "duration": 1})).unwrap()["clip_id"].as_str().unwrap().to_string();
    let pm = dispatch(&mut s, "add_mask", &json!({"clip_id": cl, "kind": "path", "points": [{"x":0.3,"y":0.3},{"x":0.7,"y":0.3},{"x":0.7,"y":0.7},{"x":0.3,"y":0.7}]})).unwrap()["mask_id"].as_str().unwrap().to_string();
    let em = dispatch(&mut s, "add_mask", &json!({"clip_id": cl, "kind": "ellipse"})).unwrap()["mask_id"].as_str().unwrap().to_string();
    (s, cl, pm, em)
}

#[test] fn track_mask_refusals_name_the_problem() {
    let (mut s, cl, pm, em) = session_with_masks();
    let e = dispatch(&mut s, "track_mask", &json!({"clip_id": cl, "mask_id": pm, "motion": "wobble"})).unwrap_err();
    assert!(e.contains("unknown motion 'wobble'") && e.contains("translation, similarity, affine, perspective"), "{e}");
    let e = dispatch(&mut s, "track_mask", &json!({"clip_id": cl, "mask_id": em})).unwrap_err();
    assert!(e.contains("needs a path mask"), "{e}");
    let e = dispatch(&mut s, "track_mask", &json!({"clip_id": cl, "mask_id": "nope"})).unwrap_err();
    assert!(e.contains("no such mask"), "{e}");
    let e = dispatch(&mut s, "track_mask", &json!({"clip_id": cl, "mask_id": pm})).unwrap_err();
    assert!(e.contains("media"), "a solid clip has no footage to track: {e}");
}

#[test] fn apply_to_path_moves_vertices_tangents_and_scales_feather() {
    use kimo_core::masks::PathPt;
    let pts = vec![PathPt { x: 0.25, y: 0.5, ix: -0.05, iy: 0.0, ox: 0.05, oy: 0.0, f: 10.0, ..Default::default() }];
    // scale 2 about the origin + translate (10, 20) at 200x100
    let h = [[2.0, 0.0, 10.0], [0.0, 2.0, 20.0], [0.0, 0.0, 1.0]];
    let o = kimo_core::mask_track::apply_to_path(&pts, &h, 200, 100);
    close(o[0].x, (0.25 * 200.0 * 2.0 + 10.0) / 200.0, 1e-12, "x"); close(o[0].y, (0.5 * 100.0 * 2.0 + 20.0) / 100.0, 1e-12, "y");
    close(o[0].ox, 0.1, 1e-12, "ox"); close(o[0].ix, -0.1, 1e-12, "ix"); close(o[0].f, 20.0, 1e-12, "feather x sqrt(det)=2");
}

// ---------- track_region on in-code frames: frame k shows the analytic texture through a known motion H_k ----------
use kimo_core::mask_track::{self as mt, Opts};
use kimo_core::trackmath::{self as tm, H3, P};
const RW: usize = 64; const RH: usize = 48;
fn tex(x: f64, y: f64) -> f64 { 128.0 + 50.0 * (0.45 * x + 0.2).sin() * (0.38 * y + 0.4).sin() + 30.0 * (0.17 * x - 0.23 * y).sin() + 20.0 * (0.11 * x + 0.29 * y + 1.0).cos() }
/// Frame showing the reference texture moved by `hk` (reference pixel -> frame pixel); pixel (i, j) samples its centre (i+0.5, j+0.5).
fn frame_of(hk: &H3) -> Vec<f32> { let hi = tm::inv3(hk).unwrap(); (0..RW * RH).map(|i| { let q = tm::apply(&hi, ((i % RW) as f64 + 0.5, (i / RW) as f64 + 0.5)).unwrap(); tex(q.0, q.1) as f32 }).collect() }
fn about_centre(a: f64, b: f64, c: f64, d: f64, tx: f64, ty: f64, g: f64, h: f64) -> H3 { let (cx, cy) = (32.0, 24.0); let m = [[a, b, tx], [c, d, ty], [g, h, 1.0]];
    tm::mul(&tm::mul(&[[1.0, 0.0, cx], [0.0, 1.0, cy], [0.0, 0.0, 1.0]], &m), &[[1.0, 0.0, -cx], [0.0, 1.0, -cy], [0.0, 0.0, 1.0]]) }
const SQ: [P; 4] = [(18.0, 13.0), (46.0, 13.0), (46.0, 35.0), (18.0, 35.0)];
fn run(model: Model, hs: &[H3], o: &Opts) -> Result<mt::RegionTrack, String> {
    let frames: Vec<Vec<f32>> = hs.iter().map(frame_of).collect(); let mut get = |i: usize| -> Result<Vec<f32>, String> { Ok(frames[i].clone()) };
    mt::track_region(&mut get, &(0..hs.len()).collect::<Vec<_>>(), RW, RH, &SQ, model, o)
}
fn corner_err(a: &H3, b: &H3) -> f64 { SQ.iter().map(|p| tm::dist(tm::apply(a, *p).unwrap(), tm::apply(b, *p).unwrap())).fold(0.0, f64::max) }

#[test] fn track_region_recovers_each_motion_model_exactly() {
    let cases: [(Model, Box<dyn Fn(f64) -> H3>); 4] = [
        (Model::Translation, Box::new(|k| about_centre(1.0, 0.0, 0.0, 1.0, 1.3 * k, -0.7 * k, 0.0, 0.0))),
        (Model::Similarity, Box::new(|k| { let (s, a) = (1.0 + 0.015 * k, (2.0 * k).to_radians()); about_centre(s * a.cos(), -s * a.sin(), s * a.sin(), s * a.cos(), 0.8 * k, 0.4 * k, 0.0, 0.0) })),
        (Model::Affine, Box::new(|k| about_centre(1.0 + 0.02 * k, 0.03 * k, -0.01 * k, 1.0 - 0.015 * k, -0.6 * k, 0.9 * k, 0.0, 0.0))),
        (Model::Perspective, Box::new(|k| about_centre(1.0 + 0.01 * k, 0.01 * k, 0.0, 1.0, 0.5 * k, 0.3 * k, 4e-4 * k, -3e-4 * k))),
    ];
    for (m, hk) in cases.iter() {
        let hs: Vec<H3> = (0..4).map(|k| hk(k as f64)).collect(); let r = run(*m, &hs, &Opts::default()).unwrap_or_else(|e| panic!("{}: {e}", m.name()));
        assert_eq!(r.h.len(), 4); assert_eq!(r.h[0], mt::IDENTITY); assert_eq!(r.stats.len(), 4); assert_eq!(r.stats[0].frame, 0); assert!(r.warnings.is_empty(), "{}: {:?}", m.name(), r.warnings);
        for k in 1..4 { let e = corner_err(&r.h[k], &hs[k]); assert!(e < 0.1, "{} frame {k}: corner error {e} px", m.name()); assert!(r.stats[k].refined && r.stats[k].rms < 1.0, "{} {:?}", m.name(), r.stats[k]); assert!(r.stats[k].inliers >= m.min_points()); }
    }
}

fn plate(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("kimo-trackmask-{tag}-{}.mkv", std::process::id()));
    let t = "128+45*sin(0.55*(X-N))*sin(0.47*Y+0.3)+30*sin(0.21*(X-N)+0.33*Y)+20*cos(0.13*(X-N)-0.29*Y+1)";
    let g = format!("nullsrc=s=64x48:r=10:d=0.5,format=rgb24,geq=r='{t}':g='{t}':b='{t}'");
    assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", &g, "-c:v", "ffv1", "-pix_fmt", "bgr0", p.to_str().unwrap()]).status().unwrap().success()); p
}
#[test] fn track_mask_tool_moves_the_path_with_the_footage_forward_and_backward() {
    let p = plate("tool"); let mut s = Session::new(); dispatch(&mut s, "project_set", &json!({"width": 64, "height": 48, "fps": 10})).unwrap();
    let a = dispatch(&mut s, "import_media", &json!({"path": p})).unwrap()["asset_id"].clone(); let t = dispatch(&mut s, "add_track", &json!({})).unwrap()["track_id"].clone();
    let cl = dispatch(&mut s, "add_clip", &json!({"track_id": t, "source_type": "media", "asset_id": a, "start": 0, "duration": 0.5})).unwrap()["clip_id"].as_str().unwrap().to_string();
    let pm = dispatch(&mut s, "add_mask", &json!({"clip_id": cl, "kind": "path", "points": [{"x":0.25,"y":0.25},{"x":0.75,"y":0.25},{"x":0.75,"y":0.75},{"x":0.25,"y":0.75}]})).unwrap()["mask_id"].as_str().unwrap().to_string();
    let r = dispatch(&mut s, "track_mask", &json!({"clip_id": cl, "mask_id": pm, "motion": "translation", "end": 0.4})).unwrap();
    assert_eq!(r["keys_written"], 5); assert_eq!(r["frames"], json!([0, 4])); assert_eq!(r["motion"], "translation"); assert_eq!(r["refined_frames"], 4);
    let keys = |s: &Session| s.project.find_clip(&cl).unwrap().mask.as_ref().unwrap().layers.iter().find(|l| l.id == pm).unwrap().path_keys.clone();
    let k = keys(&s); assert_eq!(k.len(), 5);
    for (i, key) in k.iter().enumerate() { close(key.t, i as f64 / 10.0, 1e-12, "key time"); close(key.points[0].x, 0.25 + i as f64 / 64.0, 0.002, &format!("x at frame {i}")); close(key.points[0].y, 0.25, 0.002, &format!("y at frame {i}")); }
    // backwards from frame 4 to frame 2: the keys inside [0.2, 0.4] are replaced, the earlier ones kept
    let rb = dispatch(&mut s, "track_mask", &json!({"clip_id": cl, "mask_id": pm, "motion": "similarity", "start": 0.4, "end": 0.2, "refine": false})).unwrap();
    assert_eq!(rb["frames"], json!([4, 2])); assert_eq!(rb["keys_written"], 3); assert_eq!(rb["refined_frames"], 0);
    // a single frame: nothing to track, one key, zero statistics
    let r1 = dispatch(&mut s, "track_mask", &json!({"clip_id": cl, "mask_id": pm, "start": 0.2, "end": 0.2})).unwrap(); assert_eq!((r1["keys_written"].as_u64(), r1["mean_inliers"].as_f64(), r1["mean_rms"].as_f64(), r1["min_inliers"].as_u64()), (Some(1), Some(0.0), Some(0.0), Some(0)));
    let k2 = keys(&s); assert_eq!(k2.len(), 5); for i in 0..5 { close(k2[i].points[0].x, 0.25 + i as f64 / 64.0, 0.005, &format!("after backward pass, frame {i}")); }
    std::fs::remove_file(p).unwrap();
}
