//! The kimo-mcp stdio protocol end to end: JSON-RPC framing (ignored lines, notifications, unknown methods), the meta
//! tools (kimo_search, search_tools, kimo_describe, kimo_enable_group + list_changed), grouped calls, a failed `expect`
//! surfaced as a structured error, and the inline image blocks of render results (inline.rs: as-is PNG, ffmpeg scaling,
//! JPEG, inline:never, more than four images). Canvases are 64x32 or 256x128 solids; every call answers in milliseconds.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Mcp { ch: Child, stdin: Option<ChildStdin>, out: BufReader<ChildStdout>, id: u64 }
impl Mcp {
    fn spawn(envs: &[(&str, &str)]) -> Mcp {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_kimo-mcp"));
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).env("KIMO_SPECTATOR", "off").env("KIMO_AUTO_PROXY", "off")
            .env_remove("KIMO_TOOLS").env_remove("KIMO_INLINE_IMAGES").env_remove("KIMO_INLINE_FFMPEG").env_remove("KIMO_INLINE_CAP_BYTES");
        for (k, v) in envs { cmd.env(k, v); }
        let mut ch = cmd.spawn().expect("spawn kimo-mcp");
        let stdin = ch.stdin.take().unwrap(); let out = BufReader::new(ch.stdout.take().unwrap());
        Mcp { ch, stdin: Some(stdin), out, id: 0 }
    }
    fn raw(&mut self, line: &str) { let i = self.stdin.as_mut().unwrap(); writeln!(i, "{line}").unwrap(); i.flush().unwrap(); }
    fn send(&mut self, method: &str, params: Value) { self.id += 1; let l = json!({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}).to_string(); self.raw(&l); }
    fn recv(&mut self) -> Value { let mut l = String::new(); self.out.read_line(&mut l).unwrap(); serde_json::from_str(&l).unwrap_or_else(|_| panic!("not JSON: {l}")) }
    fn rpc(&mut self, method: &str, params: Value) -> Value { self.send(method, params); self.recv() }
    fn call(&mut self, name: &str, args: Value) -> Value { self.rpc("tools/call", json!({"name": name, "arguments": args}))["result"].clone() }
    fn ok(&mut self, name: &str, args: Value) -> Value { let r = self.call(name, args.clone()); assert_eq!(r["isError"], false, "{name} {args}: {r}"); r }
    fn text(r: &Value) -> Value { serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap() }
}
/// Closing stdin ends the server's read loop, so it exits normally (and writes its coverage profile).
impl Drop for Mcp { fn drop(&mut self) { drop(self.stdin.take()); let _ = self.ch.wait(); } }

fn tmp(name: &str) -> String { std::env::temp_dir().join(format!("kimo-proto-{}-{name}", std::process::id())).to_str().unwrap().to_string() }
fn canvas(m: &mut Mcp, w: u32, h: u32) {
    m.ok("project_set", json!({"width": w, "height": h, "fps": 10}));
    let t = Mcp::text(&m.ok("add_track", json!({})))["track_id"].as_str().unwrap().to_string();
    m.ok("add_clip", json!({"track_id": t, "source_type": "solid", "color": [1, 0, 0], "start": 0, "duration": 1, "observe": "off"}));
}
fn notes(r: &Value) -> Vec<String> { r["content"].as_array().unwrap().iter().skip(1).filter(|b| b["type"] == "text").map(|b| b["text"].as_str().unwrap().to_string()).collect() }
fn images(r: &Value) -> Vec<Value> { r["content"].as_array().unwrap().iter().filter(|b| b["type"] == "image").cloned().collect() }

#[test] fn framing_pings_notifications_and_unknown_methods() {
    let mut m = Mcp::spawn(&[]);
    m.raw(""); m.raw("not json at all");
    m.raw(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string());
    let r = m.rpc("ping", json!({})); assert_eq!(r["result"], json!({}), "the junk lines and the notification got no reply: {r}");
    let r = m.rpc("resources/list", json!({})); assert_eq!(r["error"], json!({"code": -32601, "message": "method not found: resources/list"}));
    let r = m.rpc("initialize", json!({})); assert_eq!(r["result"]["capabilities"]["tools"]["listChanged"], true);
}

#[test] fn meta_tools_groups_and_list_changed() {
    let mut m = Mcp::spawn(&[("KIMO_TOOLS", "core")]);
    let core = m.rpc("tools/list", json!({}))["result"]["tools"].as_array().unwrap().len();
    let flat = m.rpc("tools/list", json!({"mode": "flat"}))["result"]["tools"].as_array().unwrap().len(); assert!(flat > core, "KIMO_TOOLS=core is the default, mode overrides it");
    let r = Mcp::text(&m.ok("kimo_search", json!({"query": "blur", "limit": 3})));
    assert_eq!(r["matches"].as_array().unwrap().len(), 3); assert!(!r["effects_and_transitions"].as_array().unwrap().is_empty());
    let r = Mcp::text(&m.ok("search_tools", json!({"query": "mask", "group": "masks", "limit": 2}))); assert!(r["matches"].as_array().unwrap().iter().all(|h| h["group"] == "masks"));
    assert_eq!(Mcp::text(&m.ok("kimo_describe", json!({"action": "add_track"})))["group"], "timeline");
    let r = m.rpc("tools/call", json!({"name": "kimo_enable_group", "arguments": {"name": "masks"}}));
    assert_eq!(Mcp::text(&r["result"])["enabled"], "masks");
    assert_eq!(m.recv(), json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}));
    let after = m.rpc("tools/list", json!({}))["result"]["tools"].as_array().unwrap().len(); assert!(after > core, "masks tools are listed individually now");
    // grouped calls
    let r = m.ok("timeline", json!({"action": "add_track"})); assert!(Mcp::text(&r)["track_id"].is_string());
    let r = m.call("timeline", json!({"action": "project_set"})); assert_eq!(r["isError"], true); assert_eq!(r["content"][0]["text"], "action 'project_set' belongs to group 'project', not 'timeline'");
}

#[test] fn a_failed_expectation_is_a_structured_error() {
    let mut m = Mcp::spawn(&[]);
    m.ok("project_set", json!({"width": 64, "height": 32, "fps": 10}));
    let t = Mcp::text(&m.ok("add_track", json!({})))["track_id"].as_str().unwrap().to_string();
    let r = m.call("add_clip", json!({"track_id": t, "source_type": "solid", "color": [0, 0, 0], "start": 0, "duration": 1, "observe": "numbers", "expect": {"luma": [0.9, 1.0], "at": 0.5}}));
    assert_eq!(r["isError"], true); assert_eq!(r["structuredContent"]["code"], "expect_failed");
    assert!(r["content"][0]["text"].as_str().unwrap().starts_with("expectation failed, so the edit was rolled back (undo): "));
    assert!(Mcp::text(&m.ok("get_project", json!({})))["tracks"][0]["clips"].as_array().unwrap().is_empty(), "rolled back");
}

#[test] fn render_results_carry_inline_images() {
    let mut m = Mcp::spawn(&[]); canvas(&mut m, 64, 32);
    let p = tmp("a.png");
    let r = m.ok("render_frame", json!({"time": 0.5, "path": p}));
    let im = images(&r); assert_eq!(im.len(), 1); assert_eq!(im[0]["mimeType"], "image/png");
    assert_eq!(im[0]["data"].as_str().unwrap().len(), std::fs::metadata(&p).unwrap().len().div_ceil(3) as usize * 4, "the PNG file itself, base64");
    let sc = &r["structuredContent"]; assert_eq!((sc["width"].as_u64(), sc["height"].as_u64(), sc["time"].as_f64()), (Some(64), Some(32), Some(0.5)));
    assert_eq!(sc["inline"], json!({"scale": 1.0, "bytes": std::fs::metadata(&p).unwrap().len(), "format": "png", "width": 64, "height": 32}));
    let r = m.ok("render_frame", json!({"time": 0.5, "path": p, "inline": "never"})); assert!(images(&r).is_empty()); assert_eq!(r["structuredContent"]["inline"], json!({"omitted": "inline:never"}));
    let r = m.ok("render_frame", json!({"time": 0.5, "path": p, "inline_format": "jpeg", "inline_quality": 50}));
    assert_eq!(images(&r)[0]["mimeType"], "image/jpeg"); assert_eq!(r["structuredContent"]["inline"]["format"], "jpeg");
    // a wide canvas is scaled down to max_inline_px on its long edge
    let mut m = Mcp::spawn(&[]); canvas(&mut m, 256, 128);
    let r = m.ok("render_frame", json!({"time": 0.5, "path": p, "max_inline_px": 128}));
    assert_eq!(r["structuredContent"]["inline"]["width"], 128); assert_eq!(r["structuredContent"]["inline"]["height"], 64); assert_eq!(r["structuredContent"]["inline"]["scale"], 0.5);
    // five renders in one batch: four inline, the fifth noted, all listed
    let ops: Vec<Value> = (0..5).map(|i| json!({"tool": "render_frame", "args": {"time": 0.5, "path": tmp(&format!("b{i}.png"))}})).collect();
    let r = m.ok("batch", json!({"ops": ops}));
    assert_eq!(images(&r).len(), 4); assert_eq!(r["structuredContent"]["images"].as_array().unwrap().len(), 5);
    assert!(notes(&r).iter().any(|n| n.contains("inline omitted: more than 4 images; use path")), "{:?}", notes(&r));
    for i in 0..5 { let _ = std::fs::remove_file(tmp(&format!("b{i}.png"))); }
    let _ = std::fs::remove_file(&p);
}
