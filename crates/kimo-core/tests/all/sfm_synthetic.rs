use kimo_core::sfm::*;
fn gauss(r: &mut Rng) -> f64 { let u = (r.next() % 1_000_000) as f64 / 1e6 + 1e-7; let v = (r.next() % 1_000_000) as f64 / 1e6; (-2.0 * u.ln()).sqrt() * (6.2831853 * v).cos() }
fn scene(nf: usize, noise: f64, outl: f64, seed: u64, f: f64) -> (Vec<Track>, Vec<Pose>, Vec<V3>) { scene_d(nf, noise, outl, seed, f, 0.0, 0.0) }
fn scene_d(nf: usize, noise: f64, outl: f64, seed: u64, f: f64, k1: f64, k2: f64) -> (Vec<Track>, Vec<Pose>, Vec<V3>) {
    let mut rng = Rng(seed); let (w, h) = (1280.0, 720.0); let mut pts = vec![];
    for _ in 0..400 { let u = |r: &mut Rng| (r.next() % 100000) as f64 / 100000.0; pts.push([(u(&mut rng) - 0.5) * 10.0, (u(&mut rng) - 0.5) * 6.0, 6.0 + u(&mut rng) * 10.0]); }
    let mut poses = vec![]; for i in 0..nf { let s = i as f64 / (nf - 1) as f64; let c = [-1.5 + 3.0 * s, 0.3 * (s * 6.0).sin(), -0.8 * s * s]; let r = exp_so3(&[0.02 * (s * 4.0).sin(), -0.25 * (s - 0.5), 0.03 * s]);
        let t = { let v = mv(&r, &c); [-v[0], -v[1], -v[2]] }; poses.push(Pose { r, t }); }
    let mut tracks = vec![]; for (j, x) in pts.iter().enumerate() { let mut ob = vec![]; for (i, p) in poses.iter().enumerate() { let c = p.apply(x); if c[2] < 0.5 { continue; } let (xa, xb) = (c[0] / c[2], c[1] / c[2]); let r2 = xa * xa + xb * xb; let sc = 1.0 + k1 * r2 + k2 * r2 * r2; let (u, v) = (f * xa * sc + w / 2.0, f * xb * sc + h / 2.0);
        if u < 0.0 || v < 0.0 || u > w || v > h { continue; } if (j * 7 + i * 3) % 11 == 0 { continue; } let bad = (rng.next() % 1000) as f64 / 1000.0 < outl; let (nx, ny) = if bad { (gauss(&mut rng) * 30.0, gauss(&mut rng) * 30.0) } else { (gauss(&mut rng) * noise, gauss(&mut rng) * noise) }; ob.push((i, u + nx, v + ny)); }
        if ob.len() >= 4 { tracks.push(Track { obs: ob }); } }
    (tracks, poses, pts)
}
fn eval(sol: &Solution, gt: &[Pose]) -> (f64, f64) {
    // similarity align solved camera centers to GT (Umeyama), then report rms position error relative to path length and mean rotation error (deg) after global rotation alignment
    let idx: Vec<usize> = (0..gt.len()).filter(|&i| sol.poses[i].is_some()).collect(); let n = idx.len() as f64;
    let a: Vec<V3> = idx.iter().map(|&i| sol.poses[i].unwrap().center()).collect(); let b: Vec<V3> = idx.iter().map(|&i| gt[i].center()).collect();
    let ma = [0, 1, 2].map(|k| a.iter().map(|p| p[k]).sum::<f64>() / n); let mb = [0, 1, 2].map(|k| b.iter().map(|p| p[k]).sum::<f64>() / n);
    let mut cov = [[0.0; 3]; 3]; for (p, q) in a.iter().zip(&b) { for i in 0..3 { for j in 0..3 { cov[i][j] += (q[i] - mb[i]) * (p[j] - ma[j]) / n; } } }
    let (u, s, v) = svd3(&cov).unwrap(); let mut d = ID3; if det3(&u) * det3(&v) < 0.0 { d[2][2] = -1.0; } let r = mm(&mm(&u, &d), &tp(&v));
    let va: f64 = a.iter().map(|p| (0..3).map(|k| (p[k] - ma[k]).powi(2)).sum::<f64>()).sum::<f64>() / n; let sc = (s[0] + s[1] + d[2][2] * s[2]) / va;
    let mut e = 0.0; for (p, q) in a.iter().zip(&b) { let pc = [p[0] - ma[0], p[1] - ma[1], p[2] - ma[2]]; let m = mv(&r, &pc); for k in 0..3 { e += (sc * m[k] + mb[k] - q[k]).powi(2); } } let pos = (e / n).sqrt();
    let mut rot = 0.0; for &i in &idx { let rc = mm(&tp(&sol.poses[i].unwrap().r), &ID3); let g = tp(&gt[i].r); let rel = mm(&r, &rc); let dr = mm(&tp(&g), &rel); rot += log_so3(&dr).iter().map(|x| x * x).sum::<f64>().sqrt().to_degrees(); } (pos, rot / n)
}
#[test] fn solves_known_camera_path() {
    let f = 1100.0; let (tracks, gt, _) = scene(60, 0.4, 0.03, 42, f);
    let sol = solve(&tracks, 60, 1280.0, 720.0, &SolveOpts { focal_px: Some(f), ..Default::default() }).expect("solve");
    let (pe, re) = eval(&sol, &gt); eprintln!("known-f: reg {}/60 rms {:.3}px pos_err {:.4} (path 3.0) rot_err {:.3} deg focal {:.1}", sol.registered, sol.rms, pe, re, sol.intr.f); for n in &sol.notes { eprintln!("  {n}"); }
    assert!(sol.registered >= 58 && sol.rms < 1.0 && pe < 0.05 && re < 0.5);
}
#[test] fn estimates_unknown_focal() {
    let f = 1100.0; let (tracks, gt, _) = scene(60, 0.4, 0.03, 9, f);
    let sol = solve(&tracks, 60, 1280.0, 720.0, &SolveOpts::default()).expect("solve");
    let (pe, re) = eval(&sol, &gt); eprintln!("unknown-f: reg {}/60 rms {:.3}px pos_err {:.4} rot_err {:.3} deg focal {:.1} (true {f})", sol.registered, sol.rms, pe, re, sol.intr.f); for n in &sol.notes { eprintln!("  {n}"); }
    assert!(sol.registered >= 58 && sol.rms < 1.0 && (sol.intr.f - f).abs() / f < 0.05 && re < 1.0);
}
#[test] fn rejects_pure_rotation() {
    let mut rng = Rng(5); let f = 1000.0; let mut tracks = vec![]; for _ in 0..200 { let x = [((rng.next() % 1000) as f64 / 500.0 - 1.0), ((rng.next() % 1000) as f64 / 500.0 - 1.0), 1.0]; let mut ob = vec![]; for i in 0..20 { let r = exp_so3(&[0.0, 0.01 * i as f64, 0.0]); let c = mv(&r, &x); ob.push((i, f * c[0] / c[2] + 640.0, f * c[1] / c[2] + 360.0)); } tracks.push(Track { obs: ob }); }
    assert!(solve(&tracks, 20, 1280.0, 720.0, &SolveOpts { focal_px: Some(f), ..Default::default() }).is_err());
}

#[test] fn solves_lens_distortion() {
    let f = 1100.0; let (tracks, gt, _) = scene_d(60, 0.4, 0.03, 21, f, -0.15, 0.04);
    let plain = solve(&tracks, 60, 1280.0, 720.0, &SolveOpts::default()).expect("solve"); let (pp, rp) = eval(&plain, &gt);
    let sol = solve(&tracks, 60, 1280.0, 720.0, &SolveOpts { refine_dist: true, ..Default::default() }).expect("solve");
    let (pe, re) = eval(&sol, &gt); eprintln!("no-distortion-model: rms {:.3}px pos {:.4} rot {:.3} deg\ndistortion model: rms {:.3}px pos {:.4} rot {:.3} deg focal {:.1} k1 {:.4} k2 {:.4} (true -0.15 0.04)", plain.rms, pp, rp, sol.rms, pe, re, sol.intr.f, sol.intr.k1, sol.intr.k2);
    assert!(sol.rms < 1.0 && sol.rms < plain.rms && (sol.intr.k1 + 0.15).abs() < 0.04 && re < 1.0 && re < rp * 0.5 && (sol.intr.f - f).abs() / f < 0.06);
}
#[test] fn distortion_model_is_harmless_without_distortion() {
    let f = 1100.0; let (tracks, gt, _) = scene(60, 0.4, 0.03, 5, f); let sol = solve(&tracks, 60, 1280.0, 720.0, &SolveOpts { refine_dist: true, ..Default::default() }).expect("solve"); let (pe, re) = eval(&sol, &gt);
    eprintln!("undistorted data with distortion model on: rms {:.3} pos {:.4} rot {:.3} k1 {:.4} k2 {:.4}", sol.rms, pe, re, sol.intr.k1, sol.intr.k2); assert!(sol.rms < 1.0 && re < 1.0 && sol.intr.k1.abs() < 0.03);
}
#[test] fn solves_pure_pan_rotation_only() {
    let (w, h, f, nf) = (1280.0, 720.0, 900.0, 50usize); let (k1, k2) = (-0.08, 0.0); let mut rng = Rng(77); let mut dirs = vec![]; for _ in 0..500 { let u = |r: &mut Rng| (r.next() % 100000) as f64 / 100000.0; let d = [(u(&mut rng) - 0.5) * 3.0, (u(&mut rng) - 0.5) * 1.6, 1.0]; let l = norm(&d); dirs.push([d[0] / l * 50.0, d[1] / l * 50.0, d[2] / l * 50.0]); }
    let gt: Vec<Pose> = (0..nf).map(|i| { let s = i as f64 / (nf - 1) as f64; Pose { r: exp_so3(&[0.05 * (s * 3.0).sin(), -0.5 * (s - 0.5), 0.04 * s]), t: [0.0; 3] } }).collect();
    let mut tracks = vec![]; for (j, x) in dirs.iter().enumerate() { let mut ob = vec![]; for (i, p) in gt.iter().enumerate() { let c = p.apply(x); if c[2] < 1.0 { continue; } let (a, b) = (c[0] / c[2], c[1] / c[2]); let r2 = a * a + b * b; let sc = 1.0 + k1 * r2 + k2 * r2 * r2; let (u, v) = (f * a * sc + w / 2.0, f * b * sc + h / 2.0); if u < 0.0 || v < 0.0 || u > w || v > h || (j * 5 + i) % 13 == 0 { continue; } let bad = rng.next() % 100 < 3; let (nx, ny) = if bad { (gauss(&mut rng) * 25.0, gauss(&mut rng) * 25.0) } else { (gauss(&mut rng) * 0.4, gauss(&mut rng) * 0.4) }; ob.push((i, u + nx, v + ny)); } if ob.len() >= 4 { tracks.push(Track { obs: ob }); } }
    assert!(solve(&tracks, nf, w, h, &SolveOpts::default()).is_err(), "full 3D must refuse a nodal pan");
    let sol = solve_auto(&tracks, nf, w, h, &SolveOpts { refine_dist: true, ..Default::default() }, "auto").expect("auto falls back"); assert_eq!(sol.mode, "rotation_only");
    // relative orientation error (global rotation aligned on frame 0)
    let mut worst: f64 = 0.0; let mut sum = 0.0; let r0 = sol.poses[0].unwrap().r; for i in 0..nf { let rs = mm(&sol.poses[i].unwrap().r, &tp(&r0)); let rg = mm(&gt[i].r, &tp(&gt[0].r)); let e = log_so3(&mm(&tp(&rg), &rs)); let d = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt().to_degrees(); worst = worst.max(d); sum += d; }
    eprintln!("rotation-only: reg {}/{nf} rms {:.3}px mean rot err {:.3} deg max {:.3} focal {:.1} (true {f}) k1 {:.4} (true {k1})", sol.registered, sol.rms, sum / nf as f64, worst, sol.intr.f, sol.intr.k1);
    for n in &sol.notes { eprintln!("  {n}"); }
    assert!(sol.registered >= 48 && sol.rms < 1.0 && worst < 0.3 && (sol.intr.f - f).abs() / f < 0.03);
}

// ---------- fast tiny-scene tests (noise-free, 320x240, 8 frames, 60 points): exact recovery up to the similarity gauge ----------
fn tiny_scene(nf: usize, f: f64, rot_only: bool) -> (Vec<Track>, Vec<Pose>) {
    let mut rng = Rng(1234); let (w, h) = (320.0, 240.0); let u = |r: &mut Rng| (r.next() % 100000) as f64 / 100000.0;
    let pts: Vec<V3> = (0..70).map(|_| { let p = [(u(&mut rng) - 0.5) * 8.0, (u(&mut rng) - 0.5) * 6.0, 5.0 + u(&mut rng) * 10.0]; if rot_only { let l = norm(&p); [p[0] / l * 50.0, p[1] / l * 50.0, p[2] / l * 50.0] } else { p } }).collect();
    let poses: Vec<Pose> = (0..nf).map(|i| { let s = i as f64 / (nf - 1) as f64; let r = exp_so3(&[0.01 * s, if rot_only { -0.2 * (s - 0.5) } else { -0.05 * (s - 0.5) }, 0.02 * s]);
        let c = if rot_only { [0.0; 3] } else { [-2.0 + 4.0 * s, 0.3 * (3.0 * s).sin(), -0.5 * s * s] }; let v = mv(&r, &c); Pose { r, t: [-v[0], -v[1], -v[2]] } }).collect();
    let mut tracks = vec![]; for (j, x) in pts.iter().enumerate() { let mut ob = vec![]; for (i, p) in poses.iter().enumerate() { let c = p.apply(x); if c[2] < 0.5 { continue; } let (a, b) = (f * c[0] / c[2] + w / 2.0, f * c[1] / c[2] + h / 2.0);
        if a < 0.0 || b < 0.0 || a > w || b > h || (j * 7 + i * 3) % 11 == 0 { continue; } ob.push((i, a, b)); } if ob.len() >= 4 { tracks.push(Track { obs: ob }); } }
    (tracks, poses)
}
#[test] fn tiny_scene_known_focal_is_recovered_exactly() {
    let (tracks, gt) = tiny_scene(8, 300.0, false);
    let sol = solve(&tracks, 8, 320.0, 240.0, &SolveOpts { focal_px: Some(300.0), refine_focal: false, max_keyframes: 4, ..Default::default() }).expect("solve");
    let (pe, re) = eval(&sol, &gt); assert_eq!(sol.mode, "full"); assert_eq!(sol.registered, 8, "{:?}", sol.notes);
    assert!(sol.rms < 1e-3 && pe < 1e-3 && re < 0.01, "rms {} pos {pe} rot {re}", sol.rms); assert_eq!(sol.intr.f, 300.0);
    assert_eq!(sol.points.len(), sol.point_track.len()); assert!(sol.frame_rms.iter().all(|e| *e < 1e-3));
    assert!(sol.notes.iter().any(|n| n.starts_with("init pair frames 0-")) && sol.notes.last().unwrap().starts_with("8/8 frames registered"));
    // every solved point reprojects onto its observations
    for (x, &t) in sol.points.iter().zip(&sol.point_track) { for &(f, u, v) in &tracks[t].obs { let p = project(sol.poses[f].as_ref().unwrap(), x, &sol.intr).unwrap(); assert!((p.0 - u).abs() < 1e-2 && (p.1 - v).abs() < 1e-2); } }
}
#[test] fn tiny_scene_unknown_focal_is_estimated() {
    let (tracks, gt) = tiny_scene(8, 300.0, false);
    let sol = solve(&tracks, 8, 320.0, 240.0, &SolveOpts { max_keyframes: 4, ..Default::default() }).expect("solve");
    let (pe, re) = eval(&sol, &gt); assert!((sol.intr.f - 300.0).abs() < 1.0 && sol.rms < 0.01 && pe < 0.01 && re < 0.05, "f {} rms {} pos {pe} rot {re}", sol.intr.f, sol.rms);
}
#[test] fn tiny_nodal_pan_falls_back_to_rotation_only() {
    let (tracks, gt) = tiny_scene(6, 280.0, true); let o = SolveOpts { focal_px: Some(280.0), refine_focal: false, max_keyframes: 3, ..Default::default() };
    let e = solve(&tracks, 6, 320.0, 240.0, &o).err().unwrap(); assert!(e.contains("no usable initial pair"), "{e}");
    let e = solve_auto(&tracks, 6, 320.0, 240.0, &o, "full").err().unwrap(); assert!(e.contains("no usable initial pair"), "{e}");
    let sol = solve_auto(&tracks, 6, 320.0, 240.0, &o, "auto").expect("auto falls back"); assert_eq!(sol.mode, "rotation_only"); assert_eq!(sol.registered, 6);
    assert!(sol.notes[0].contains("fell back to rotation-only")); assert!(sol.rms < 1e-3, "rms {}", sol.rms);
    let r0 = sol.poses[0].unwrap().r; for i in 0..6 { let rs = mm(&sol.poses[i].unwrap().r, &tp(&r0)); let rg = mm(&gt[i].r, &tp(&gt[0].r)); let e = log_so3(&mm(&tp(&rg), &rs)); assert!(norm(&e).to_degrees() < 0.01, "frame {i}"); assert_eq!(sol.poses[i].unwrap().t, [0.0; 3]); }
    for d in &sol.points { assert!((norm(d) - 1.0).abs() < 1e-9, "points are unit directions"); }
    // the explicit rotation mode, with the focal estimated
    let sr = solve_auto(&tracks, 6, 320.0, 240.0, &SolveOpts { max_keyframes: 3, ..Default::default() }, "rotation").unwrap(); assert!((sr.intr.f - 280.0).abs() < 2.0, "focal {}", sr.intr.f);
}
#[test] fn tiny_scene_distortion_and_tangential_models_stay_at_zero_on_a_pinhole() {
    let (tracks, gt) = tiny_scene(8, 300.0, false);
    let sol = solve(&tracks, 8, 320.0, 240.0, &SolveOpts { focal_px: Some(300.0), refine_focal: false, refine_dist: true, tangential: true, max_keyframes: 4, ..Default::default() }).expect("solve");
    let (pe, re) = eval(&sol, &gt); assert!(sol.rms < 0.01 && pe < 0.01 && re < 0.05, "rms {} pos {pe} rot {re}", sol.rms);
    assert!(sol.intr.k1.abs() < 2e-3 && sol.intr.k2.abs() < 2e-3 && sol.intr.p1.abs() < 1e-4 && sol.intr.p2.abs() < 1e-4, "{:?}", sol.intr);
    assert!(sol.notes.iter().any(|n| n.starts_with("distortion multi-start picked")) && sol.notes.iter().any(|n| n.starts_with("tangential: all-frame BA")), "{:?}", sol.notes);
}
