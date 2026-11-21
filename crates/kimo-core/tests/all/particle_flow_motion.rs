//! render_particle_flow_motion (ext/particle_flow_motion.rs) on a tiny target: a 16-slot emitter, 0.5 s of simulation, 32x24 PNG.
use kimo_core::commands::{dispatch, Session};
use kimo_core::particle_advect::Advect;
use kimo_core::particle_flow::{Blend, ColorStop, Emitter, Flow};
use serde_json::{json, Value};

fn flow() -> Value {
    serde_json::to_value(Flow {
        emitter: Emitter { seed: 11, count: 16, origin: [0.0; 3], spread: [0.2, 0.1, 0.2], base_velocity: [0.0, 1.5, 0.0], velocity_jitter: 0.3, birth_rate: 40.0, lifetime: [1.0, 2.0],
            color_stops: vec![ColorStop { offset: 0.0, rgb: [1.0, 0.8, 0.2] }, ColorStop { offset: 1.0, rgb: [0.2, 0.1, 0.6] }], alpha_start: 0.9, alpha_end: 0.6, fade_in: 0.1, fade_out: 0.15,
            size_start: 0.05, size_end: 0.1, blend: Blend::Additive, hardness: 4.0 },
        field: Advect::Curl3 { noise: "perlin".into(), seed: 3, frequency: 0.8, amplitude: 1.0, octaves: 2 }, gravity: [0.0, -0.5, 0.0], drag: 0.1, coupling: 2.0, field_accel: 0.0, dt: 0.01,
    }).unwrap()
}
fn camera() -> Value { json!({"eye": [0.0, 0.5, -5.0], "pan": 0.0, "tilt": 0.0, "roll": 0.0, "sensor_height": 24.0, "focal_length": 50.0, "near": 0.1}) }

#[test]
fn renders_the_simulated_particles() {
    let mut s = Session::new(); let png = std::env::temp_dir().join(format!("kimo-pfm-{}.png", std::process::id())); let ps = png.to_str().unwrap().to_string();
    let base = json!({"flow": flow(), "camera": camera(), "time": 0.5, "width": 32, "height": 24, "path": ps});
    let r = dispatch(&mut s, "render_particle_flow_motion", &base).unwrap();
    // 40 births per second for 0.5 s overflow the 16-slot ring; every particle lives at least 1 s, so all 16 slots are alive
    assert_eq!(r["active"], 16); assert!(r["drawn"].as_u64().unwrap() > 0 && r["drawn"].as_u64().unwrap() <= 16, "{r}"); assert!((r["sim_time"].as_f64().unwrap() - 0.5).abs() < 1e-9);
    let b = std::fs::read(&png).unwrap(); assert_eq!(&b[1..4], b"PNG"); std::fs::remove_file(&png).unwrap();
}
