//! Tool definitions, part 5 of 5: a verbatim slice of the registry table (order is preserved, see `build_registry`).
#![allow(unused_imports)]
use super::*;

pub(super) fn tools(eff_kinds: &[&str]) -> Vec<Command> {
    vec![
    Command { name: "smooth_curve", description: "Graph editor: Gaussian-smooth a keyframed property's values (sigma in keys). Use on noisy tracker output or hand-keyed jitter. Keeps key times and key count. Edges are replicated, so the first and last values move too unless pin_ends=true. A very large sigma flattens the curve to roughly the average of its end values.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("property", s("property")), ("sigma", n("keys, default 2")), ("pin_ends", json!({"type":"boolean","description":"keep the first and last key values untouched, default false"}))], &["clip_id","property"]),
        run: |ss, a| { let sg = opt_num(a, "sigma", 2.0).max(0.3); let pin = a.get("pin_ends").and_then(|v| v.as_bool()).unwrap_or(false); let c = clip(&mut ss.project, a)?; let pr = c.props.get_mut(str_arg(a, "property")?).ok_or("no such property")?; let v: Vec<f64> = pr.keys.iter().map(|k| k.v).collect(); let n = v.len() as i64; let r = (sg * 3.0).ceil() as i64;
            for (i, k) in pr.keys.iter_mut().enumerate() { let (mut s0, mut w0) = (0.0, 0.0); for d in -r..=r { let j = (i as i64 + d).clamp(0, n - 1); let w = (-(d * d) as f64 / (2.0 * sg * sg)).exp(); s0 += v[j as usize] * w; w0 += w; } k.v = s0 / w0; }
            if pin && n >= 1 { pr.keys[0].v = v[0]; let l = (n - 1) as usize; pr.keys[l].v = v[l]; } Ok(json!({"ok":true,"keys":n,"pin_ends":pin})) } },
    Command { name: "set_text_path", description: "Lay a text clip's characters along a path or a preset shape, each glyph rotated to follow it. Text on a path (AE Path Options): lays the text's characters along a smooth path (points normalized 0..1, or shape circle|ellipse|figure8|wave|arc|spiral with cx,cy,rx,ry) with each glyph rotated to the tangent. `start` (0..1 progress of the first character) can be animated with animate {start,end,from,to,ease}; flip reverses direction. Combine with add_text_animator for per-character effects along the path. remove=true clears it.", mutates: true,
        schema: obj(vec![("clip_id", s("text clip")), ("shape", s("preset shape")), ("points", json!({"type":"array","items":{"type":"array","items":{"type":"number"}}})), ("cx", n("")), ("cy", n("")), ("rx", n("")), ("ry", n("")), ("start", n("initial progress 0..1")), ("flip", json!({"type":"boolean"})), ("animate", json!({"type":"object"})), ("remove", json!({"type":"boolean"}))], &["clip_id"]),
        run: |ss, a| { let rm = a.get("remove").and_then(|v| v.as_bool()).unwrap_or(false); if rm { let c = clip(&mut ss.project, a)?; if let Some(fx) = c.text_fx.as_mut() { fx.path = None; } return Ok(json!({"ok":true})); }
            let (p, cl): (Vec<(f64, f64)>, bool) = if let Some(sh) = a.get("shape").and_then(|v| v.as_str()) { crate::motion::preset(sh, opt_num(a, "cx", 0.5), opt_num(a, "cy", 0.5), opt_num(a, "rx", 0.3), opt_num(a, "ry", 0.25)).ok_or("bad shape")? } else if let Some(v) = a.get("points") { (pt_arr(v).into_iter().map(|q| (q[0], q[1])).collect(), false) } else { return Err("give shape or points".into()) };
            if p.len() < 2 { return Err("need at least 2 points".into()); } let pts = crate::motion::smooth_points(&p, cl, 1.0);
            let mp = crate::motion::MotionPath { points: pts, closed: cl, auto_orient: false, orient_offset: 0.0, constant_speed: true, orient_smooth: 0.0 };
            let c = clip(&mut ss.project, a)?; let dur = c.duration;
            // the ease is read before the clip changes, so a bad one is refused with the clip untouched
            let anim = match a.get("animate") { Some(an) => Some((an, serde_json::from_value::<Ease>(an.get("ease").cloned().unwrap_or(json!("in_out"))).map_err(|e| e.to_string())?)), None => None };
            let fx = c.text_fx.get_or_insert_with(TextFx::default); fx.path = Some(mp); fx.path_start = Prop::new(opt_num(a, "start", 0.0)); fx.path_flip = a.get("flip").and_then(|v| v.as_bool()).unwrap_or(false);
            if let Some((an, e)) = anim { let g = |k: &str, d: f64| an.get(k).and_then(|v| v.as_f64()).unwrap_or(d); fx.path_start.set_key(g("start", 0.0), g("from", 0.0), e.clone()); fx.path_start.set_key(g("end", dur), g("to", 1.0), e); }
            Ok(json!({"ok":true})) } },
    Command { name: "track_beats", description: "Estimate BPM and beat times from an audio or video asset. Beat tracker: estimates BPM and beat times (seconds, in the asset's own timeline plus `t0`) from an audio/video asset by onset-envelope autocorrelation with phase lock. Use the result with bind_beats (the older detect_beats takes a file path and returns cut points) to drive motion. Range limits via min_bpm/max_bpm (default 70-180). Tier: good on clear percussive music, unreliable on ambient or rubato audio; check with the returned beat list against what you hear.", mutates: false,
        schema: obj(vec![("asset_id", s("imported asset")), ("t0", n("start seconds, default 0")), ("duration", n("seconds to analyse, default 30")), ("min_bpm", n("")), ("max_bpm", n(""))], &["asset_id"]),
        run: |ss, a| { let id = str_arg(a, "asset_id")?; let path = ss.project.resolve(&ss.project.assets.get(id).ok_or("no such asset")?.path); let (bpm, beats, conf) = crate::motion::detect_beats(&path, opt_num(a, "t0", 0.0), opt_num(a, "duration", 30.0), opt_num(a, "min_bpm", 70.0), opt_num(a, "max_bpm", 180.0))?; Ok(json!({"bpm": bpm, "beats": beats, "count": beats.len(), "confidence": conf})) } },
    Command { name: "bind_beats", description: "Beat-driven motion (kimo exclusive): keyframe a clip property so it hits on every beat. Give `bpm` (+ `offset` seconds of first beat) or an explicit `beats` list (e.g. from track_beats, timeline seconds). Each beat jumps the property by `amount` (added to its current base value) and decays back over `decay` seconds with the chosen curve: pulse (instant hit, exponential decay), kick (fast attack, ease out), swing (alternates +amount/-amount on successive beats), flash. Works on any numeric prop: scale, rotation, opacity, x, y, or an effect param via property name. Replaces keys on that property in the covered span.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("property", s("property e.g. scale, rotation, x, opacity")), ("bpm", n("")), ("offset", n("time of first beat, clip-local seconds, default 0")), ("beats", json!({"type":"array","items":{"type":"number"}})), ("amount", n("peak change, default 0.1")), ("decay", n("seconds, default 0.25 of a beat")), ("curve", json!({"type":"string","enum":["pulse","kick","swing","flash"]})), ("every", n("act on every Nth beat, default 1"))], &["clip_id","property"]),
        run: |ss, a| { let fps = ss.project.fps; let c = clip(&mut ss.project, a)?; let dur = c.duration; let prop = str_arg(a, "property")?.to_string(); let base = c.props.get(&prop).map(|p| p.base).unwrap_or(0.0);
            let mut beats: Vec<f64> = if let Some(b) = a.get("beats").and_then(|v| v.as_array()) { b.iter().filter_map(|v| v.as_f64()).map(|t| t - c.start).collect() } else { let bpm = num_arg(a, "bpm")?; let per = 60.0 / bpm; let mut v = vec![]; let mut t = opt_num(a, "offset", 0.0); while t < dur { if t >= 0.0 { v.push(t); } t += per; } v };
            beats.retain(|t| *t >= 0.0 && *t < dur); let per = if beats.len() > 1 { (beats[beats.len() - 1] - beats[0]) / (beats.len() - 1) as f64 } else { 0.5 }; let amt = opt_num(a, "amount", 0.1); let decay = opt_num(a, "decay", 0.25 * per).max(1.0 / fps); let every = opt_num(a, "every", 1.0).max(1.0) as usize; let curve = a.get("curve").and_then(|v| v.as_str()).unwrap_or("pulse").to_string();
            let mut pr = Prop::new(base); let sel: Vec<(usize, f64)> = beats.iter().cloned().enumerate().filter(|(i, _)| i % every == 0).collect(); let mut nk = 0;
            let val = |t: f64, bi: usize| -> f64 { let mut v = 0.0; for (k, (i, bt)) in sel.iter().enumerate().skip(bi.saturating_sub(1)).take(3) { let _ = i; let dt = t - bt; if dt < 0.0 { continue; } let sgn = if curve == "swing" && k % 2 == 1 { -1.0 } else { 1.0 }; let x = (dt / decay).min(6.0); v += sgn * amt * match curve.as_str() { "kick" => { let atk = (dt / (decay * 0.15)).min(1.0); atk * (-x * 2.5).exp() * 1.4 }, "flash" => (1.0 - dt / decay).max(0.0), _ => (-x * 3.0).exp() }; } v };
            let end = dur; let mut t = 0.0; let step = 1.0 / fps; let mut bi = 0usize; while t <= end + 1e-9 { while bi + 1 < sel.len() && sel[bi + 1].1 <= t { bi += 1; } pr.set_key(t, base + val(t, bi), Ease::Linear); nk += 1; t += step; }
            c.props.insert(prop, pr); Ok(json!({"ok":true,"beats":sel.len(),"keys":nk})) } },
    Command { name: "set_mesh_pin", description: "Pin a layer onto a deforming mesh of quads, for curved or waving surfaces such as cloth and flags. Pin a layer onto a deforming mesh (curved/waving surfaces, cloth, flags): the layer canvas is cut into (nx-1)*(ny-1) cells, each perspective-pinned to its own quad. `keys` = [{time (clip-local s), verts: [x0,y0,x1,y1,...] row-major nx*ny, px}]. Interpolates linearly between keys; supports motion_blur. track_mesh writes the same data.", mutates: true,
        schema: obj(vec![("clip_id", s("layer")), ("nx", n("vertex columns >= 2")), ("ny", n("vertex rows >= 2")), ("keys", json!({"type":"array","items":{"type":"object"}}))], &["clip_id","nx","ny","keys"]),
        run: |ss, a| { let (nx, ny) = (num_arg(a, "nx")? as usize, num_arg(a, "ny")? as usize); if nx < 2 || ny < 2 { return Err("nx, ny >= 2".into()); } let ks = a.get("keys").and_then(|v| v.as_array()).ok_or("keys needed")?; let c = clip(&mut ss.project, a)?;
            // every key is checked before the clip changes, so a bad key is refused with the clip untouched
            let parsed: Vec<(f64, Vec<f64>)> = ks.iter().map(|kk| { let t = kk.get("time").and_then(|v| v.as_f64()).ok_or("key needs time")?; let v: Vec<f64> = kk.get("verts").and_then(|v| v.as_array()).ok_or("key needs verts")?.iter().filter_map(|v| v.as_f64()).collect(); if v.len() != nx * ny * 2 { return Err(format!("verts needs {} numbers", nx * ny * 2)); } Ok((t, v)) }).collect::<Result<_, String>>()?;
            c.props.insert("pin_on".into(), Prop::new(1.0)); c.props.insert("mesh_nx".into(), Prop::new(nx as f64)); c.props.insert("mesh_ny".into(), Prop::new(ny as f64));
            for (t, v) in parsed {
                for k in 0..nx * ny { for (ax, nm) in [(0usize, "mx"), (1, "my")] { let pr = c.props.entry(format!("{nm}{k}")).or_insert_with(|| Prop::new(v[k * 2 + ax])); pr.set_key(t, v[k * 2 + ax], Ease::Linear); } } } Ok(json!({"ok":true})) } },
    Command { name: "track_mesh", description: "Mesh tracker for curved or deforming surfaces: a global homography refined at every grid vertex. Mesh tracker for curved or deforming surfaces: tracks the quad's global homography, then refines every vertex of an nx*ny grid against the frame-0 patch (warped by the local Jacobian, photometric-normalised, neighbour-regularised) and pins target_clip_id onto the mesh. Optional verts0 = normalised x,y of every grid vertex in the first frame (when the surface is already curved there). Heavier than track_planar (about 20-40x per vertex). Tier: handles smooth deformation up to about 12 px of vertex offset from the global plane per frame; no topology change, no occlusion handling per vertex yet.", mutates: true,
        schema: obj(vec![("clip_id", s("media clip")), ("corners", json!({"type":"array","items":{"type":"number"}})), ("nx", n("default 7")), ("ny", n("default 5")), ("target_clip_id", s("layer to pin")), ("start", n("")), ("end", n("")), ("return_verts", json!({"type":"boolean"})), ("verts0", json!({"type":"array","items":{"type":"number"}}))], &["clip_id","corners"]),
        run: |ss, a| { let cid = str_arg(a, "clip_id")?.to_string(); let cr: Vec<f64> = a.get("corners").and_then(|v| v.as_array()).ok_or("corners needed")?.iter().filter_map(|v| v.as_f64()).collect(); if cr.len() != 8 { return Err("corners needs 8 numbers".into()); }
            let (w, h) = (ss.project.width as f64, ss.project.height as f64); let mut q = [[0f64; 2]; 4]; for i in 0..4 { q[i] = [cr[i * 2] * w, cr[i * 2 + 1] * h]; } let (nx, ny) = (opt_num(a, "nx", 7.0) as usize, opt_num(a, "ny", 5.0) as usize);
            let mc = ss.project.find_clip(&cid).ok_or("no such clip")?.clone(); let nf = ((mc.duration * ss.project.fps).ceil() as usize).max(1); let f0 = opt_num(a, "start", 0.0) as usize; let f1 = (opt_num(a, "end", (nf - 1) as f64) as usize).min(nf - 1);
            let t0 = std::time::Instant::now(); let (vs, conf) = crate::planar::track_mesh(&ss.project, &cid, q, nx, ny, f0, f1, a.get("verts0").and_then(|v| v.as_array()).map(|v| { let f: Vec<f64> = v.iter().filter_map(|x| x.as_f64()).collect(); f.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0] * ss.project.width as f64, c[1] * ss.project.height as f64)).collect() }))?; let secs = t0.elapsed().as_secs_f64();
            if let Some(tid) = a.get("target_clip_id").and_then(|v| v.as_str()) { let fps = ss.project.fps; let ts = ss.project.find_clip(tid).ok_or("no target clip")?.start; let tc = ss.project.find_clip_mut(tid).ok_or("no target clip")?;
                tc.props.insert("pin_on".into(), Prop::new(1.0)); tc.props.insert("mesh_nx".into(), Prop::new(nx as f64)); tc.props.insert("mesh_ny".into(), Prop::new(ny as f64));
                for k in 0..nx * ny { for (ax, nm) in [(0usize, "mx"), (1, "my")] { let mut pr = Prop::new(vs[0][k][ax]); for (i, fv) in vs.iter().enumerate() { pr.set_key(mc.start + (f0 + i) as f64 / fps - ts, fv[k][ax], Ease::Linear); } tc.props.insert(format!("{nm}{k}"), pr); } } }
            let mean = conf.iter().sum::<f64>() / conf.len() as f64; let mut res = json!({"frames": vs.len(), "seconds": secs, "mean_conf": mean, "min_conf": conf.iter().cloned().fold(1.0, f64::min)});
            if a.get("return_verts").and_then(|v| v.as_bool()).unwrap_or(false) { res["verts"] = json!(vs.iter().map(|f| f.iter().flat_map(|p| vec![p[0], p[1]]).collect::<Vec<f64>>()).collect::<Vec<_>>()); } Ok(res) } },
    Command { name: "simulate_physics", description: "2D rigid-body physics for layers: each clip becomes a circle body that is simulated and baked to keyframes. 2D rigid-body physics for layers: each listed clip becomes a circle body (radius_px, mass) with start position x,y (pixel offsets from canvas centre, same space as the x/y clip properties), velocity vx,vy (px/s) and spin. Gravity, drag, restitution, friction, canvas walls and body-body collisions are simulated deterministically (480 Hz sub-steps) and baked to dense x, y, rotation keyframes, so the result is scrub-safe, editable in the graph editor and motion-blurs normally. Tier: circle proxies only (no polygon/concave shapes, no joints).", mutates: true,
        schema: obj(vec![("bodies", json!({"type":"array","items":{"type":"object"},"description":"[{clip_id,x,y,vx,vy,radius_px,mass,spin_deg_s}]"})), ("gravity", n("px/s^2 downward, default 1800")), ("gravity_x", n("px/s^2, default 0")), ("drag", n("0..5, default 0.05")), ("restitution", n("0..1, default 0.7")), ("friction", n("0..1, default 0.3")), ("walls", json!({"type":"string","description":"letters from LRTB, default LRB"})), ("collide", json!({"type":"boolean"})), ("duration", n("seconds, default the first clip duration")), ("start", n("clip-local start time, default 0"))], &["bodies"]),
        run: |ss, a| { let (w, h, fps) = (ss.project.width as f64, ss.project.height as f64, ss.project.fps); let arr = a.get("bodies").and_then(|v| v.as_array()).ok_or("bodies needed")?; let mut ids = vec![]; let mut bs = vec![]; let mut dur = opt_num(a, "duration", 0.0);
            for b in arr { let cid = b.get("clip_id").and_then(|v| v.as_str()).ok_or("body needs clip_id")?.to_string(); let g = |k: &str, d: f64| b.get(k).and_then(|v| v.as_f64()).unwrap_or(d); let c = ss.project.find_clip(&cid).ok_or("no such clip")?; if dur <= 0.0 { dur = c.duration; }
                let r = g("radius_px", 60.0); bs.push(crate::physics::Body2 { x: g("x", 0.0), y: g("y", 0.0), vx: g("vx", 0.0), vy: g("vy", 0.0), r, m: g("mass", (r * r / 3600.0).max(0.05)), rot: g("rot", 0.0), w: g("spin_deg_s", 0.0) }); ids.push(cid); }
            let wl = a.get("walls").and_then(|v| v.as_str()).unwrap_or("LRB").to_uppercase(); let pr = crate::physics::Params2 { gx: opt_num(a, "gravity_x", 0.0), gy: opt_num(a, "gravity", 1800.0), drag: opt_num(a, "drag", 0.05), restitution: opt_num(a, "restitution", 0.7).clamp(0.0, 1.0), friction: opt_num(a, "friction", 0.3), half_w: w / 2.0, half_h: h / 2.0, walls: [wl.contains('L'), wl.contains('R'), wl.contains('T'), wl.contains('B')], collide: a.get("collide").and_then(|v| v.as_bool()).unwrap_or(true) };
            let n = ((dur * fps).ceil() as usize).max(2); let t0 = opt_num(a, "start", 0.0); let tr = crate::physics::sim2d(bs, &pr, fps, n);
            for (bi, cid) in ids.iter().enumerate() { let c = ss.project.find_clip_mut(cid).ok_or("no such clip")?; for (k, nm) in [(0usize, "x"), (1, "y"), (2, "rotation")] { let mut p = Prop::new(tr[0][bi][k]); for (fi, fr) in tr.iter().enumerate() { p.set_key(t0 + fi as f64 / fps, fr[bi][k], Ease::Linear); } c.props.insert(nm.into(), p); } }
            Ok(json!({"bodies": ids.len(), "frames": n})) } },
    Command { name: "apply_motion_preset", description: "Apply a motion preset (After Effects / CapCut style) baked to per-frame keyframes on x, y, scale, rotation and opacity. Motion preset (After Effects / CapCut animation vocabulary) baked to dense per-frame keyframes on x, y, scale, rotation, opacity, relative to the clip's current values at `at`. Give `preset` by name (pop_in, bounce_in, elastic_in, slide_in_left/right/up/down, whip_in, spin_in, zoom_punch, shake, glitch_jitter, wobble, float, pulse (bpm), heartbeat, fade_in, fade_out, fade_out_down, drop_out, drift_zoom) OR a plain-language `prompt` (e.g. 'bounce in from above, then shake') and the matching preset(s) are applied; several presets compose when their time ranges differ. Parameters: at (clip-local s), duration (s, default 0.6), intensity (default 1), distance_px (default 420), bpm. Result lists exactly what was applied, so an AI can verify the interpretation. Presets stack: later calls keep earlier keys outside their own range.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("preset", s("preset name")), ("prompt", s("plain-language description, used when preset is not given")), ("at", n("clip-local start, default 0")), ("duration", n("seconds, default 0.6 (loops for float/pulse/heartbeat/shake until the end of the clip when until_end=true)")), ("intensity", n("default 1")), ("distance_px", n("default 420")), ("bpm", n("for pulse, default 120")), ("until_end", json!({"type":"boolean"}))], &["clip_id"]),
        run: |ss, a| { let fps = ss.project.fps; let cid = str_arg(a, "clip_id")?.to_string(); let names: Vec<String> = if let Some(p) = a.get("preset").and_then(|v| v.as_str()) { let norm = p.trim().to_lowercase().replace([' ', '-'], "_"); if crate::presets::NAMES.contains(&norm.as_str()) { vec![norm] } else { let m: Vec<String> = crate::presets::from_prompt(p).into_iter().take(1).map(|x| x.0.to_string()).collect(); if m.is_empty() { return Err(format!("unknown preset {p}; available: {}", crate::presets::NAMES.join(", "))); } m } } else if let Some(pr) = a.get("prompt").and_then(|v| v.as_str()) { crate::presets::from_prompt(pr).into_iter().take(2).map(|x| x.0.to_string()).collect() } else { return Err("give preset or prompt".into()) };
            if names.is_empty() { return Err(format!("no preset matched that prompt; available: {}", crate::presets::NAMES.join(", "))); }
            let (at, k, dist, bpm) = (opt_num(a, "at", 0.0), opt_num(a, "intensity", 1.0), opt_num(a, "distance_px", 420.0), opt_num(a, "bpm", 120.0)); let until_end = a.get("until_end").and_then(|v| v.as_bool()).unwrap_or(false);
            let c = ss.project.find_clip_mut(&cid).ok_or("no such clip")?; let base = |c: &crate::model::Clip, key: &str, d: f64| c.props.get(key).map(|p| p.at(at)).unwrap_or(d); let (bx, by, bs, br, bo) = (base(c, "x", 0.0), base(c, "y", 0.0), base(c, "scale", 1.0), base(c, "rotation", 0.0), base(c, "opacity", 1.0));
            let mut applied = vec![]; let mut t_cursor = at;
            for (ni, nm) in names.iter().enumerate() { let looping = matches!(nm.as_str(), "float" | "pulse" | "heartbeat" | "shake" | "glitch_jitter") && until_end; let dflt = if matches!(nm.as_str(), "float" | "pulse" | "heartbeat") { 2.0 } else { 0.6 }; let d = if looping { (c.duration - t_cursor).max(0.1) } else { opt_num(a, "duration", dflt).max(0.05) }; let t0 = if ni == 0 { at } else { t_cursor };
                if crate::presets::sample(nm, 0.0, d, k, dist, bpm).is_none() { return Err(format!("unknown preset {nm}; available: {}", crate::presets::NAMES.join(", "))); }
                let n = ((d * fps).ceil() as usize).max(1); let mut ps: Vec<crate::keyframe::Prop> = vec![]; for (key, dv) in [("x", bx), ("y", by), ("scale", bs), ("rotation", br), ("opacity", bo)] { ps.push(c.props.remove(key).unwrap_or_else(|| crate::keyframe::Prop::new(dv))); }
                for i in 0..=n { let s = (i as f64 / fps).min(d); let (dx, dy, sc, rt, op) = crate::presets::sample(nm, s, d, k, dist, bpm).unwrap(); let tt = t0 + s; for (pi, v) in [bx + dx, by + dy, bs * sc, br + rt, bo * op].iter().enumerate() { ps[pi].set_key(tt, *v, Ease::Linear); } }
                for (pi, key) in ["x", "y", "scale", "rotation", "opacity"].iter().enumerate() { c.props.insert((*key).into(), ps.remove(0)); let _ = pi; }
                applied.push(json!({"preset": nm, "at": t0, "duration": d, "frames": n + 1})); t_cursor = t0 + d; }
            Ok(json!({"applied": applied})) } },
    Command { name: "motion_extract", description: "Measure a clip's global camera motion per frame as curves, optionally writing it onto another clip. Motion match: measure a clip's global motion (camera shake, push, pan, roll) per frame relative to its first frame by tracking the whole frame, return it as curves, and optionally write it onto target_clip_id as x / y / scale / rotation keyframes (so graphics, text or effects move with the footage like a match-move; invert=true applies the opposite, i.e. a stabiliser; gain scales it). Global similarity only (no parallax, no rolling shutter).", mutates: true,
        schema: obj(vec![("clip_id", s("media clip to measure")), ("start", n("first frame")), ("end", n("last frame")), ("target_clip_id", s("optional: clip that receives the motion")), ("invert", json!({"type":"boolean"})), ("gain", n("default 1")), ("return_curves", json!({"type":"boolean"}))], &["clip_id"]),
        run: |ss, a| { let cid = str_arg(a, "clip_id")?.to_string(); let mc = ss.project.find_clip(&cid).ok_or("no such clip")?.clone(); let nf = ((mc.duration * ss.project.fps).ceil() as usize).max(1); let f0 = opt_num(a, "start", 0.0) as usize; let f1 = (opt_num(a, "end", (nf - 1) as f64) as usize).min(nf - 1);
            let t0 = std::time::Instant::now(); let gm = crate::planar::global_motion(&ss.project, &cid, f0, f1)?; let secs = t0.elapsed().as_secs_f64(); let fps = ss.project.fps; let gain = opt_num(a, "gain", 1.0); let sgn = if a.get("invert").and_then(|v| v.as_bool()).unwrap_or(false) { -1.0 } else { 1.0 };
            let rng = |k: usize| { let v: Vec<f64> = gm.iter().map(|g| g[k]).collect(); (v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max)) };
            if let Some(tid) = a.get("target_clip_id").and_then(|v| v.as_str()) { let ts = ss.project.find_clip(tid).ok_or("no target clip")?.start; let tc = ss.project.find_clip_mut(tid).ok_or("no target clip")?; let base = |tc: &crate::model::Clip, k: &str, d: f64| tc.props.get(k).map(|p| p.at(0.0)).unwrap_or(d); let (bx, by, bs, br) = (base(tc, "x", 0.0), base(tc, "y", 0.0), base(tc, "scale", 1.0), base(tc, "rotation", 0.0));
                let mut px = Prop::new(bx); let mut py = Prop::new(by); let mut ps = Prop::new(bs); let mut pr = Prop::new(br);
                for (i, g) in gm.iter().enumerate() { let t = mc.start + (f0 + i) as f64 / fps - ts; px.set_key(t, bx + sgn * gain * g[0], Ease::Linear); py.set_key(t, by + sgn * gain * g[1], Ease::Linear); let sc = if sgn > 0.0 { 1.0 + (g[2] - 1.0) * gain } else { 1.0 / (1.0 + (g[2] - 1.0) * gain) }; ps.set_key(t, bs * sc, Ease::Linear); pr.set_key(t, br + sgn * gain * g[3], Ease::Linear); }
                tc.props.insert("x".into(), px); tc.props.insert("y".into(), py); tc.props.insert("scale".into(), ps); tc.props.insert("rotation".into(), pr); }
            let mut res = json!({"frames": gm.len(), "seconds": secs, "range": {"dx": rng(0), "dy": rng(1), "scale": rng(2), "rot_deg": rng(3)}}); if a.get("return_curves").and_then(|v| v.as_bool()).unwrap_or(false) { res["curves"] = json!(gm); } Ok(res) } },
    Command { name: "motion_diff", description: "Motion diff: compare the global motion of two clips (e.g. your edit vs a reference video) frame by frame. Returns per-channel RMS difference (px, scale, degrees), the correlation of each channel, and the frame of worst disagreement, so an AI can tell whether a recreation moves like the original. Both clips are tracked with the same whole-frame tracker.", mutates: false,
        schema: obj(vec![("clip_a", s("media clip")), ("clip_b", s("media clip")), ("start", n("first frame")), ("end", n("last frame"))], &["clip_a","clip_b"]),
        run: |ss, a| { let (ca, cb) = (str_arg(a, "clip_a")?.to_string(), str_arg(a, "clip_b")?.to_string()); let m = ss.project.find_clip(&ca).ok_or("no clip_a")?.clone(); let nf = ((m.duration * ss.project.fps).ceil() as usize).max(1); let f0 = opt_num(a, "start", 0.0) as usize; let f1 = (opt_num(a, "end", (nf - 1) as f64) as usize).min(nf - 1);
            let ga = crate::planar::global_motion(&ss.project, &ca, f0, f1)?; let gb = crate::planar::global_motion(&ss.project, &cb, f0, f1)?; let n = ga.len().min(gb.len()); if n < 2 { return Err("need at least 2 frames".into()); }
            let mut ch = serde_json::Map::new(); let mut worst = (0usize, 0f64); for (k, nm) in ["dx_px", "dy_px", "scale", "rot_deg"].iter().enumerate() { let (a1, b1): (Vec<f64>, Vec<f64>) = (ga[..n].iter().map(|g| g[k]).collect(), gb[..n].iter().map(|g| g[k]).collect()); let rms = (a1.iter().zip(&b1).map(|(x, y)| (x - y).powi(2)).sum::<f64>() / n as f64).sqrt(); let (ma, mb) = (a1.iter().sum::<f64>() / n as f64, b1.iter().sum::<f64>() / n as f64); let (mut sab, mut saa, mut sbb) = (0.0, 1e-12, 1e-12); for i in 0..n { sab += (a1[i] - ma) * (b1[i] - mb); saa += (a1[i] - ma).powi(2); sbb += (b1[i] - mb).powi(2); }
                ch.insert(nm.to_string(), json!({"rms_diff": rms, "correlation": sab / (saa * sbb).sqrt()})); if k < 2 { for i in 0..n { let d = (a1[i] - b1[i]).abs(); if d > worst.1 { worst = (i, d); } } } }
            Ok(json!({"frames": n, "channels": ch, "worst_translation_frame": f0 + worst.0, "worst_translation_px": worst.1})) } },
    Command { name: "shards_project", description: "Per-shard screen-space data for a shard burst at a timeline time: centre, bounds, depth and alpha. Per-shard screen-space data for a plane_fracture / shard burst at a timeline time: id, centre, bounding box, depth and alpha of every visible shard (canvas pixels). Use it to attach text, light, sound or a tracked overlay to a specific fragment, or to occlude shards by a foreground matte.", mutates: false,
        schema: obj(vec![("time", n("timeline seconds")), ("min_alpha", n("drop shards below this alpha, default 0.02"))], &["time"]),
        run: |ss, a| { let tt = num_arg(a, "time")?; let ma = opt_num(a, "min_alpha", 0.02); let _ = crate::shards::take_probe(); render::prepare_at(&ss.project, &mut ss.cache, Some(tt))?; let _ = render::render_frame(&ss.project, tt, &ss.cache)?; let v: Vec<_> = crate::shards::take_probe().into_iter().filter(|q| q.alpha >= ma).collect(); Ok(json!({"count": v.len(), "shards": v})) } },
    Command { name: "render_frame", description: "Render the frame at a timeline time to a PNG file so you can look at it. Returns the path and proxy_assets used. Honors enabled preview proxies; missing/stale proxies and managed color use originals. Optional isolate (draw only these clips) / hide (draw all except these) take clip ids or @names; render-time only, the project is not changed, and a child keeps its parent-resolved pose while hiding a parent hides only that clip's own pixels.", mutates: false,
        schema: obj(vec![("isolate", crate::ext::view::list_schema(crate::ext::view::ISOLATE_DOC)), ("hide", crate::ext::view::list_schema(crate::ext::view::HIDE_DOC)), ("time", n("seconds")), ("path", s("output .png path")), ("inline", json!({"type":"string","enum":["auto","always","never"],"description":"MCP clients: attach a downscaled copy of the frame as an image block (default auto = yes unless env KIMO_INLINE_IMAGES=0). The file on disk is always full resolution."})), ("max_inline_px", n("inline long edge cap, 128..2048, default 1024")), ("inline_format", json!({"type":"string","enum":["png","jpeg"],"description":"default png; falls back to jpeg q80 when the png is over the 1 MB cap"})), ("inline_quality", n("jpeg quality 1..100, default 80"))], &["time","path"]),
        run: |ss, a| crate::ext::view::with_view(ss, a, |ss| { let p = str_arg(a, "path")?; render::check_still_path(p, true)?; render::render_png_with(&ss.project, num_arg(a, "time")?, p, &mut ss.cache, Some(&mut ss.fcache))?; ss.media.push(Media { path: p.to_string(), w: ss.project.width as u32, h: ss.project.height as u32, mime: "image/png" }); Ok(json!({"path":p,"proxy_assets":ss.cache.proxy_assets()})) }) },
    Command { name: "export", description: "Render the timeline (or a start/end range) to an H.264 mp4. Returns path, duration (s) and bytes (file size, e.g. for a 16 MB cap).", mutates: false,
        schema: obj(vec![("path", s("output .mp4 path")), ("crf", n("quality 0..51, default 18")), ("start", n("optional range start, seconds (default 0)")), ("end", n("optional range end, seconds (default timeline end). Frames and audio equal the same span of a full export.")), ("audio", json!({"type":"boolean","description":"default true. false writes picture only, so range exports can be joined and one continuous export_audio mix muxed once (separately encoded AAC ranges leave a gap at every seam)"})), ("low_memory", json!({"type":"boolean","description":"opt-in: lower x264 lookahead/refs/b-frames to cut encoder memory (encoder RSS 479 to 257 MB measured at 1080p, size -0.3%, PSNR -0.02 dB on a synthetic clip) for small machines; output bytes differ from the default"}))], &["path"]),
        run: |ss, a| { let p = str_arg(a, "path")?; let crf = opt_num(a, "crf", 18.0) as u32; let wa = a.get("audio").and_then(|v| v.as_bool()).unwrap_or(true); let lm = a.get("low_memory").and_then(|v| v.as_bool()).unwrap_or(false); let d = if a.get("start").is_some() || a.get("end").is_some() { let st = opt_num(a, "start", 0.0); let en = opt_num(a, "end", ss.project.duration()); render::export_range_opts(&ss.project, p, crf, st, en, lm, wa)? } else if wa { render::export_opts(&ss.project, p, crf, lm)? } else { let dur = ss.project.duration(); render::export_range_opts(&ss.project, p, crf, 0.0, dur, lm, false)? }; let bytes = std::fs::metadata(p).map(|m| m.len()).map_err(|e| format!("export wrote no readable file at {p}: {e}"))?; Ok(crate::buildinfo::stamp(json!({"path":p,"duration":d,"bytes":bytes}))) } },
    Command { name: "join_export", description: "Join picture-only range exports in order and mux one continuous audio stream, with no AAC gaps at the seams. Join picture-only range exports (export with audio=false) in order and mux ONE continuous audio stream for the same span, so there are no AAC gaps at the seams. start = timeline time of the first part.", mutates: false,
        schema: obj(vec![("parts", json!({"type":"array","items":{"type":"string"},"description":"picture-only mp4 paths, in order"})), ("path", s("output .mp4 path")), ("start", n("timeline start of the first part, seconds (default 0)"))], &["parts","path"]),
        run: |ss, a| { let parts: Vec<String> = a.get("parts").and_then(|v| v.as_array()).ok_or("missing array 'parts'")?.iter().map(|v| v.as_str().map(String::from).ok_or("parts must be strings".to_string())).collect::<Result<_, _>>()?; let p = str_arg(a, "path")?; let d = render::join_export(&ss.project, &parts, p, opt_num(a, "start", 0.0))?; Ok(crate::buildinfo::stamp(json!({"path":p,"duration":d}))) } },
    Command { name: "export_alpha", description: "Export the timeline or a range with transparency as ProRes 4444, VP9 WebM with alpha, or an RGBA PNG sequence. Export the timeline (or start/end range) WITH transparency: format prores4444 (.mov, 10-bit 4:4:4:4), webm (VP9 + alpha) or png (RGBA PNG sequence, path pattern like /dir/f_%05d.png). Straight alpha, no audio. Requires the managed color pipeline to be off.", mutates: false,
        schema: obj(vec![("path", s("output path (.mov/.webm, or a PNG pattern)")), ("format", json!({"type":"string","enum":["prores4444","webm","png"]})), ("start", n("range start seconds, default 0")), ("end", n("range end seconds, default timeline end"))], &["path","format"]),
        run: |ss, a| { let d = render::export_alpha(&ss.project, str_arg(a, "path")?, str_arg(a, "format")?, opt_num(a, "start", 0.0), opt_num(a, "end", ss.project.duration()))?; Ok(crate::buildinfo::stamp(json!({"path":str_arg(a, "path")?,"duration":d}))) } },
    Command { name: "save_project", description: "Save the project to a JSON file.", mutates: false,
        schema: obj(vec![("path", s("file path")), ("with_history", json!({"type":"boolean","description":"also write <path>.history with the undo steps and named checkpoints, so they survive a restart (load_project with_history restores them). Can be large: every step is a whole project."})), ("history_steps", n("with_history: newest undo steps to persist, default 20 (checkpoints are always all kept)"))], &["path"]),
        run: |ss, a| { let p = str_arg(a, "path")?;
            // project: streamed through a hashing writer into <p>.tmp then renamed, so a crash or full disk never leaves a truncated project and the history header can bind to the exact bytes
            let kimo_block = crate::buildinfo::saved_block(&mut ss.created_with); ss.project.base_dir = crate::deps::dir_of(p); crate::deps::set_base_dir(ss.project.base_dir.clone()); let tmp = format!("{p}.tmp"); let sha = { let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?; let mut w = FnvWriter { inner: std::io::BufWriter::new(f), h: 0xcbf29ce484222325 };
                ({ #[derive(serde::Serialize)] struct Saved<'a> { #[serde(skip_serializing_if = "Option::is_none")] kimo: Option<Value>, #[serde(flatten)] project: &'a Project } serde_json::to_writer_pretty(&mut w, &Saved { kimo: kimo_block, project: &ss.project }) }).map_err(|e| { let _ = std::fs::remove_file(&tmp); e.to_string() })?; std::io::Write::flush(&mut w).map_err(|e| { let _ = std::fs::remove_file(&tmp); e.to_string() })?; w.inner.get_ref().sync_all().map_err(|e| e.to_string())?; w.h };
            let had_old = std::fs::remove_file(format!("{p}.history")).is_ok(); // never leave an old history beside the new project, whatever happens next
            std::fs::rename(&tmp, p).map_err(|e| { let _ = std::fs::remove_file(&tmp); e.to_string() })?;
            if a.get("with_history").and_then(|v| v.as_bool()).unwrap_or(false) { // streamed straight to the file from references (no intermediate JSON Value tree: that tree was several times the size of the history and ran the engine out of memory on large projects); only the newest history_steps undo steps are written (default 20)
                #[derive(serde::Serialize)] struct Hist<'a> { version: u32, project_sha: String, undo_len: usize, undo: &'a [Project], undo_labels: &'a [String], checkpoints: &'a [(String, Project, usize)] }
                let keep = (opt_num(a, "history_steps", 20.0).max(0.0) as usize).min(ss.undo.len()); let (us, ls) = (ss.undo.len() - keep, ss.undo_labels.len().saturating_sub(keep));
                let htmp = format!("{p}.history.tmp"); let f = std::fs::File::create(&htmp).map_err(|e| e.to_string())?; let mut bw = std::io::BufWriter::new(f);
                serde_json::to_writer(&mut bw, &Hist { version: 1, project_sha: format!("{sha:016x}"), undo_len: keep, undo: &ss.undo[us..], undo_labels: &ss.undo_labels[ls..], checkpoints: &ss.checkpoints }).map_err(|e| { let _ = std::fs::remove_file(&htmp); e.to_string() })?; std::io::Write::flush(&mut bw).map_err(|e| e.to_string())?; bw.get_ref().sync_all().map_err(|e| e.to_string())?;
                std::fs::rename(&htmp, format!("{p}.history")).map_err(|e| e.to_string())?;
                let hb = std::fs::metadata(format!("{p}.history")).map(|m| m.len()).unwrap_or(0); return Ok(json!({"path":p,"history":format!("{p}.history"),"undo_steps":keep,"undo_steps_available":ss.undo.len(),"checkpoints":ss.checkpoints.len(),"history_bytes":hb,"project_sha":format!("{sha:016x}")})); }
            // a save without history must not leave an older history beside the new project
            let stale = had_old;
            Ok(json!({"path":p,"stale_history_removed":stale})) } },
    Command { name: "load_project", description: "Load a project JSON file.", mutates: true,
        schema: obj(vec![("path", s("file path")), ("report", json!({"type":"boolean","description":"also return `dependencies`: counts and the list of missing / changed external files (same data as list_dependencies, without hashing)"})), ("with_history", json!({"type":"boolean","description":"also load <path>.history (written by save_project with_history) so undo steps and checkpoints come back; the load itself is one more undo step on top. If the history is missing or does not belong to this project file (sha check) the project still loads and history_ignored says why"})), ("restore", json!({"type":"boolean","description":"with_history, crash recovery: when the history is restored the load itself is NOT an extra undo step, so the undo stack is exactly what was saved (the first undo reverts the last edit before the save)"}))], &["path"]),
        run: |ss, a| { let pth = str_arg(a, "path")?.to_string(); let t = std::fs::read_to_string(&pth).map_err(|e| e.to_string())?;
            let mut raw: Value = serde_json::from_str(&t).map_err(|e| e.to_string())?;
            let kimo_header = raw.get("kimo").cloned();
            let mut file_schema_version: u32 = 1;
            if let Some(k) = &kimo_header { if let Some(sv) = k.get("schema_version").and_then(|v| v.as_u64()) {
                if sv > crate::buildinfo::SCHEMA_VERSION as u64 { return Err(format!("project schema_version {sv} is newer than this build supports ({}); it was saved by kimo {} (commit {}). Update kimo to open it.", crate::buildinfo::SCHEMA_VERSION, k.pointer("/last_saved_with/version").and_then(|v| v.as_str()).unwrap_or("?"), k.pointer("/last_saved_with/commit").and_then(|v| v.as_str()).unwrap_or("?"))); }
                file_schema_version = sv as u32; } }
            // Migrate the raw JSON forward to this build's SCHEMA_VERSION BEFORE deserializing into Project (a
            // migration may change shapes serde's #[serde(default)] alone can't -- see migrations.rs). A file
            // already at (or past, refused above) the current version runs zero migrations and is untouched.
            crate::migrations::migrate(&mut raw, file_schema_version);
            let proj: Project = serde_json::from_value(raw).map_err(|e| e.to_string())?;
            for (id, asset) in &proj.assets { if std::path::Path::new(&asset.path).is_file() || crate::deps::seq_frame(&asset.path, 0).is_some() { render::check_importable(&asset.path).map_err(|e| format!("asset {id}: {e}"))?; } }
            ss.created_with = Some(kimo_header.and_then(|k| k.get("created_with").cloned()).unwrap_or_else(|| json!({"version": "unknown", "note": "saved before build identity existed"})));
            let mut ignored: Option<String> = None; let hist = if a.get("with_history").and_then(|v| v.as_bool()).unwrap_or(false) { match std::fs::read(format!("{pth}.history")) { Err(e) => { ignored = Some(format!("missing ({e})")); None } Ok(ht) => { let h: Value = serde_json::from_slice(&ht).map_err(|e| e.to_string())?;
                let want = h.get("project_sha").and_then(|v| v.as_str()).unwrap_or(""); let have = format!("{:016x}", fnv64(t.as_bytes()));
                if want != have { ignored = Some(if want.is_empty() { "history file has no project_sha (older format)".to_string() } else { format!("history belongs to a different project save (history sha {want}, project sha {have})") }); None } else {
                let u: Vec<Project> = serde_json::from_value(h["undo"].clone()).map_err(|e| e.to_string())?; let ul: Vec<String> = serde_json::from_value(h["undo_labels"].clone()).map_err(|e| e.to_string())?; let cp: Vec<(String, Project, usize)> = serde_json::from_value(h["checkpoints"].clone()).map_err(|e| e.to_string())?;
                if u.len() != ul.len() { return Err("history file is inconsistent (undo and labels differ in length)".into()); } Some((u, ul, cp)) } } } } else { None };
            let restored = hist.is_some(); ss.project = proj; ss.project.base_dir = crate::deps::dir_of(&pth); crate::deps::set_base_dir(ss.project.base_dir.clone()); crate::auto_proxy::on_project_loaded(&ss.project); if let Some((u, ul, cp)) = hist { ss.undo = u; ss.undo_labels = ul; ss.checkpoints = cp; ss.redo.clear(); ss.redo_labels.clear(); }
            { let others: Vec<&Project> = ss.undo.iter().chain(ss.redo.iter()).chain(ss.checkpoints.iter().map(|c| &c.1)).collect(); ss.project.continue_ids_after_load(&others); } let mut out = json!({"tracks":ss.project.tracks.len(),"next_id":ss.project.next_id,"undo_steps":ss.undo.len(),"checkpoints":ss.checkpoints.len(),"history_ignored":ignored});
            if restored && a.get("restore").and_then(|v| v.as_bool()).unwrap_or(false) { out["no_history"] = json!(true); } // crash restore: the saved history is the whole history (no load step on top)
            if a.get("report").and_then(|v| v.as_bool()).unwrap_or(false) { let items = crate::deps::list_dependencies(&ss.project, false); let bad: Vec<&crate::deps::DepItem> = items.iter().filter(|i| i.status != "ok").collect(); out["dependencies"] = json!({"total": items.len(), "missing": items.iter().filter(|i| i.status == "missing").count(), "changed": items.iter().filter(|i| i.status == "changed").count(), "not_ok": bad}); }
            Ok(out) } },
    Command { name: "kimo_info", description: "Build identity of this server: semver version, git commit, project schema version, enabled features and backend versions (ffmpeg, ffprobe). The same identity is recorded in saved projects (the \"kimo\" block) and in export reports.", mutates: false,
        schema: obj(vec![], &[]),
        run: |ss, _a| {let mut v=crate::buildinfo::info();v["limits"]=serde_json::to_value(ss.resource_limits).unwrap_or(Value::Null);Ok(v)} },
    Command { name: "cache_stats", description: "Frame cache statistics: entries, bytes, cap, hits, misses, evictions, hit_rate. The cache holds recently rendered frames for scrubbing and repeated render_frame / get_scopes; it is never used by export.", mutates: false,
        schema: obj(vec![], &[]),
        run: |ss, _a| { let f = &ss.fcache; let tot = f.hits + f.misses; Ok(json!({"entries": f.len(), "bytes": f.bytes(), "cap_bytes": f.cap, "hits": f.hits, "misses": f.misses, "evictions": f.evictions, "hit_rate": if tot == 0 { 0.0 } else { f.hits as f64 / tot as f64 }})) } },
    Command { name: "cache_clear", description: "Empty the frame cache (frees its memory). Renders afterwards are bit-identical, only slower once.", mutates: false,
        schema: obj(vec![], &[]),
        run: |ss, _a| { let (n, b) = (ss.fcache.len(), ss.fcache.bytes()); ss.fcache.clear(); Ok(json!({"cleared_entries": n, "freed_bytes": b})) } },
    Command { name: "cache_set_budget", description: "Set the frame cache size in megabytes (0 turns it off). Must not exceed the memory budget; shrinking evicts the oldest frames.", mutates: false,
        schema: obj(vec![("megabytes", n("cache size in MB, 0 = off"))], &["megabytes"]),
        run: |ss, a| { let mb = num_arg(a, "megabytes")?; if !(mb >= 0.0) || !mb.is_finite() { return Err(format!("megabytes must be >= 0, got {mb}")); }
            let want = (mb * 1048576.0) as usize; let b = crate::guard::BUDGET.load(std::sync::atomic::Ordering::Relaxed);
            if b != usize::MAX && want > b { return Err(format!("cache budget {mb} MB is above the memory budget of {} MB (raise KIMO_MEM_MB or choose a smaller cache)", b >> 20)); }
            ss.fcache.set_cap(want); Ok(json!({"cap_bytes": ss.fcache.cap, "entries": ss.fcache.len(), "bytes": ss.fcache.bytes()})) } },
    Command { name: "undo", description: "Undo the last mutating operation(s). steps (default 1) undoes several at once. history_list shows what each step is.", mutates: false,
        schema: obj(vec![("steps", n("how many steps to undo, default 1"))], &[]),
        run: |ss, a| { let steps = opt_num(a, "steps", 1.0).max(1.0) as usize; if ss.undo.is_empty() { return Err("nothing to undo".into()); } let mut done = 0;
            while done < steps { let Some(prev) = ss.undo.pop() else { break }; let lb = ss.undo_labels.pop().unwrap_or_default(); let cur = std::mem::replace(&mut ss.project, prev); ss.redo.push(cur); ss.redo_labels.push(lb); done += 1; }
            ss.project.recompute_next_id(); Ok(json!({"ok":true,"undone":done,"undo_left":ss.undo.len(),"redo_available":ss.redo.len()})) } },
    Command { name: "redo", description: "Redo the last undone operation(s). steps (default 1).", mutates: false,
        schema: obj(vec![("steps", n("how many steps to redo, default 1"))], &[]),
        run: |ss, a| { let steps = opt_num(a, "steps", 1.0).max(1.0) as usize; if ss.redo.is_empty() { return Err("nothing to redo".into()); } let mut done = 0;
            while done < steps { let Some(nx) = ss.redo.pop() else { break }; let lb = ss.redo_labels.pop().unwrap_or_default(); let cur = std::mem::replace(&mut ss.project, nx); ss.undo.push(cur); ss.undo_labels.push(lb); done += 1; }
            ss.project.recompute_next_id(); Ok(json!({"ok":true,"redone":done,"undo_left":ss.undo.len(),"redo_available":ss.redo.len()})) } },
    Command { name: "history_list", description: "List the edit history: undo steps (oldest first, the last one is what `undo` reverts next), how many redo steps exist, and the named checkpoints.", mutates: false,
        schema: obj(vec![], &[]),
        run: |ss, _| Ok(json!({"undo": ss.undo_labels.iter().enumerate().map(|(i, l)| json!({"step": i, "tool": l})).collect::<Vec<_>>(),
            "next_undo": ss.undo_labels.last(), "redo": ss.redo_labels.iter().rev().cloned().collect::<Vec<_>>(),
            "checkpoints": ss.checkpoints.iter().map(|(n, _, at)| json!({"name": n, "after_undo_steps": at})).collect::<Vec<_>>() })) },
    Command { name: "checkpoint", description: "Save a named snapshot of the whole project (for example before a colour grade) so it can be restored later with `restore`. Same name overwrites. Keeps the newest 24. Memory only, not saved with the project.", mutates: false,
        schema: obj(vec![("name", s("checkpoint name"))], &["name"]),
        run: |ss, a| { let name = str_arg(a, "name")?.trim().to_string(); if name.is_empty() || name.len() > 80 { return Err("name must be 1 to 80 characters".into()); }
            let snap = (name.clone(), ss.project.clone(), ss.undo.len()); if let Some(e) = ss.checkpoints.iter_mut().find(|c| c.0 == name) { *e = snap; } else { ss.checkpoints.push(snap); if ss.checkpoints.len() > MAX_CHECKPOINTS { ss.checkpoints.remove(0); } }
            Ok(json!({"ok":true,"name":name,"checkpoints":ss.checkpoints.iter().map(|c| c.0.clone()).collect::<Vec<_>>()})) } },
    Command { name: "restore", description: "Restore the whole project to a named checkpoint. This is itself one undo step, so `undo` brings back what you had before restoring. The checkpoint stays available.", mutates: true,
        schema: obj(vec![("name", s("checkpoint name from history_list"))], &["name"]),
        run: |ss, a| { let name = str_arg(a, "name")?.to_string(); let p = ss.checkpoints.iter().find(|c| c.0 == name).map(|c| c.1.clone()).ok_or_else(|| format!("no checkpoint '{name}'. Existing: {}", ss.checkpoints.iter().map(|c| c.0.as_str()).collect::<Vec<_>>().join(", ")))?;
            ss.project = p; ss.project.recompute_next_id(); Ok(json!({"ok":true,"restored":name})) } },
    Command { name: "checkpoint_delete", description: "Delete a named checkpoint.", mutates: false,
        schema: obj(vec![("name", s("checkpoint name"))], &["name"]),
        run: |ss, a| { let name = str_arg(a, "name")?.to_string(); let l = ss.checkpoints.len(); ss.checkpoints.retain(|c| c.0 != name); if ss.checkpoints.len() == l { Err(format!("no checkpoint '{name}'")) } else { Ok(json!({"ok":true})) } } },
    Command { name: "gpu2d_status", description: "Probe whether the v7 opt-in GPU/2D compute path can open a wgpu adapter, without rendering anything. Feature `gpu2d`, default off. Returns backend (the adapter's name/driver string, e.g. a lavapipe/llvmpipe software-Vulkan string) and fallback_reason (set, backend=\"cpu\", when gpu2d is not compiled in or no adapter opens). The CPU path (used by every other effect and by gpu_gaussian_blur_frame/gpu_color_matrix_frame when the GPU is unavailable) is unaffected either way.", mutates: false,
        schema: obj(vec![], &[]),
        run: |_ss, _a| { let s = crate::gpu2d::probe(); Ok(json!({"backend": s.backend, "fallback_reason": s.fallback_reason, "gpu_available": s.fallback_reason.is_none()})) } },
    Command { name: "gpu_gaussian_blur_frame", description: "v7 opt-in GPU path: render the frame at `time` and gaussian-blur it through a WGSL compute shader on whatever wgpu adapter is present. Feature `gpu2d`, default off; lavapipe software Vulkan on this box; same sigma meaning as the glow effect's kernel. Falls back to the identical CPU reference implementation when gpu2d is not compiled in or no adapter opens (backend=\"cpu\" in the result either way tells you which ran). Writes the result to `path` (.png). With compare_cpu (default true) also runs the CPU reference and reports max_abs_diff, a live CPU-vs-GPU tolerance check. This does not change the project or any existing render path -- it is a standalone preview/diagnostic tool, not wired into add_effect.", mutates: false,
        schema: obj(vec![("time", n("seconds")), ("sigma", n("gaussian sigma in px, default 4")), ("path", s("output .png path")), ("compare_cpu", json!({"type":"boolean","description":"default true: also run the CPU reference and report max_abs_diff"})), ("software_only", json!({"type":"boolean","description":"force wgpu's fallback (software) adapter, default false"}))], &["time","path"]),
        run: |ss, a| { let t = num_arg(a, "time")?; let sigma = opt_num(a, "sigma", 4.0) as f32; let path = str_arg(a, "path")?; render::check_still_path(path, true)?;
            let frame = render::render_frame(&ss.project, t, &ss.cache)?;
            let compare = a.get("compare_cpu").and_then(|v| v.as_bool()).unwrap_or(true);
            let software_only = a.get("software_only").and_then(|v| v.as_bool()).unwrap_or(false);
            let t0 = std::time::Instant::now();
            let (out, status) = crate::gpu2d::gpu_gaussian_blur(&frame, sigma, software_only).unwrap_or_else(|e| (crate::gpu2d::cpu_gaussian_blur(&frame, sigma), crate::gpu2d::Gpu2dStatus { backend: "cpu".into(), fallback_reason: Some(e) }));
            let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let mut res = json!({"path": path, "backend": status.backend, "fallback_reason": status.fallback_reason, "elapsed_ms": elapsed_ms});
            if compare { let cpu = crate::gpu2d::cpu_gaussian_blur(&frame, sigma); let max_abs_diff = out.d.iter().zip(cpu.d.iter()).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max); res["max_abs_diff_vs_cpu"] = json!(max_abs_diff); }
            render::write_image(&out, path)?; Ok(res) } },
    Command { name: "gpu_color_matrix_frame", description: "v7 opt-in GPU path: render the frame at `time` and apply a colour matrix through a WGSL compute shader on whatever wgpu adapter is present. Feature `gpu2d`, default off; lavapipe software Vulkan on this box. matrix is 20 numbers: 4 AE-style rows of [r,g,b,a,offset] producing out r,g,b,a. Falls back to the identical CPU reference implementation when gpu2d is not compiled in or no adapter opens. Writes the result to `path` (.png). With compare_cpu (default true) also runs the CPU reference and reports max_abs_diff. Standalone preview/diagnostic tool, not wired into add_effect or any existing render path.", mutates: false,
        schema: obj(vec![("time", n("seconds")), ("matrix", json!({"type":"array","items":{"type":"number"},"minItems":20,"maxItems":20,"description":"4 rows x [r,g,b,a,offset], row order r,g,b,a; identity = [1,0,0,0,0, 0,1,0,0,0, 0,0,1,0,0, 0,0,0,1,0]"})), ("path", s("output .png path")), ("compare_cpu", json!({"type":"boolean","description":"default true"})), ("software_only", json!({"type":"boolean"}))], &["time","matrix","path"]),
        run: |ss, a| { let t = num_arg(a, "time")?; let path = str_arg(a, "path")?; render::check_still_path(path, true)?;
            let marr: Vec<f64> = a.get("matrix").and_then(|v| v.as_array()).ok_or("matrix needed: 20 numbers")?.iter().filter_map(|v| v.as_f64()).collect();
            if marr.len() != 20 { return Err(format!("matrix needs exactly 20 numbers (4 rows of r,g,b,a,offset), got {}", marr.len())); }
            let mut m = [0f32; 20]; for i in 0..20 { m[i] = marr[i] as f32; }
            let frame = render::render_frame(&ss.project, t, &ss.cache)?;
            let compare = a.get("compare_cpu").and_then(|v| v.as_bool()).unwrap_or(true);
            let software_only = a.get("software_only").and_then(|v| v.as_bool()).unwrap_or(false);
            let t0 = std::time::Instant::now();
            let (out, status) = crate::gpu2d::gpu_color_matrix(&frame, &m, software_only).unwrap_or_else(|e| (crate::gpu2d::cpu_color_matrix(&frame, &m), crate::gpu2d::Gpu2dStatus { backend: "cpu".into(), fallback_reason: Some(e) }));
            let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let mut res = json!({"path": path, "backend": status.backend, "fallback_reason": status.fallback_reason, "elapsed_ms": elapsed_ms});
            if compare { let cpu = crate::gpu2d::cpu_color_matrix(&frame, &m); let max_abs_diff = out.d.iter().zip(cpu.d.iter()).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max); res["max_abs_diff_vs_cpu"] = json!(max_abs_diff); }
            render::write_image(&out, path)?; Ok(res) } },
    ]
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;

    #[test] fn smooth_curve_names_a_missing_property_or_clip() {
        let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
        refuse(&mut s, "smooth_curve", json!({"clip_id": id, "property": "nope"}), "no such property");
        refuse(&mut s, "smooth_curve", json!({"clip_id": "zz", "property": "x"}), "no such clip 'zz'");
        refuse(&mut s, "smooth_curve", json!({"clip_id": id}), "missing string 'property'");
    }

    #[test] fn set_text_path_refusals_leave_the_clip_unchanged() {
        let (mut s, t) = session(); let id = text(&mut s, &t);
        refuse(&mut s, "set_text_path", json!({"clip_id": id, "shape": "blob"}), "bad shape");
        refuse(&mut s, "set_text_path", json!({"clip_id": id}), "give shape or points");
        refuse(&mut s, "set_text_path", json!({"clip_id": id, "points": [[0.5, 0.5]]}), "need at least 2 points");
        refuse(&mut s, "set_text_path", json!({"clip_id": "zz", "remove": true}), "no such clip 'zz'");
        refuse(&mut s, "set_text_path", json!({"clip_id": "zz", "shape": "circle"}), "no such clip 'zz'");
        // regression: the new path used to be stored before the bad ease was refused
        assert!(err(&mut s, "set_text_path", json!({"clip_id": id, "shape": "circle", "animate": {"ease": "wobbly"}})).contains("wobbly"));
        assert!(cj(&s, &id)["text_fx"].is_null());
    }

    fn beat_session(curve: &str, extra: Value) -> (Session, String, Value) {
        let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
        let mut a = json!({"clip_id": id, "property": "rotation", "bpm": 120, "amount": 10, "decay": 0.2, "curve": curve}); for (k, v) in extra.as_object().unwrap() { a[k] = v.clone(); }
        let r = c(&mut s, "bind_beats", a); (s, id, r)
    }
    #[test] fn bind_beats_flash_drops_linearly_over_the_decay_on_every_beat() {
        // 120 bpm on a 1 s clip: beats at 0 and 0.5; flash = amount * (1 - dt/decay), one key per 0.1 s frame
        let (s, id, r) = beat_session("flash", json!({}));
        assert_eq!((r["beats"].as_u64(), r["keys"].as_u64()), (Some(2), Some(11)), "{r}");
        let want = [10.0, 5.0, 0.0, 0.0, 0.0, 10.0, 5.0, 0.0, 0.0, 0.0, 0.0];
        let k = keys(&s, &id, "rotation"); assert_eq!(k.len(), 11);
        for (i, (k, w)) in k.iter().zip(want).enumerate() { close(k.0, i as f64 * 0.1, 1e-9, "key time"); close(k.1, w, 1e-9, &format!("flash key {i}")); }
    }
    #[test] fn bind_beats_swing_alternates_sign_and_kick_attacks_from_zero() {
        // swing: beat 1 pulls the other way, plus beat 0's tail 10 e^(-3 * min(0.5/0.2, 6)) = 10 e^-7.5
        let (s, id, _) = beat_session("swing", json!({}));
        let k = keys(&s, &id, "rotation"); close(k[0].1, 10.0, 1e-9, "swing beat 0"); close(k[5].1, 10.0 * (-7.5f64).exp() - 10.0, 1e-9, "swing beat 1");
        // kick: zero at the beat, full attack after 15% of decay, then 1.4 e^(-2.5 dt/decay): at 0.1 s = 14 e^-1.25
        let (s, id, _) = beat_session("kick", json!({}));
        let k = keys(&s, &id, "rotation"); close(k[0].1, 0.0, 1e-12, "kick starts at 0"); close(k[1].1, 10.0 * 1.4 * (-1.25f64).exp(), 1e-9, "kick at 0.1 s");
    }
    #[test] fn bind_beats_skips_beats_before_the_clip_and_defaults_the_decay() {
        // offset -0.25: beats at 0.25 and 0.75; 0.3 s is 0.05 s after the first
        let (s, id, r) = beat_session("flash", json!({"offset": -0.25}));
        assert_eq!(r["beats"], 2); close(keys(&s, &id, "rotation")[3].1, 10.0 * (1.0 - 0.05 / 0.2), 1e-9, "0.05 s after the 0.25 s beat");
        // one explicit beat: the period falls back to 0.5 s, decay = 0.25 * 0.5 = 0.125 s
        let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
        c(&mut s, "bind_beats", json!({"clip_id": id, "property": "rotation", "beats": [0.3], "amount": 1, "curve": "flash"}));
        let k = keys(&s, &id, "rotation"); close(k[3].1, 1.0, 1e-9, "the beat"); close(k[4].1, 1.0 - 0.1 / 0.125, 1e-9, "0.1 s later");
    }
    #[test] fn bind_beats_names_missing_arguments() {
        let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
        refuse(&mut s, "bind_beats", json!({"clip_id": "zz", "property": "x", "bpm": 60}), "no such clip 'zz'");
        refuse(&mut s, "bind_beats", json!({"clip_id": id, "bpm": 60}), "missing string 'property'");
        refuse(&mut s, "bind_beats", json!({"clip_id": id, "property": "x"}), "missing number 'bpm'");
    }

    #[test] fn set_mesh_pin_refusals_leave_the_clip_unchanged() {
        let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
        refuse(&mut s, "set_mesh_pin", json!({"clip_id": id, "nx": 1, "ny": 2, "keys": []}), "nx, ny >= 2");
        refuse(&mut s, "set_mesh_pin", json!({"clip_id": id, "nx": 2, "ny": 2}), "keys needed");
        refuse(&mut s, "set_mesh_pin", json!({"clip_id": "zz", "nx": 2, "ny": 2, "keys": []}), "no such clip 'zz'");
        refuse(&mut s, "set_mesh_pin", json!({"clip_id": id, "ny": 2, "keys": []}), "missing number 'nx'");
        refuse(&mut s, "set_mesh_pin", json!({"clip_id": id, "nx": 2, "keys": []}), "missing number 'ny'");
        // regression: pin_on / mesh_nx / mesh_ny used to be written before a bad key was refused
        let v8 = [0, 0, 64, 0, 0, 32, 64, 32];
        refuse(&mut s, "set_mesh_pin", json!({"clip_id": id, "nx": 2, "ny": 2, "keys": [{"verts": v8}]}), "key needs time");
        refuse(&mut s, "set_mesh_pin", json!({"clip_id": id, "nx": 2, "ny": 2, "keys": [{"time": 0}]}), "key needs verts");
        refuse(&mut s, "set_mesh_pin", json!({"clip_id": id, "nx": 2, "ny": 2, "keys": [{"time": 0, "verts": v8}, {"time": 1, "verts": [1, 2]}]}), "verts needs 8 numbers");
        assert!(s.project.find_clip(&id).unwrap().props.get("pin_on").is_none());
    }

    #[test] fn simulate_physics_names_bad_bodies() {
        let (mut s, t) = session(); shape(&mut s, &t, 0.0, 1.0);
        refuse(&mut s, "simulate_physics", json!({}), "bodies needed");
        refuse(&mut s, "simulate_physics", json!({"bodies": [{}]}), "body needs clip_id");
        refuse(&mut s, "simulate_physics", json!({"bodies": [{"clip_id": "zz"}]}), "no such clip");
    }

    #[test] fn apply_motion_preset_refuses_unknown_names_and_prompts() {
        let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 2.0);
        refuse(&mut s, "apply_motion_preset", json!({"clip_id": id}), "give preset or prompt");
        refuse(&mut s, "apply_motion_preset", json!({"clip_id": id, "preset": "zzz"}), "unknown preset zzz; available: pop_in, bounce_in,*");
        refuse(&mut s, "apply_motion_preset", json!({"clip_id": id, "prompt": "do nothing special"}), "no preset matched that prompt; available: pop_in*");
        refuse(&mut s, "apply_motion_preset", json!({"clip_id": "zz", "preset": "fade_in"}), "no such clip 'zz'");
        refuse(&mut s, "apply_motion_preset", json!({"preset": "fade_in"}), "missing string 'clip_id'");
    }
    #[test] fn motion_preset_on_a_clip_without_transform_props_starts_from_defaults() {
        let mut s = load(json!({"name": "t", "markers": [], "assets": {}, "width": 64, "height": 32, "fps": 10, "tracks": [{"id": "track1", "clips": [{"id": "clip9", "source": {"type": "solid", "color": [1, 0, 0]}, "start": 0, "duration": 1, "in_point": 0, "speed": 1, "effects": [], "blend": "normal", "props": {}}]}]}));
        c(&mut s, "apply_motion_preset", json!({"clip_id": "clip9", "preset": "fade_out", "duration": 0.2}));
        let cl = s.project.find_clip("clip9").unwrap();
        for (k, base) in [("x", 0.0), ("y", 0.0), ("scale", 1.0), ("rotation", 0.0), ("opacity", 1.0)] { assert_eq!(cl.props[k].base, base, "{k} default"); }
        assert_eq!(keys(&s, "clip9", "opacity"), vec![(0.0, 1.0), (0.1, 0.5), (0.2, 0.0)]);
    }

    #[test] fn shards_project_without_a_burst_reports_no_shards() {
        let (mut s, t) = session(); shape(&mut s, &t, 0.0, 1.0);
        refuse(&mut s, "shards_project", json!({}), "missing number 'time'");
        assert_eq!(c(&mut s, "shards_project", json!({"time": 0.5, "min_alpha": 0.5})), json!({"count": 0, "shards": []}));
    }

    #[test] fn gpu_frame_tools_check_their_arguments_before_rendering() {
        let (mut s, t) = session(); shape(&mut s, &t, 0.0, 1.0);
        let still = "/tmp/x.txt: this writes a still image, so the path must end in .png (or .jpg). Nothing was written. To look at a video file (an export or a source clip) use review_export {path}";
        refuse(&mut s, "gpu_gaussian_blur_frame", json!({"path": "/tmp/x.png"}), "missing number 'time'");
        refuse(&mut s, "gpu_gaussian_blur_frame", json!({"time": 0.5}), "missing string 'path'");
        refuse(&mut s, "gpu_gaussian_blur_frame", json!({"time": 0.5, "path": "/tmp/x.txt"}), still);
        refuse(&mut s, "gpu_color_matrix_frame", json!({"time": 0.5, "path": "/tmp/x.png"}), "matrix needed: 20 numbers");
        refuse(&mut s, "gpu_color_matrix_frame", json!({"time": 0.5, "path": "/tmp/x.png", "matrix": [1, 0]}), "matrix needs exactly 20 numbers (4 rows of r,g,b,a,offset), got 2");
        refuse(&mut s, "gpu_color_matrix_frame", json!({"time": 0.5, "matrix": [1, 0]}), "missing string 'path'");
        refuse(&mut s, "gpu_color_matrix_frame", json!({"path": "/tmp/x.png", "matrix": [1, 0]}), "missing number 'time'");
        refuse(&mut s, "gpu_color_matrix_frame", json!({"time": 0.5, "path": "/tmp/x.txt", "matrix": vec![0; 20]}), still);
    }
    #[test] fn gpu_frame_tools_report_an_unwritable_output() {
        // the frame renders but cannot be written: the output folder does not exist
        let (mut s, t) = session(); shape(&mut s, &t, 0.0, 1.0);
        let bad = tmp("missing_dir").join("x.png"); let bs = bad.to_str().unwrap();
        assert!(dispatch(&mut s, "gpu_gaussian_blur_frame", &json!({"time": 0.5, "path": bs, "compare_cpu": false})).is_err());
        assert!(dispatch(&mut s, "gpu_color_matrix_frame", &json!({"time": 0.5, "path": bs, "matrix": vec![0.0; 20], "compare_cpu": false})).is_err());
        assert!(!bad.exists());
    }

    #[test] fn export_tools_name_missing_arguments() {
        let (mut s, t) = session(); shape(&mut s, &t, 0.0, 0.2);
        refuse(&mut s, "export", json!({}), "missing string 'path'");
        refuse(&mut s, "join_export", json!({"path": "/tmp/j.mp4"}), "missing array 'parts'");
        refuse(&mut s, "join_export", json!({"path": "/tmp/j.mp4", "parts": [1]}), "parts must be strings");
        refuse(&mut s, "join_export", json!({"parts": ["/tmp/a.mp4"]}), "missing string 'path'");
        refuse(&mut s, "export_alpha", json!({"path": "/tmp/x.mov"}), "missing string 'format'");
        refuse(&mut s, "export_alpha", json!({"format": "png"}), "missing string 'path'");
    }
    #[test] fn exports_into_a_missing_folder_fail_in_every_variant() {
        let (mut s, t) = session(); shape(&mut s, &t, 0.0, 0.2);
        let dir = tmp("no_such_dir"); let bad = dir.join("out.mp4"); let b = bad.to_str().unwrap();
        for a in [json!({"path": b}), json!({"path": b, "audio": false}), json!({"path": b, "start": 0.1}), json!({"path": b, "low_memory": true})] { assert!(dispatch(&mut s, "export", &a).is_err(), "{a}"); }
        assert!(dispatch(&mut s, "join_export", &json!({"path": b, "parts": [dir.join("a.mp4").to_str().unwrap()]})).is_err());
        assert!(dispatch(&mut s, "export_alpha", &json!({"path": b, "format": "gif"})).is_err());
        assert!(!dir.exists(), "nothing was created");
    }

    #[test] fn track_beats_motion_and_mesh_tools_name_missing_arguments() {
        let (mut s, t) = session(); let id = shape(&mut s, &t, 0.0, 1.0);
        refuse(&mut s, "track_beats", json!({}), "missing string 'asset_id'");
        refuse(&mut s, "track_beats", json!({"asset_id": "zz"}), "no such asset");
        refuse(&mut s, "motion_extract", json!({}), "missing string 'clip_id'");
        refuse(&mut s, "motion_extract", json!({"clip_id": "zz"}), "no such clip 'zz'");
        refuse(&mut s, "motion_diff", json!({"clip_b": id}), "missing string 'clip_a'");
        refuse(&mut s, "motion_diff", json!({"clip_a": id}), "missing string 'clip_b'");
        refuse(&mut s, "motion_diff", json!({"clip_a": "zz", "clip_b": id}), "no such clip 'zz'");
        refuse(&mut s, "track_mesh", json!({"corners": [0, 0, 1, 0, 1, 1, 0, 1]}), "missing string 'clip_id'");
        refuse(&mut s, "track_mesh", json!({"clip_id": id}), "corners needed");
        refuse(&mut s, "track_mesh", json!({"clip_id": id, "corners": [1]}), "corners needs 8 numbers");
        refuse(&mut s, "track_mesh", json!({"clip_id": "zz", "corners": [0, 0, 1, 0, 1, 1, 0, 1]}), "no such clip 'zz'");
    }
    #[test] fn media_analysis_on_a_missing_file_fails_and_names_it() {
        let (mut s, t) = with_asset(); c(&mut s, "add_clip", json!({"track_id": t, "source_type": "media", "asset_id": "a1", "start": 0, "duration": 1}));
        assert!(err(&mut s, "shards_project", json!({"time": 0.5})).contains("kimo_clip.mkv"));
        assert!(dispatch(&mut s, "track_beats", &json!({"asset_id": "a1"})).is_err());
    }

    #[test] fn undo_and_redo_move_whole_steps_and_refuse_when_empty() {
        let mut s = Session::new();
        refuse(&mut s, "undo", json!({}), "nothing to undo"); refuse(&mut s, "redo", json!({}), "nothing to redo");
        c(&mut s, "add_track", json!({})); c(&mut s, "add_track", json!({}));
        assert_eq!(c(&mut s, "undo", json!({"steps": 5})), json!({"ok": true, "undone": 2, "undo_left": 0, "redo_available": 2}));
        assert!(s.project.tracks.is_empty());
        assert_eq!(c(&mut s, "redo", json!({"steps": 9})), json!({"ok": true, "redone": 2, "undo_left": 2, "redo_available": 0}));
        assert_eq!(s.project.tracks.len(), 2);
        let h = c(&mut s, "history_list", json!({})); assert_eq!(h["undo"].as_array().unwrap().len(), 2); assert_eq!(h["next_undo"], "add_track");
    }

    #[test] fn checkpoints_are_named_capped_and_restorable() {
        let (mut s, _t) = session();
        for n in ["checkpoint", "restore", "checkpoint_delete"] { refuse(&mut s, n, json!({}), "missing string 'name'"); }
        refuse(&mut s, "checkpoint", json!({"name": "  "}), "name must be 1 to 80 characters");
        refuse(&mut s, "checkpoint", json!({"name": "x".repeat(81)}), "name must be 1 to 80 characters");
        for i in 0..25 { c(&mut s, "checkpoint", json!({"name": format!("c{i}")})); }
        let r = c(&mut s, "checkpoint", json!({"name": "c24"})); let names = r["checkpoints"].as_array().unwrap();
        assert_eq!(names.len(), 24, "the oldest is dropped past 24"); assert_eq!(names[0], "c1");
        refuse(&mut s, "restore", json!({"name": "c0"}), "no checkpoint 'c0'. Existing: c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15, c16, c17, c18, c19, c20, c21, c22, c23, c24");
        c(&mut s, "add_track", json!({})); assert_eq!(s.project.tracks.len(), 2);
        assert_eq!(c(&mut s, "restore", json!({"name": "c3"})), json!({"ok": true, "restored": "c3"})); assert_eq!(s.project.tracks.len(), 1);
        assert_eq!(c(&mut s, "checkpoint_delete", json!({"name": "c3"})), json!({"ok": true}));
        refuse(&mut s, "checkpoint_delete", json!({"name": "c3"}), "no checkpoint 'c3'");
    }

    #[test] fn frame_cache_tools_report_clear_and_bound_the_budget() {
        let (mut s, _t) = session();
        assert_eq!(c(&mut s, "cache_stats", json!({}))["hit_rate"], 0.0, "no lookups yet");
        assert_eq!(c(&mut s, "cache_clear", json!({})), json!({"cleared_entries": 0, "freed_bytes": 0}));
        refuse(&mut s, "cache_set_budget", json!({"megabytes": -1}), "megabytes must be >= 0, got -1");
        refuse(&mut s, "cache_set_budget", json!({}), "missing number 'megabytes'");
        assert_eq!(c(&mut s, "cache_set_budget", json!({"megabytes": 2}))["cap_bytes"], 2 * 1048576);
        let i = c(&mut s, "kimo_info", json!({})); assert!(i["limits"].is_object(), "{i}");
        let g = c(&mut s, "gpu2d_status", json!({})); assert_eq!(g["gpu_available"], g["fallback_reason"].is_null());
    }

    #[test] fn save_project_failures_leave_no_temporary_file() {
        let (mut s, _t) = session();
        // the target is a folder: the rename onto it fails and the .tmp is removed
        let d = tmp("dir_target"); std::fs::create_dir_all(&d).unwrap(); let ds = d.to_str().unwrap().to_string();
        assert!(dispatch(&mut s, "save_project", &json!({"path": ds})).is_err());
        assert!(!std::path::Path::new(&format!("{ds}.tmp")).exists());
        // the history temp name is a folder: the project is saved, the history is not
        let p = tmp("h.kimo"); let ps = p.to_str().unwrap().to_string(); std::fs::create_dir_all(format!("{ps}.history.tmp")).unwrap();
        assert!(dispatch(&mut s, "save_project", &json!({"path": ps, "with_history": true})).is_err());
        assert!(p.is_file());
        // an existing non-empty folder named <path>.history cannot be replaced
        let q = tmp("q.kimo"); let qs = q.to_str().unwrap().to_string(); std::fs::create_dir_all(format!("{qs}.history/x")).unwrap();
        assert!(dispatch(&mut s, "save_project", &json!({"path": qs, "with_history": true})).is_err());
        refuse(&mut s, "save_project", json!({}), "missing string 'path'");
        for x in [d.clone(), std::path::PathBuf::from(format!("{ps}.history.tmp")), std::path::PathBuf::from(format!("{qs}.history"))] { let _ = std::fs::remove_dir_all(x); }
        for x in [p, q] { let _ = std::fs::remove_file(x); }
    }

    #[test] fn load_project_refuses_bad_files_and_newer_schemas() {
        let mut s = Session::new();
        refuse(&mut s, "load_project", json!({}), "missing string 'path'");
        let p = tmp("bad.kimo"); let ps = p.to_str().unwrap();
        std::fs::write(&p, "{\"tracks\": 5}").unwrap(); assert!(err(&mut s, "load_project", json!({"path": ps})).contains("invalid type"));
        std::fs::write(&p, "not json").unwrap(); assert!(dispatch(&mut s, "load_project", &json!({"path": ps})).is_err());
        std::fs::write(&p, json!({"kimo": {"schema_version": 999, "last_saved_with": {"version": "9.9", "commit": "abc"}}, "tracks": []}).to_string()).unwrap();
        assert_eq!(err(&mut s, "load_project", json!({"path": ps})), format!("project schema_version 999 is newer than this build supports ({}); it was saved by kimo 9.9 (commit abc). Update kimo to open it.", crate::buildinfo::SCHEMA_VERSION));
        std::fs::write(&p, json!({"kimo": {"schema_version": 999}, "tracks": []}).to_string()).unwrap();
        assert!(err(&mut s, "load_project", json!({"path": ps})).ends_with("saved by kimo ? (commit ?). Update kimo to open it."));
        // a kimo header without schema_version is a version 1 file
        std::fs::write(&p, json!({"kimo": {"created_with": {"version": "0.1"}}, "name": "t", "markers": [], "assets": {}, "width": 64, "height": 32, "fps": 10, "tracks": []}).to_string()).unwrap();
        assert_eq!(c(&mut s, "load_project", json!({"path": ps}))["tracks"], 0);
        let _ = std::fs::remove_file(&p);
    }

    /// Saves a one-track project with history, then lets `edit` rewrite the history file before loading it back.
    fn reload_with_history(edit: impl Fn(&str, &mut Value)) -> (Session, Result<Value, String>) {
        let (mut s, _t) = session(); let p = tmp("hist.kimo"); let ps = p.to_str().unwrap().to_string();
        c(&mut s, "save_project", json!({"path": ps, "with_history": true}));
        let hp = format!("{ps}.history"); let mut h: Value = serde_json::from_str(&std::fs::read_to_string(&hp).unwrap()).unwrap();
        edit(&std::fs::read_to_string(&p).unwrap(), &mut h); std::fs::write(&hp, h.to_string()).unwrap();
        let mut s2 = Session::new(); let r = dispatch(&mut s2, "load_project", &json!({"path": ps, "with_history": true, "restore": true}));
        let _ = std::fs::remove_file(&p); let _ = std::fs::remove_file(&hp); (s2, r)
    }
    #[test] fn load_project_with_history_checks_the_history_file() {
        let (s, r) = reload_with_history(|_, _| {}); let r = r.unwrap();
        assert_eq!((r["undo_steps"].as_u64(), r["no_history"].as_bool()), (Some(2), Some(true)), "{r}"); assert_eq!(s.undo_labels, vec!["project_set", "add_track"]);
        let (_, r) = reload_with_history(|_, h| { *h = json!({"undo": []}); });
        assert_eq!(r.unwrap()["history_ignored"], "history file has no project_sha (older format)");
        let (_, r) = reload_with_history(|t, h| { h["project_sha"] = json!("0000000000000001"); let _ = t; });
        let r = r.unwrap(); let want = r["history_ignored"].as_str().unwrap().to_string();
        assert!(want.starts_with("history belongs to a different project save (history sha 0000000000000001, project sha "), "{want}");
        let (_, r) = reload_with_history(|t, h| { assert_eq!(h["project_sha"], json!(format!("{:016x}", crate::commands::fnv64(t.as_bytes())))); h["undo_labels"] = json!(["only one"]); });
        assert_eq!(r.unwrap_err(), "history file is inconsistent (undo and labels differ in length)");
        for k in ["undo", "undo_labels", "checkpoints"] { let (_, r) = reload_with_history(|_, h| { h[k] = json!(5); }); assert!(r.unwrap_err().contains("invalid type"), "{k}"); }
    }
    #[test] fn load_project_reports_a_missing_or_unreadable_history_and_dependencies() {
        let (mut s, _t) = session(); let p = tmp("nohist.kimo"); let ps = p.to_str().unwrap().to_string();
        c(&mut s, "save_project", json!({"path": ps}));
        let r = c(&mut s, "load_project", json!({"path": ps, "with_history": true, "report": true}));
        assert!(r["history_ignored"].as_str().unwrap().starts_with("missing ("), "{r}"); assert!(r.get("no_history").is_none());
        assert_eq!(r["dependencies"]["total"], 0);
        std::fs::write(format!("{ps}.history"), "not json").unwrap();
        assert!(dispatch(&mut s, "load_project", &json!({"path": ps, "with_history": true})).is_err());
        let _ = std::fs::remove_file(format!("{ps}.history")); let _ = std::fs::remove_file(&p);
    }
}
