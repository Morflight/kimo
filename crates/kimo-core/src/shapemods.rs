//! Shape-layer modifiers (AE: Offset Paths, Repeater, Merge Paths, Trim Paths, Gradient Stroke).
//! Stored as effects on a Shape clip with kinds `shape_*` (params are animatable Props); the effect pipeline ignores them
//! and the shape rasterizer consumes them here. Everything is signed-distance based, so modifiers compose in effect order.
use crate::model::Effect;
use std::f64::consts::{PI, TAU};

pub const KINDS: &[(&str, &str, &[(&str, f64)])] = &[
    ("shape_offset", "Offset Paths: grow (+) or shrink (-) the outline by `amount` px", &[("amount", 0.0)]),
    ("shape_repeater", "Repeater: `copies` (1-64) copies, each offset by dx,dy px, rotated `rotation` deg and scaled by `scale` per copy (about canvas center); union of copies", &[("copies", 3.0), ("dx", 40.0), ("dy", 0.0), ("rotation", 0.0), ("scale", 1.0)]),
    ("shape_merge", "Merge Paths with a second shape (strings.shape rect|ellipse|polygon|line|path (path uses strings.d), strings.mode union|subtract|intersect|exclude); params w,h,dx,dy,sides,corner_radius", &[("w", 100.0), ("h", 100.0), ("dx", 0.0), ("dy", 0.0), ("sides", 5.0), ("corner_radius", 0.0)]),
    ("shape_trim", "Trim Paths: stroke only between start..end (0..1 of the outline: exact arc length on Bezier paths, angle around the center from -x on primitives), shifted by offset (cycles). Needs stroke_width > 0", &[("start", 0.0), ("end", 1.0), ("offset", 0.0)]),
    ("shape_gradient_stroke", "Gradient Stroke: stroke color blends (r1,g1,b1) -> (r2,g2,b2) across the shape along `angle` deg", &[("r1", 1.0), ("g1", 0.2), ("b1", 0.4), ("r2", 0.2), ("g2", 0.6), ("b2", 1.0), ("angle", 0.0)]),
    ("shape_gradient_fill", "Gradient Fill: fill color blends (r1,g1,b1) -> (r2,g2,b2) across the shape along `angle` deg (same linear blend as Gradient Stroke, applied to the fill instead)", &[("r1", 1.0), ("g1", 0.2), ("b1", 0.4), ("r2", 0.2), ("g2", 0.6), ("b2", 1.0), ("angle", 0.0)]),
];

pub fn sd(shape: &str, sides: u32, hw: f64, hh: f64, cr: f64, px: f64, py: f64) -> f64 {
    match shape {
        "ellipse" => { let k = ((px / hw.max(1e-6)).powi(2) + (py / hh.max(1e-6)).powi(2)).sqrt(); (k - 1.0) * hw.min(hh) }
        "polygon" => { let n = sides.max(3) as f64; let r = hw.min(hh); let a = py.atan2(px) + PI / 2.0; let seg = TAU / n; let aa = a.rem_euclid(seg) - seg / 2.0; (px * px + py * py).sqrt() * aa.cos() - r * (seg / 2.0).cos() }
        "line" => (py.abs() - hh.max(1.0)).max(px.abs() - hw),
        _ => { let (qx, qy) = (px.abs() - hw + cr, py.abs() - hh + cr); (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - cr }
    }
}

pub struct Stack<'a> { pub path: Option<crate::paths::Path>, pub dist: Box<dyn Fn(f64, f64) -> f64 + 'a>, pub trim: Option<(f64, f64, f64)>, pub grad: Option<([f64; 3], [f64; 3], f64)>, pub grad_fill: Option<([f64; 3], [f64; 3], f64)> }

/// Build the distance function (pixel coords) for `base` modified by the clip's shape_* effects at clip-local time `t`.
/// The clip's Bezier path (effect `shape_path`: strings.d = SVG path, strings.fill_rule = nonzero|evenodd), if any.
pub fn path_of(effects: &[Effect], t: f64) -> Option<crate::paths::Path> { let e = effects.iter().find(|e| e.kind == "shape_path")?; let keys: Vec<(f64, String, String)> = e.strings.get("keys").and_then(|k| serde_json::from_str::<Vec<(f64, String, String)>>(k).ok()).unwrap_or_default(); crate::paths::Path::parse_at(e.strings.get("d")?, &keys, t, e.strings.get("fill_rule").map_or(false, |r| r == "evenodd")).ok() }

pub fn build<'a>(effects: &'a [Effect], base: impl Fn(f64, f64) -> f64 + 'a, cx: f64, cy: f64, t: f64) -> Stack<'a> {
    let mut dist: Box<dyn Fn(f64, f64) -> f64 + 'a> = Box::new(base);
    let (mut trim, mut grad, mut grad_fill) = (None, None, None);
    // outline parameter for the path operators: arc length on a Bezier path, angle around the centre on primitives (as trim uses)
    let pth = path_of(effects, t); let rmax = 2.0 * cx.hypot(cy);
    let u_of: std::rc::Rc<dyn Fn(f64, f64) -> f64 + 'a> = match &pth { Some(p) => { let p = p.clone(); std::rc::Rc::new(move |x, y| p.nearest(x, y).1) } None => std::rc::Rc::new(|x: f64, y: f64| (y.atan2(x) + PI) / TAU) };
    for e in effects.iter().filter(|e| e.kind.starts_with("shape_")) {
        let g = |k: &str, d: f64| e.params.get(k).map(|p| p.at(t)).unwrap_or(d);
        let prev = dist;
        match e.kind.as_str() {
            "shape_offset" => { let a = g("amount", 0.0); dist = Box::new(move |x, y| prev(x, y) - a); }
            "shape_repeater" => { let n = (g("copies", 3.0).round() as i32).clamp(1, 64); let (dx, dy, rot, sc) = (g("dx", 40.0), g("dy", 0.0), g("rotation", 0.0).to_radians(), g("scale", 1.0).clamp(0.05, 20.0));
                dist = Box::new(move |x, y| { let mut best = f64::MAX; for i in 0..n { let s = sc.powi(i); let (px, py) = (x - cx - dx * i as f64, y - cy - dy * i as f64); let a = -rot * i as f64; let (ca, sa) = (a.cos(), a.sin()); let (qx, qy) = ((px * ca - py * sa) / s, (px * sa + py * ca) / s); best = best.min(prev(qx + cx, qy + cy) * s); } best }); }
            "shape_merge" => { let shape = e.strings.get("shape").cloned().unwrap_or("ellipse".into()); let mode = e.strings.get("mode").cloned().unwrap_or("union".into());
                let (hw, hh, dx, dy, sides, cr) = (g("w", 100.0) / 2.0, g("h", 100.0) / 2.0, g("dx", 0.0), g("dy", 0.0), g("sides", 5.0) as u32, g("corner_radius", 0.0));
                let p2 = if shape == "path" { e.strings.get("d").and_then(|d| crate::paths::Path::parse(d, false).ok()) } else { None };
                dist = Box::new(move |x, y| { let a = prev(x, y); let b = match &p2 { Some(p) => p.sd(x - cx - dx, y - cy - dy), None => sd(&shape, sides, hw, hh, cr, x - cx - dx, y - cy - dy) }; match mode.as_str() { "subtract" => a.max(-b), "intersect" => a.max(b), "exclude" => a.min(b).max(-(a.max(b))), _ => a.min(b) } }); }
            "shape_trim" => { trim = Some((g("start", 0.0), g("end", 1.0), g("offset", 0.0))); dist = prev; }
            "shape_gradient_stroke" => { grad = Some(([g("r1", 1.0), g("g1", 0.2), g("b1", 0.4)], [g("r2", 0.2), g("g2", 0.6), g("b2", 1.0)], g("angle", 0.0).to_radians())); dist = prev; }
            "shape_gradient_fill" => { grad_fill = Some(([g("r1", 1.0), g("g1", 0.2), g("b1", 0.4)], [g("r2", 0.2), g("g2", 0.6), g("b2", 1.0)], g("angle", 0.0).to_radians())); dist = prev; }
            k if k.starts_with("shape_op_") => { dist = crate::pathops::apply(e, prev, cx, cy, t, u_of.clone(), rmax); }
            _ => { dist = prev; }
        }
    }
    Stack { path: pth, dist, trim, grad, grad_fill }
}

impl Stack<'_> {
    /// 0..1 visibility of the stroke at (px,py) relative to shape center under Trim Paths.
    pub fn trim_gate(&self, px: f64, py: f64) -> f64 {
        let Some((s, e, off)) = self.trim else { return 1.0 };
        let (s, e) = (s.clamp(0.0, 1.0), e.clamp(0.0, 1.0)); if e - s >= 0.9999 && off == 0.0 { return 1.0; }
        // exact arc length on Bezier paths; angle around the center on primitives
        let u0 = match &self.path { Some(p) => p.nearest(px, py).1, None => (py.atan2(px) + PI) / TAU };
        let u = (u0 - off).rem_euclid(1.0); if u >= s && u <= e { 1.0 } else { 0.0 }
    }
    pub fn stroke_color(&self, px: f64, py: f64, w: f64, h: f64, base: [f64; 3]) -> [f64; 3] {
        let Some((c1, c2, a)) = self.grad else { return base };
        let ext = (w.max(h) / 2.0).max(1.0); let k = ((px * a.cos() + py * a.sin()) / ext * 0.5 + 0.5).clamp(0.0, 1.0);
        [c1[0] + (c2[0] - c1[0]) * k, c1[1] + (c2[1] - c1[1]) * k, c1[2] + (c2[2] - c1[2]) * k]
    }
    /// Same linear blend as `stroke_color`, for Gradient Fill (a separate modifier/colour pair so a shape can carry
    /// a solid stroke over a gradient fill, or vice versa, at once).
    pub fn fill_color(&self, px: f64, py: f64, w: f64, h: f64, base: [f64; 3]) -> [f64; 3] {
        let Some((c1, c2, a)) = self.grad_fill else { return base };
        let ext = (w.max(h) / 2.0).max(1.0); let k = ((px * a.cos() + py * a.sin()) / ext * 0.5 + 0.5).clamp(0.0, 1.0);
        [c1[0] + (c2[0] - c1[0]) * k, c1[1] + (c2[1] - c1[1]) * k, c1[2] + (c2[2] - c1[2]) * k]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyframe::Prop;
    fn fx(kind: &str, p: &[(&str, f64)], s: &[(&str, &str)]) -> Effect {
        Effect { id: "fx".into(), kind: kind.into(), params: p.iter().map(|(k, v)| (k.to_string(), Prop::new(*v))).collect(), strings: s.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }
    /// disk of radius 10 centred on (50, 50)
    fn disk(x: f64, y: f64) -> f64 { (x - 50.0).hypot(y - 50.0) - 10.0 }
    fn dist(e: &[Effect], x: f64, y: f64) -> f64 { (build(e, disk, 50.0, 50.0, 0.0).dist)(x, y) }
    #[test] fn primitive_signed_distances() {
        assert_eq!(sd("ellipse", 0, 10.0, 5.0, 0.0, 0.0, 0.0), -5.0); assert_eq!(sd("ellipse", 0, 10.0, 5.0, 0.0, 10.0, 0.0), 0.0);
        assert!((sd("polygon", 4, 10.0, 10.0, 0.0, 0.0, 0.0) + 10.0 * (PI / 4.0).cos()).abs() < 1e-12);
        assert_eq!(sd("line", 0, 10.0, 0.5, 0.0, 0.0, 0.0), -1.0, "line half-thickness is at least 1 px");
        assert!((sd("rect", 0, 10.0, 10.0, 2.0, 10.0, 10.0) - (8f64.sqrt() - 2.0)).abs() < 1e-12); assert_eq!(sd("rect", 0, 10.0, 10.0, 0.0, 0.0, 0.0), -10.0);
    }
    #[test] fn offset_and_repeater_modify_the_distance() {
        assert_eq!(dist(&[fx("shape_offset", &[("amount", 2.0)], &[])], 50.0, 50.0), -12.0);
        assert_eq!(dist(&[fx("shape_repeater", &[("copies", 2.0), ("dx", 30.0)], &[])], 80.0, 50.0), -10.0, "second copy 30 px right");
        assert!((dist(&[fx("shape_repeater", &[("copies", 2.0), ("dx", 0.0), ("scale", 2.0), ("rotation", 90.0)], &[])], 50.0, 65.0) + 5.0).abs() < 1e-9, "scaled, rotated copy reaches (50,65)");
    }
    #[test] fn merge_modes_combine_with_a_second_shape() {
        let m = |mode: &str| vec![fx("shape_merge", &[("w", 10.0), ("h", 10.0), ("dx", 8.0)], &[("shape", "rect"), ("mode", mode)])];
        for (mode, want) in [("union", [-1.0, -4.0, -5.0]), ("subtract", [2.0, 3.0, -5.0]), ("intersect", [2.0, -3.0, 8.0]), ("exclude", [-1.0, 3.0, -5.0])] {
            let e = m(mode); for (i, x) in [62.0, 56.0, 45.0].iter().enumerate() { assert_eq!(dist(&e, *x, 50.0), want[i], "{mode} at x {x}"); }
        }
        let p = [fx("shape_merge", &[], &[("shape", "path"), ("mode", "intersect"), ("d", "M -5 -5 L 5 -5 L 5 5 L -5 5 Z")])];
        assert!((dist(&p, 50.0, 50.0) + 5.0).abs() < 1e-6, "intersection with the 10 px path square");
        assert!((dist(&[fx("shape_merge", &[], &[])], 95.0, 50.0) + 5.0).abs() < 1e-9, "default: union with a 100 px ellipse");
        assert_eq!(dist(&[fx("shape_unknown", &[("amount", 9.0)], &[]), fx("blur", &[], &[])], 50.0, 50.0), -10.0, "unknown modifiers are ignored");
    }
    #[test] fn trim_gate_follows_the_outline_parameter() {
        assert_eq!(build(&[], disk, 50.0, 50.0, 0.0).trim_gate(1.0, 0.01), 1.0);
        let half = [fx("shape_trim", &[("start", 0.0), ("end", 0.5)], &[])]; let s = build(&half, disk, 50.0, 50.0, 0.0);
        assert_eq!(s.trim_gate(-1.0, -0.01), 1.0); assert_eq!(s.trim_gate(1.0, 0.01), 0.0);
        let full = [fx("shape_trim", &[("offset", 0.25)], &[])]; let s = build(&full, disk, 50.0, 50.0, 0.0); assert_eq!(s.trim_gate(1.0, 0.01), 1.0); assert_eq!(s.trim_gate(-1.0, -0.01), 1.0);
        let path = [fx("shape_path", &[], &[("d", "M 0 0 L 100 0")]), fx("shape_trim", &[("end", 0.5)], &[])]; let s = build(&path, disk, 50.0, 50.0, 0.0);
        assert_eq!(s.trim_gate(10.0, 0.0), 1.0); assert_eq!(s.trim_gate(90.0, 0.0), 0.0);
    }
    #[test] fn gradients_blend_along_the_angle() {
        let plain = build(&[], disk, 50.0, 50.0, 0.0); assert_eq!(plain.stroke_color(5.0, 0.0, 20.0, 20.0, [0.1; 3]), [0.1; 3]); assert_eq!(plain.fill_color(5.0, 0.0, 20.0, 20.0, [0.1; 3]), [0.1; 3]);
        let bw = [("r1", 0.0), ("g1", 0.0), ("b1", 0.0), ("r2", 1.0), ("g2", 1.0), ("b2", 1.0)]; let wb = [("r1", 1.0), ("g1", 1.0), ("b1", 1.0), ("r2", 0.0), ("g2", 0.0), ("b2", 0.0)];
        let stroke = [fx("shape_gradient_stroke", &bw, &[])];
        let s = build(&stroke, disk, 50.0, 50.0, 0.0); assert_eq!(s.stroke_color(5.0, 0.0, 20.0, 20.0, [0.1; 3]), [0.75; 3]);
        let fill = [fx("shape_gradient_fill", &wb, &[])];
        let s = build(&fill, disk, 50.0, 50.0, 0.0); assert_eq!(s.fill_color(5.0, 0.0, 20.0, 20.0, [0.1; 3]), [0.25; 3]);
    }
    #[test] fn path_of_reads_the_shape_path_effect() {
        assert!(path_of(&[fx("shape_path", &[], &[("d", "M 0 0 L 10 0"), ("keys", "not json")])], 0.0).is_some());
        assert!(path_of(&[fx("shape_path", &[], &[])], 0.0).is_none()); assert!(path_of(&[], 0.0).is_none());
    }
}
