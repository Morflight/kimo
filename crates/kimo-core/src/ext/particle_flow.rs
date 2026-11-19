use super::kit::*;
fn read<T: serde::de::DeserializeOwned>(a: &Value, k: &str) -> Result<T, String> { serde_json::from_value(a.get(k).ok_or_else(|| format!("{k} required"))?.clone()).map_err(|e| e.to_string()) }
pub fn commands() -> Vec<Command> {
    vec![Command { name: "render_particle_flow", description: "Render a deterministic seeded particle emitter advected by a 3D curl, potential or Worley flow field. Deterministic seeded particle emitter advected by a curl3/potential3/worley3 field (semi-implicit Euler, gravity, drag, field-velocity coupling), recycled through a preallocated ring pool (no per-frame heap allocation), smoothstep fade-in/out, cubic-eased size bump (small at birth and death, peak mid-life), multi-stop gradient color; camera-v2 perspective depth-splat with distance-attenuated footprint, radial gaussian kernel of configurable hardness, additive or premultiplied-alpha blending, tiled inner loop, z-buffer occlusion against optional mesh depth; PNG via the CPU image encoder. Inputs flow {emitter,field,gravity,drag,coupling,dt}, camera, time, width, height, optional mesh, depth_range, path. Budget: 1..100000 particles, dt (0,1], 2..=8 color stops, target 1..1920x1080. Limits: correctness-first CPU path; no motion blur, trails, or fluid coupling.", mutates: false, schema: obj(vec![("flow", json!({"type":"object"})), ("camera", json!({"type":"object"})), ("time", json!({"type":"number"})), ("width", json!({"type":"integer"})), ("height", json!({"type":"integer"})), ("mesh", json!({"type":"object"})), ("depth_range", json!({"type":"array"})), ("path", s("PNG output"))], &["flow", "camera", "time", "width", "height", "path"]), run: |_, a| {
        let flow: crate::particle_flow::Flow = read(a, "flow")?;
        let camera: crate::camera_projection::Camera = read(a, "camera")?;
        let time = num_arg(a, "time")?;
        let w = num_arg(a, "width")? as usize;
        let h = num_arg(a, "height")? as usize;
        let path = str_arg(a, "path")?;
        if !path.to_lowercase().ends_with(".png") {
            return Err("PNG path required".into());
        }
        let mesh: Option<crate::threed::primitives::Geometry> = if a.get("mesh").map(|m| m.is_null()).unwrap_or(true) { None } else { Some(read(a, "mesh")?) };
        let dr: Vec<f64> = if a.get("depth_range").map(|m| m.is_null()).unwrap_or(true) { vec![0.5, 100.0] } else { read(a, "depth_range")? };
        if dr.len() != 2 {
            return Err("depth_range [near,far]".into());
        }
        let mut sim = crate::particle_flow::Simulation::new(flow)?;
        sim.advance(time)?;
        let (frame, out) = crate::particle_flow::render(&mut sim, &camera, w, h, mesh.as_ref(), [dr[0], dr[1]])?;
        crate::render::write_image(&frame, path)?;
        Ok(json!({"path":path,"active":out.active,"drawn":out.drawn,"occluded_by_mesh":out.occluded_by_mesh,"behind_camera":out.behind_camera,"sim_time":sim.time(),"backend":"cpu_camera_v2_depth_splat"}))
    } }]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args() -> Value {
        json!({"flow": {"emitter": {"seed": 1, "count": 4, "origin": [0, 0, 0], "spread": [0.1, 0.1, 0.1], "base_velocity": [0, 0, 0], "velocity_jitter": 0, "birth_rate": 40, "lifetime": [1, 2],
                "color_stops": [{"offset": 0, "rgb": [1, 1, 1]}, {"offset": 1, "rgb": [1, 1, 1]}], "alpha_start": 1, "alpha_end": 1, "fade_in": 0.1, "fade_out": 0.1, "size_start": 0.05, "size_end": 0.05, "blend": "additive", "hardness": 4},
                "field": {"kind": "curl3", "noise": "perlin", "seed": 1, "frequency": 1, "amplitude": 0, "octaves": 1}, "gravity": [0, 0, 0], "drag": 0, "coupling": 0, "field_accel": 0, "dt": 0.01},
            "camera": {"eye": [0, 0, -4], "pan": 0, "tilt": 0, "roll": 0, "sensor_height": 24, "focal_length": 35, "near": 0.1}, "time": 0.2, "width": 16, "height": 12, "path": "/nonexistent-kimo-dir/flow.png"})
    }
    fn run(a: Value) -> Result<Value, String> { (commands()[0].run)(&mut crate::commands::Session::new(), &a) }
    fn without(k: &str) -> Value { let mut a = args(); a.as_object_mut().unwrap().remove(k); a }
    fn with(k: &str, v: Value) -> Value { let mut a = args(); a[k] = v; a }
    #[test]
    fn render_particle_flow_refuses_missing_and_malformed_inputs() {
        assert_eq!(run(without("flow")).unwrap_err(), "flow required");
        assert_eq!(run(without("camera")).unwrap_err(), "camera required");
        for k in ["time", "width", "height", "path"] { let e = run(without(k)).unwrap_err(); assert!(e.contains(k), "{e}"); }
        assert!(run(with("camera", json!(3))).unwrap_err().contains("invalid type"));
        assert_eq!(run(with("path", json!("x.jpg"))).unwrap_err(), "PNG path required");
        assert!(run(with("mesh", json!(5))).unwrap_err().contains("invalid type"));
        assert!(run(with("depth_range", json!("near"))).unwrap_err().contains("invalid type"));
        assert_eq!(run(with("depth_range", json!([1]))).unwrap_err(), "depth_range [near,far]");
        let mut a = args(); a["flow"]["dt"] = json!(0); assert_eq!(run(a).unwrap_err(), "dt (0, 1]");
        assert_eq!(run(with("time", json!(-1))).unwrap_err(), "advance requires current<=to_time<=1e4");
        assert_eq!(run(with("width", json!(4000))).unwrap_err(), "target 1..1920x1080");
    }
    #[test]
    fn render_particle_flow_reports_an_unwritable_path_and_writes_a_png() {
        assert!(run(args()).is_err(), "directory does not exist");
        let d = std::env::temp_dir().join(format!("kimo-pflow-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap(); let p = d.join("flow.png");
        let r = run(with("path", json!(p.to_str().unwrap()))).unwrap(); assert_eq!(r["active"], 4); assert_eq!(r["behind_camera"], 0); assert_eq!(r["sim_time"], 0.2); assert!(p.exists());
        std::fs::remove_dir_all(d).unwrap();
    }
}
