//! render_particle_flow (3D flow-field particles, particle_flow.rs) through its command, on a 16x12 target.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};

fn flow_json() -> Value { json!({"emitter": {"seed": 11, "count": 64, "origin": [0.0, 0.0, 0.0], "spread": [0.2, 0.1, 0.2], "base_velocity": [0.0, 1.5, 0.0], "velocity_jitter": 0.3, "birth_rate": 40.0, "lifetime": [1.0, 2.0],
    "color_stops": [{"offset": 0.0, "rgb": [1.0, 0.8, 0.2]}, {"offset": 0.5, "rgb": [1.0, 0.3, 0.05]}, {"offset": 1.0, "rgb": [0.2, 0.1, 0.6]}], "alpha_start": 0.9, "alpha_end": 0.6, "fade_in": 0.1, "fade_out": 0.15,
    "size_start": 0.02, "size_end": 0.08, "blend": "additive", "hardness": 4.0},
    "field": {"kind": "curl3", "noise": "perlin", "seed": 3, "frequency": 0.8, "amplitude": 1.0, "octaves": 2}, "gravity": [0.0, -0.5, 0.0], "drag": 0.1, "coupling": 2.0, "field_accel": 0.0, "dt": 0.01}) }

#[test]
fn render_particle_flow_command_writes_the_simulated_frame() {
    let mut s = Session::new();
    let path = std::env::temp_dir().join(format!("kimo_pflow_{}.png", std::process::id())); let p = path.to_str().unwrap().to_string();
    let a = json!({"flow": flow_json(), "camera": {"eye": [0.0, 0.5, 5.0], "pan": 0.0, "tilt": 0.0, "roll": 0.0, "sensor_height": 24.0, "focal_length": 20.0, "near": 0.1}, "time": 0.5, "width": 16, "height": 12, "path": p, "mesh": null, "depth_range": [0.5, 50.0]});
    let r = dispatch(&mut s, "render_particle_flow", &a); let ok = path.exists(); let _ = std::fs::remove_file(&path); let r = r.unwrap();
    assert!(ok, "PNG written"); assert_eq!(r["sim_time"], json!(0.5)); assert!(r["active"].as_u64().unwrap() > 0); assert_eq!(r["backend"], "cpu_camera_v2_depth_splat");
}
