//! add_pip (KF14): a one-call picture-in-picture inset of a second video over the current one --
//! a corner webcam bubble, a floating circular avatar, or a TV-frame-style cutaway, the look KR2
//! found on 4 of 8 top creators (Johnny Harris's stylized TV-frame cutaways, Rober's and Abdaal's
//! circular floating webcam/avatar bubbles) with no single named tool to make it (KR2: "the single
//! most common un-packaged look ... no named tool"; add_pip was its #1 ranked proposal).
//!
//! EXISTING-TOOL CHECK (done first, per the brief): the registry has no overlay/inset/picture_in_picture/
//! add_overlay/compositing tool (grepped golden_registry.txt and every ext/*.rs and commands/tools_*.rs
//! for "pip", "picture_in_picture", "overlay", "inset" -- nothing). What DOES exist, and what this file
//! reuses rather than reinventing: `add_track` + `add_clip` (a second media clip on its own track IS
//! how compositing already works here -- higher tracks draw over lower ones), `add_mask` (kind=rect
//! with `round`, or kind=ellipse, to crop the inset to a rounded-rect or circle -- the exact mechanism
//! KR2's own Kimo-mapping section named: "achievable manually: add_clip ... set_property ... add_mask
//! or add_shape for a circular crop and a border/shadow"), `add_shape` (for the border ring and the
//! drop-shadow silhouette, same shape, stroke-only / fill-only), `set_property` / `set_keyframe` (for
//! position, scale and the pop/slide/fade animation), and `set_audio` (mute_source). Every mutation
//! below goes through the real registered tool via `dispatch_nested`, the same "no existing primitive
//! changes, call through dispatch_nested" pattern `drop_short_segments`/`trim_filler_words` already use.
//!
//! COORDINATE MATH (read from render.rs/masks.rs, not guessed): a media clip is decoded and fit-resized
//! to the full project canvas (width x height) BEFORE `scale`/`x`/`y` are applied (render.rs: `fit` runs
//! in `prepare_frame_sized`, upstream of the one place `Xform` reads scale/x/y). So with `fit: "cover"`,
//! a clip's own pre-transform frame is always exactly canvas-sized regardless of the final inset size,
//! and a uniform `scale` of `size` (size = fraction of frame WIDTH) yields an on-screen box of
//! `size*canvas_width` x `size*canvas_height` -- canvas aspect, not source aspect, which is what "cover"
//! is for. Mask `rect`/`ellipse` params (cx,cy,w,h,round) are normalized to that SAME pre-transform
//! canvas-sized frame (masks.rs::raw: `mw = w*wf/2`, `wf` = the frame passed in, i.e. canvas width) --
//! so a circle inscribed in a non-square box needs `w_norm = h_norm = min(canvas_w,canvas_h)/{canvas_w,
//! canvas_h}` to get equal pixel radii on both axes, not w=h=1 (which would be elliptical on a 16:9
//! canvas). The border/shadow `add_shape` clips are NOT canvas-fit (shapes draw at their literal
//! width/height), so they're sized directly in on-screen px (box_w, box_h, or the same inscribed-circle
//! diameter), matching the mask's effective on-screen footprint exactly (verified by the real-check
//! renders below).
use super::kit::*;
use crate::commands::dispatch_nested as call;
use crate::model::Source;

const ENTER_EXIT_S: f64 = 0.25; // ~0.25s scale-overshoot, per KR2's measured "pop/scale-overshoot-in" entrance rule
const ROUND_FRAC: f64 = 0.18; // corner radius as a fraction of min(box_w,box_h)/2, a visible-but-modest rounding (KR2's TV-frame examples)
const MARGIN_FRAC: f64 = 0.03; // inset's gap from the canvas edge, as a fraction of min(canvas_w,canvas_h)

fn opt_num_checked(a: &Value, k: &str, d: f64) -> Result<f64, String> {
    match a.get(k) { None | Some(Value::Null) => Ok(d), Some(v) => v.as_f64().ok_or_else(|| crate::errors::arg_err(a, k, "number")) }
}
fn opt_bool(a: &Value, k: &str, d: bool) -> Result<bool, String> {
    match a.get(k) { None | Some(Value::Null) => Ok(d), Some(v) => v.as_bool().ok_or_else(|| crate::errors::arg_err(a, k, "boolean")) }
}
fn opt_str<'a>(a: &'a Value, k: &str, d: &'a str) -> Result<&'a str, String> {
    match a.get(k) { None | Some(Value::Null) => Ok(d), Some(v) => v.as_str().ok_or_else(|| crate::errors::arg_err(a, k, "string")) }
}
fn opt_color(a: &Value, k: &str, d: [f64; 4]) -> Result<[f64; 4], String> {
    let Some(v) = a.get(k).filter(|v| !v.is_null()) else { return Ok(d) };
    let arr = v.as_array().ok_or_else(|| format!("add_pip: '{k}' must be an RGB or RGBA array, channels 0..1"))?;
    if !(3..=4).contains(&arr.len()) { return Err(format!("add_pip: '{k}' must have 3 or 4 channels")); }
    let mut out = d;
    for (i, c) in arr.iter().enumerate() { out[i] = c.as_f64().filter(|x| x.is_finite() && (0.0..=1.0).contains(x)).ok_or_else(|| format!("add_pip: '{k}' channels must be finite numbers in 0..1"))?; }
    if arr.len() == 3 { out[3] = 1.0; }
    Ok(out)
}
fn rgba_json(c: [f64; 4]) -> Value { json!([c[0], c[1], c[2], c[3]]) }

/// Box top-left-relative center offset (px from canvas center) for the 5 named corners/center, or a
/// normalized {x,y} object (0..1 of the canvas, (0.5,0.5) = center of the canvas = where the inset's
/// own center sits). x is rightward-positive, y is downward-positive (same convention add_text/
/// lower_third already use: lower_third's own "lower" y default is positive).
fn resolve_position(a: &Value, cw: f64, ch: f64, half_w: f64, half_h: f64) -> Result<(f64, f64), String> {
    let margin = MARGIN_FRAC * cw.min(ch);
    let pos = a.get("position").cloned().unwrap_or(json!("top_right"));
    if let Some(s) = pos.as_str() {
        return Ok(match s {
            "top_right" => (cw / 2.0 - half_w - margin, -(ch / 2.0) + half_h + margin),
            "top_left" => (-(cw / 2.0) + half_w + margin, -(ch / 2.0) + half_h + margin),
            "bottom_right" => (cw / 2.0 - half_w - margin, ch / 2.0 - half_h - margin),
            "bottom_left" => (-(cw / 2.0) + half_w + margin, ch / 2.0 - half_h - margin),
            "center" => (0.0, 0.0),
            o => return Err(format!("add_pip: unknown position '{o}'; use top_right, top_left, bottom_right, bottom_left, center, or {{x,y}} normalized")),
        });
    }
    if let Some(o) = pos.as_object() {
        let x = o.get("x").and_then(|v| v.as_f64()).filter(|v| v.is_finite()).ok_or("add_pip: position.x must be a finite number")?;
        let y = o.get("y").and_then(|v| v.as_f64()).filter(|v| v.is_finite()).ok_or("add_pip: position.y must be a finite number")?;
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) { return Err("add_pip: position.x and position.y must be within 0..1".into()); }
        return Ok(((x - 0.5) * cw, (y - 0.5) * ch));
    }
    Err("add_pip: position must be a string (top_right|top_left|bottom_right|bottom_left|center) or {x,y} normalized 0..1".into())
}

/// One property's full keyframe-or-constant setup. `enter`/`exit` are each `Some((start_value, ease))`
/// when that side of this property is animated, `None` to just hold `steady` the whole clip. Builds the
/// same always-4-key shape lower_third.rs uses (entry, hold-start, hold-end, exit) so an inactive side
/// behaves as a plain constant: its key equals `steady` with ease "hold", which is indistinguishable
/// from not animating that side at all.
#[allow(clippy::too_many_arguments)]
fn animate_prop(ss: &mut Session, clip_id: &str, property: &str, steady: f64, duration_s: f64, enter: Option<(f64, &str)>, exit: Option<(f64, &str)>) -> Result<(), String> {
    if enter.is_none() && exit.is_none() {
        call(ss, "set_property", &json!({"clip_id": clip_id, "property": property, "value": steady}))?;
        return Ok(());
    }
    let enter_dur = ENTER_EXIT_S.min(duration_s / 3.0);
    let exit_dur = ENTER_EXIT_S.min(duration_s / 3.0);
    let (v0, e_in) = enter.unwrap_or((steady, "hold"));
    let (v3, e_out) = exit.unwrap_or((steady, "hold"));
    call(ss, "set_keyframe", &json!({"clip_id": clip_id, "property": property, "time": 0.0, "value": v0, "ease": e_in}))?;
    call(ss, "set_keyframe", &json!({"clip_id": clip_id, "property": property, "time": enter_dur, "value": steady, "ease": "hold"}))?;
    call(ss, "set_keyframe", &json!({"clip_id": clip_id, "property": property, "time": (duration_s - exit_dur).max(enter_dur), "value": steady, "ease": e_out}))?;
    call(ss, "set_keyframe", &json!({"clip_id": clip_id, "property": property, "time": duration_s, "value": v3, "ease": "hold"}))?;
    Ok(())
}

fn run(ss: &mut Session, a: &Value) -> Result<Value, String> {
    const KNOWN: &[&str] = &["source", "at_s", "duration_s", "position", "size", "shape", "border_px", "border_color", "shadow", "animate_in", "animate_out", "source_in_s", "mute_source"];
    if let Some(o) = a.as_object() { for k in o.keys() { if !KNOWN.contains(&k.as_str()) { return Err(format!("add_pip: unknown argument '{k}'")); } } } else { return Err("add_pip: arguments must be an object".into()); }

    let source = str_arg(a, "source")?.to_string();
    let at_s = num_arg(a, "at_s")?;
    let duration_s = num_arg(a, "duration_s")?;
    if !(at_s.is_finite() && at_s >= 0.0) { return Err("add_pip: at_s must be a finite number >= 0".into()); }
    if !(duration_s.is_finite() && duration_s > 0.0) { return Err("add_pip: duration_s must be a finite number > 0".into()); }

    let size = opt_num_checked(a, "size", 0.3)?;
    if !(size.is_finite() && size > 0.0 && size <= 1.0) { return Err("add_pip: size must be a finite number in (0, 1]".into()); }

    let shape = opt_str(a, "shape", "rounded")?.to_string();
    if !["rect", "rounded", "circle"].contains(&shape.as_str()) { return Err(format!("add_pip: shape must be rect, rounded or circle, got '{shape}'")); }

    let border_px = opt_num_checked(a, "border_px", 0.0)?;
    if !(border_px.is_finite() && border_px >= 0.0) { return Err("add_pip: border_px must be a finite number >= 0".into()); }
    let border_color = opt_color(a, "border_color", [1.0, 1.0, 1.0, 1.0])?;

    let shadow = opt_bool(a, "shadow", true)?;

    let animate_in = opt_str(a, "animate_in", "pop")?.to_string();
    let animate_out = opt_str(a, "animate_out", "fade")?.to_string();
    for (k, v) in [("animate_in", &animate_in), ("animate_out", &animate_out)] {
        if !["pop", "slide", "fade", "none"].contains(&v.as_str()) { return Err(format!("add_pip: {k} must be pop, slide, fade or none, got '{v}'")); }
    }

    let source_in_s = opt_num_checked(a, "source_in_s", 0.0)?;
    if !(source_in_s.is_finite() && source_in_s >= 0.0) { return Err("add_pip: source_in_s must be a finite number >= 0".into()); }
    let mute_source = opt_bool(a, "mute_source", true)?;

    if source.trim().is_empty() { return Err("add_pip: source must not be empty".into()); }

    let cw = ss.project.width as f64;
    let ch = ss.project.height as f64;
    let box_w = size * cw;
    let box_h = size * ch;
    let half_w = box_w / 2.0;
    let half_h = box_h / 2.0;
    let (x_off, y_off) = resolve_position(a, cw, ch, half_w, half_h)?;

    // Resolve source: an existing media clip's own asset (a new independent clip is still created --
    // the original clip is never touched), or a file path imported fresh. import_media's own error
    // ("no such file"/bad format) is what a bad path surfaces as, not a bespoke check here.
    let asset_id: String = match ss.project.find_clip(&source) {
        Some(c) => match &c.source {
            Source::Media { asset } => asset.clone(),
            _ => return Err(format!("add_pip: clip '{source}' is not a media clip; source must be a media clip id or a file path")),
        },
        None => {
            let r = call(ss, "import_media", &json!({"path": source}))?;
            r["asset_id"].as_str().ok_or("add_pip: import_media returned no asset_id")?.to_string()
        }
    };

    let track_id = call(ss, "add_track", &json!({}))?["track_id"].as_str().ok_or("add_pip: add_track returned no track_id")?.to_string();
    let _ = call(ss, "set_track", &json!({"track_id": track_id, "name": "pip"}));

    // Shared on-screen geometry for the border ring and the shadow silhouette (both are plain add_shape
    // clips, which draw at their literal pixel width/height -- unlike a media clip they are not canvas-fit,
    // so they're sized directly to match the inset's own on-screen footprint computed above).
    let circle_diam = box_w.min(box_h);
    let corner_px = ROUND_FRAC * box_w.min(box_h) / 2.0;
    let (shape_kind, shw, shh, shcr) = match shape.as_str() {
        "circle" => ("ellipse", circle_diam, circle_diam, 0.0),
        "rounded" => ("rect", box_w, box_h, corner_px),
        _ => ("rect", box_w, box_h, 0.0),
    };

    let mut shadow_clip_id: Option<String> = None;
    if shadow {
        let off = MARGIN_FRAC * cw.min(ch) * 0.6; // small down-right offset, a flat drop-shadow silhouette (no blur primitive on shapes)
        let cid = call(ss, "add_shape", &json!({
            "track_id": track_id, "shape": shape_kind, "start": at_s, "duration": duration_s,
            "width": shw, "height": shh, "fill": [0.0, 0.0, 0.0, 0.45], "corner_radius": shcr,
        }))?["clip_id"].as_str().ok_or("add_pip: add_shape (shadow) returned no clip_id")?.to_string();
        call(ss, "set_property", &json!({"clip_id": cid, "property": "x", "value": x_off + off}))?;
        call(ss, "set_property", &json!({"clip_id": cid, "property": "y", "value": y_off + off}))?;
        shadow_clip_id = Some(cid);
    }

    let pip_clip_id = call(ss, "add_clip", &json!({
        "track_id": track_id, "source_type": "media", "asset_id": asset_id,
        "start": at_s, "duration": duration_s, "in_point": source_in_s, "fit": "cover",
    }))?["clip_id"].as_str().ok_or("add_pip: add_clip returned no clip_id")?.to_string();

    if shape != "rect" {
        let (mask_kind, w_norm, h_norm, round) = if shape == "circle" {
            ("ellipse", cw.min(ch) / cw, cw.min(ch) / ch, 0.0)
        } else {
            ("rect", 1.0, 1.0, ROUND_FRAC)
        };
        call(ss, "add_mask", &json!({"clip_id": pip_clip_id, "kind": mask_kind, "params": {"cx": 0.5, "cy": 0.5, "w": w_norm, "h": h_norm, "round": round}}))?;
    }

    if mute_source { call(ss, "set_audio", &json!({"clip_id": pip_clip_id, "mute": true}))?; }

    // scale <- "pop" only; opacity <- "fade" only; x <- "slide" only. A property neither side
    // animates is just set_property'd to its steady value (cheaper, and matches existing tools' style
    // for the common "none" case).
    let pop_enter = (animate_in == "pop").then_some((0.0, "out_back"));
    let pop_exit = (animate_out == "pop").then_some((0.0, "in_back"));
    animate_prop(ss, &pip_clip_id, "scale", size, duration_s, pop_enter, pop_exit)?;

    let fade_enter = (animate_in == "fade").then_some((0.0, "linear"));
    let fade_exit = (animate_out == "fade").then_some((0.0, "linear"));
    animate_prop(ss, &pip_clip_id, "opacity", 1.0, duration_s, fade_enter, fade_exit)?;

    let dir = if x_off >= 0.0 { 1.0 } else { -1.0 };
    let slide_enter = (animate_in == "slide").then_some((x_off + dir * box_w, "out_quart"));
    let slide_exit = (animate_out == "slide").then_some((x_off + dir * box_w, "in_quart"));
    animate_prop(ss, &pip_clip_id, "x", x_off, duration_s, slide_enter, slide_exit)?;
    call(ss, "set_property", &json!({"clip_id": pip_clip_id, "property": "y", "value": y_off}))?;

    let mut border_clip_id: Option<String> = None;
    if border_px > 0.0 {
        let cid = call(ss, "add_shape", &json!({
            "track_id": track_id, "shape": shape_kind, "start": at_s, "duration": duration_s,
            "width": shw, "height": shh, "fill": [0.0, 0.0, 0.0, 0.0], "corner_radius": shcr,
            "stroke_width": border_px, "stroke_color": rgba_json(border_color),
        }))?["clip_id"].as_str().ok_or("add_pip: add_shape (border) returned no clip_id")?.to_string();
        call(ss, "set_property", &json!({"clip_id": cid, "property": "x", "value": x_off}))?;
        call(ss, "set_property", &json!({"clip_id": cid, "property": "y", "value": y_off}))?;
        border_clip_id = Some(cid);
    }

    Ok(json!({
        "track_id": track_id, "clip_id": pip_clip_id, "asset_id": asset_id,
        "shadow_clip_id": shadow_clip_id, "border_clip_id": border_clip_id,
        "box_width_px": box_w, "box_height_px": box_h, "x": x_off, "y": y_off,
    }))
}

pub fn commands() -> Vec<Command> {
    vec![Command {
        name: "add_pip",
        description: "Picture-in-picture: composite a second video as a small inset over the current one, like a corner webcam bubble or floating avatar. A TV-frame cutaway is the same shape with corners instead of a circle. One call instead of add_clip + set_property x3 + add_mask + add_shape by hand: a new dedicated track; the source clip (a media file path, imported fresh, or an existing clip id on the timeline, whose asset is reused without touching the original clip) scaled to `size` (fraction of frame width, default 0.3, canvas-aspect box via fit:cover so there is no letterbox inside the inset) and placed at `position` (top_right default, top_left, bottom_right, bottom_left, center, or {x,y} normalized 0..1); cropped to `shape` (rect, rounded default with a modest corner radius, or circle, inscribed so it is a true circle on any canvas aspect); an optional stroke `border_px`/`border_color` ring and, by default, a soft flat drop-shadow silhouette behind it (`shadow`, default true); `animate_in`/`animate_out` (pop default-in with a ~0.25s scale-overshoot per the measured creator rule, slide, fade, or none; default-out fade) on entry/exit; `source_in_s` trims into the source and `mute_source` (default true) silences the inset's own audio so it never competes with the main track. Never appears before at_s or after at_s+duration_s. Deterministic (same input, same geometry and keys) and undoable with undo as one step, like any other mutating tool.",
        mutates: true,
        schema: obj(vec![
            ("source", s("media file path, or an existing clip id on the timeline whose asset is reused")),
            ("at_s", n("timeline seconds the inset starts at, >= 0")),
            ("duration_s", n("seconds the inset is on screen, > 0")),
            ("position", json!({"description":"top_right (default), top_left, bottom_right, bottom_left, center, or {x,y} normalized 0..1 (the inset's own center as a fraction of the canvas)"})),
            ("size", n("fraction of frame width the inset's box width is, 0 (exclusive) to 1; default 0.3")),
            ("shape", json!({"type":"string","enum":["rect","rounded","circle"],"description":"crop shape; default rounded"})),
            ("border_px", n("stroke width in px around the inset; default 0 (no border)")),
            ("border_color", json!({"type":"array","items":{"type":"number","minimum":0,"maximum":1},"minItems":3,"maxItems":4,"description":"RGB or RGBA 0..1, default opaque white"})),
            ("shadow", json!({"type":"boolean","description":"a soft flat drop-shadow silhouette behind the inset; default true"})),
            ("animate_in", json!({"type":"string","enum":["pop","slide","fade","none"],"description":"entrance style; default pop (~0.25s scale-overshoot)"})),
            ("animate_out", json!({"type":"string","enum":["pop","slide","fade","none"],"description":"exit style; default fade"})),
            ("source_in_s", n("seconds into the source to start reading from; default 0")),
            ("mute_source", json!({"type":"boolean","description":"mute the inset clip's own audio; default true"})),
        ], &["source", "at_s", "duration_s"]),
        run,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 64x36 canvas plus an 8x8 PNG still in a test-unique directory.
    fn setup(tag: &str) -> (Session, std::path::PathBuf, String) {
        let mut ss = Session::new(); ss.project.width = 64; ss.project.height = 36;
        let dir = std::env::temp_dir().join(format!("kimo-add-pip-{}-{tag}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("still.png"); let p = png.to_str().unwrap().to_string();
        crate::render::write_image(&crate::effects::Frame { w: 8, h: 8, d: vec![0.5; 8 * 8 * 4] }, &p).unwrap();
        (ss, dir, p)
    }
    fn close(a: &Value, b: f64) { assert!((a.as_f64().unwrap() - b).abs() < 1e-9, "{a} vs {b}"); }
    #[test]
    fn corners_and_normalized_positions_place_the_box_inside_a_margin() {
        let (mut ss, dir, p) = setup("place");
        // size 0.5 of 64x36 = 32x18 box; margin 0.03*36 = 1.08
        let r = run(&mut ss, &json!({"source": p, "at_s": 0.0, "duration_s": 1.5, "size": 0.5, "position": "bottom_left", "shape": "circle", "border_px": 2.0, "border_color": [1.0, 0.0, 0.0], "shadow": false, "animate_in": "slide", "animate_out": "pop", "mute_source": false})).unwrap();
        close(&r["box_width_px"], 32.0); close(&r["box_height_px"], 18.0);
        close(&r["x"], -32.0 + 16.0 + 1.08); close(&r["y"], 18.0 - 9.0 - 1.08);
        assert!(r["shadow_clip_id"].is_null()); assert!(r["border_clip_id"].is_string());
        assert!(ss.project.find_clip(r["border_clip_id"].as_str().unwrap()).is_some());
        // reuse the clip just placed as the source: same asset, a new clip
        let again = run(&mut ss, &json!({"source": r["clip_id"], "at_s": 0.5, "duration_s": 1.0, "position": "top_left", "shape": "rect", "animate_in": "none", "animate_out": "slide"})).unwrap();
        assert_eq!(again["asset_id"], r["asset_id"]); assert_ne!(again["clip_id"], r["clip_id"]);
        // default size 0.3: 19.2x10.8 box
        close(&again["x"], -32.0 + 9.6 + 1.08); close(&again["y"], -18.0 + 5.4 + 1.08);
        assert!(again["shadow_clip_id"].is_string(), "shadow defaults on"); assert!(again["border_clip_id"].is_null());
        // {x,y}: the inset centre at that fraction of the canvas
        let norm = run(&mut ss, &json!({"source": r["clip_id"], "at_s": 0.0, "duration_s": 1.0, "position": {"x": 0.25, "y": 1.0}, "border_px": 1.0, "border_color": [0.0, 1.0, 0.0, 0.5]})).unwrap();
        close(&norm["x"], -16.0); close(&norm["y"], 18.0);
        for (pos, x, y) in [("top_right", 32.0 - 9.6 - 1.08, -18.0 + 5.4 + 1.08), ("bottom_right", 32.0 - 9.6 - 1.08, 18.0 - 5.4 - 1.08), ("center", 0.0, 0.0)] {
            let o = run(&mut ss, &json!({"source": r["clip_id"], "at_s": 0.0, "duration_s": 1.0, "position": pos})).unwrap();
            close(&o["x"], x); close(&o["y"], y);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn every_bad_argument_is_refused_before_anything_is_added() {
        let (mut ss, dir, p) = setup("refuse");
        let base = json!({"source": p, "at_s": 0.0, "duration_s": 1.0});
        let with = |k: &str, v: Value| { let mut a = base.clone(); a[k] = v; a };
        let tracks = ss.project.tracks.len();
        assert_eq!(run(&mut ss, &json!([1])).unwrap_err(), "add_pip: arguments must be an object");
        assert_eq!(run(&mut ss, &with("colour", json!(1))).unwrap_err(), "add_pip: unknown argument 'colour'");
        for (k, v, e) in [
            ("at_s", json!(-1.0), "add_pip: at_s must be a finite number >= 0"),
            ("duration_s", json!(0.0), "add_pip: duration_s must be a finite number > 0"),
            ("size", json!(1.5), "add_pip: size must be a finite number in (0, 1]"),
            ("shape", json!("star"), "add_pip: shape must be rect, rounded or circle, got 'star'"),
            ("border_px", json!(-1.0), "add_pip: border_px must be a finite number >= 0"),
            ("border_color", json!("red"), "add_pip: 'border_color' must be an RGB or RGBA array, channels 0..1"),
            ("border_color", json!([1.0, 1.0]), "add_pip: 'border_color' must have 3 or 4 channels"),
            ("border_color", json!([2.0, 0.0, 0.0]), "add_pip: 'border_color' channels must be finite numbers in 0..1"),
            ("animate_in", json!("spin"), "add_pip: animate_in must be pop, slide, fade or none, got 'spin'"),
            ("animate_out", json!("spin"), "add_pip: animate_out must be pop, slide, fade or none, got 'spin'"),
            ("source_in_s", json!(-0.5), "add_pip: source_in_s must be a finite number >= 0"),
            ("source", json!("  "), "add_pip: source must not be empty"),
            ("position", json!("middle"), "add_pip: unknown position 'middle'; use top_right, top_left, bottom_right, bottom_left, center, or {x,y} normalized"),
            ("position", json!({"x": "left", "y": 0.5}), "add_pip: position.x must be a finite number"),
            ("position", json!({"x": 0.5}), "add_pip: position.y must be a finite number"),
            ("position", json!({"x": 1.5, "y": 0.5}), "add_pip: position.x and position.y must be within 0..1"),
            ("position", json!(3), "add_pip: position must be a string (top_right|top_left|bottom_right|bottom_left|center) or {x,y} normalized 0..1"),
        ] { assert_eq!(run(&mut ss, &with(k, v)).unwrap_err(), e, "{k}"); }
        // wrong JSON types go through the shared argument error, naming the field
        for (k, v) in [("size", json!("big")), ("border_px", json!(true)), ("source_in_s", json!("0")), ("shape", json!(5)), ("animate_in", json!(1)), ("animate_out", json!(1)), ("shadow", json!("yes")), ("mute_source", json!(0))] {
            assert!(run(&mut ss, &with(k, v)).unwrap_err().contains(k), "{k}");
        }
        assert_eq!(ss.project.tracks.len(), tracks, "nothing was added");
        // an existing non-media clip is not a valid source
        let t = call(&mut ss, "add_track", &json!({})).unwrap()["track_id"].clone();
        let shape = call(&mut ss, "add_shape", &json!({"track_id": t, "shape": "rect", "start": 0.0, "duration": 1.0, "width": 4.0, "height": 4.0})).unwrap()["clip_id"].as_str().unwrap().to_string();
        assert_eq!(run(&mut ss, &with("source", json!(shape))).unwrap_err(), format!("add_pip: clip '{shape}' is not a media clip; source must be a media clip id or a file path"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
