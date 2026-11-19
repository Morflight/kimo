//! The engine sidecar: the GUI launches and supervises one `kimo-mcp` child over stdio JSON-RPC (MCP), and can probe
//! a fresh one (initialize + tools/list) to prove what an external client would get.
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn engine_file_name() -> &'static str { engine_file_name_for(cfg!(windows)) }
fn engine_file_name_for(windows: bool) -> &'static str { if windows { "kimo-mcp.exe" } else { "kimo-mcp" } }

/// Where the engine binary is, first hit wins: $KIMO_MCP_BIN, next to the GUI binary (Tauri sidecar layout), `bin/`
/// beside it, `../lib/kimo/` (deb layout), the Tauri resource dir, then PATH.
pub fn find_engine(exe_dir: Option<&Path>, resource_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KIMO_MCP_BIN").map(PathBuf::from) { if p.is_file() { return Some(p); } }
    let name = engine_file_name();
    let mut cands = Vec::new();
    if let Some(d) = exe_dir { cands.extend([d.join(name), d.join("bin").join(name), d.join("..").join("lib").join("kimo").join(name)]); }
    if let Some(d) = resource_dir { cands.extend([d.join(name), d.join("bin").join(name)]); }
    // a source checkout: crates/kimo-gui/target/<profile>/kimo-gui next to the workspace's target/<profile>/kimo-mcp
    if let Some(d) = exe_dir {
        for up in d.ancestors().take(6) {
            for prof in ["release", "dist", "debug"] { cands.push(up.join("target").join(prof).join(name)); }
        }
    }
    cands.into_iter().find(|p| p.is_file()).or_else(|| crate::agents::which("kimo-mcp"))
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ProbeInfo {
    pub ok: bool,
    pub version: String,
    pub protocol: String,
    /// Tools a client sees on tools/list (kimo lists tool groups; each has many actions).
    pub tools: usize,
    pub actions: usize,
    pub names: Vec<String>,
    pub ms: u128,
    pub error: Option<String>,
}

/// Sum of the "[N actions]" counts in group descriptions (1 for a plain tool).
pub fn count_actions(tools: &[Value]) -> usize {
    tools.iter().map(|t| {
        let d = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
        d.rfind('[').and_then(|i| d[i + 1..].split_whitespace().next()).and_then(|n| n.parse::<usize>().ok())
            .filter(|_| d.contains(" actions]")).unwrap_or(1)
    }).sum()
}

type Pending = Arc<Mutex<HashMap<u64, Sender<Result<Value, String>>>>>;

/// One stdio JSON-RPC connection to a child process.
pub struct Rpc {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Pending,
    next: AtomicU64,
    alive: Arc<AtomicBool>,
    pub pid: u32,
}

impl Rpc {
    pub fn spawn(cmd: &Path, args: &[String], env: &[(String, String)], stderr_log: Option<&Path>) -> Result<Rpc, String> {
        let mut c = Command::new(cmd);
        c.args(args).envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str()))).stdin(Stdio::piped()).stdout(Stdio::piped());
        // stderr goes to a file, never to a captured pipe nobody drains
        match stderr_log.and_then(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).ok()) {
            Some(f) => { c.stderr(Stdio::from(f)); }
            None => { c.stderr(Stdio::null()); }
        }
        #[cfg(windows)]
        { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); } // CREATE_NO_WINDOW
        let child = c.spawn().map_err(|e| format!("could not start {}: {e}", cmd.display()))?;
        Self::from_child(child)
    }

    // Own cleanup even if a caller supplies a child without the required pipes.
    fn from_child(mut child: Child) -> Result<Rpc, String> {
        let pipes: Result<_, &str> = (|| Ok((child.stdin.take().ok_or("no stdin")?, child.stdout.take().ok_or("no stdout")?)))();
        let (stdin, stdout) = match pipes {
            Ok(pipes) => pipes,
            Err(error) => { let _ = child.kill(); let _ = child.wait(); return Err(error.into()); }
        };
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));
        let (p2, a2) = (pending.clone(), alive.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                let Some(id) = v.get("id").and_then(|i| i.as_u64()) else { continue }; // notifications: ignored
                if let Some(tx) = p2.lock().unwrap().remove(&id) {
                    let r = match v.get("error") { Some(e) => Err(e.get("message").and_then(|m| m.as_str()).unwrap_or("engine error").to_string()), None => Ok(v.get("result").cloned().unwrap_or(Value::Null)) };
                    let _ = tx.send(r);
                }
            }
            a2.store(false, Ordering::SeqCst);
            for (_, tx) in p2.lock().unwrap().drain() { let _ = tx.send(Err("the engine exited".into())); }
        });
        let pid = child.id();
        Ok(Rpc { child: Mutex::new(child), stdin: Mutex::new(stdin), pending, next: AtomicU64::new(1), alive, pid })
    }

    pub fn alive(&self) -> bool { self.alive.load(Ordering::SeqCst) }

    pub fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        if !self.alive() { return Err("the engine is not running".into()); }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel();
        self.pending.lock().unwrap().insert(id, tx);
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
        {
            let mut w = self.stdin.lock().unwrap();
            if w.write_all(line.as_bytes()).and_then(|_| w.write_all(b"\n")).and_then(|_| w.flush()).is_err() {
                self.pending.lock().unwrap().remove(&id);
                return Err("the engine closed its input".into());
            }
        }
        match rx.recv_timeout(timeout) {
            Ok(r) => r,
            Err(_) => { self.pending.lock().unwrap().remove(&id); Err(format!("no answer from the engine in {} s", timeout.as_secs())) }
        }
    }

    pub fn notify(&self, method: &str) {
        let line = json!({"jsonrpc": "2.0", "method": method}).to_string();
        let mut w = self.stdin.lock().unwrap();
        let _ = w.write_all(line.as_bytes()).and_then(|_| w.write_all(b"\n")).and_then(|_| w.flush());
    }

    pub fn kill(&self) {
        let mut c = self.child.lock().unwrap();
        let _ = c.kill();
        let _ = c.wait();
        self.alive.store(false, Ordering::SeqCst);
    }

    /// initialize + notifications/initialized + tools/list, the exact handshake an MCP client does.
    pub fn handshake(&self) -> Result<(Value, Vec<Value>), String> {
        let init = self.request("initialize", json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "kimo-gui", "version": env!("CARGO_PKG_VERSION")}}), Duration::from_secs(20))?;
        self.notify("notifications/initialized");
        let list = self.request("tools/list", json!({}), Duration::from_secs(20))?;
        let tools = list.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
        Ok((init, tools))
    }
}

impl Drop for Rpc { fn drop(&mut self) { self.kill(); } }

fn info_from(init: &Value, tools: &[Value], ms: u128) -> ProbeInfo {
    ProbeInfo {
        ok: true,
        version: init.pointer("/serverInfo/version").and_then(|v| v.as_str()).unwrap_or("?").to_string(),
        protocol: init.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("?").to_string(),
        tools: tools.len(),
        actions: count_actions(tools),
        names: tools.iter().filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from)).collect(),
        ms, error: None,
    }
}

/// Start a FRESH engine with exactly the launch an external client would use, handshake, and stop it.
pub fn probe(cmd: &Path, args: &[String], env: &[(String, String)]) -> ProbeInfo {
    let t0 = std::time::Instant::now();
    let mut env = env.to_vec();
    env.push(("KIMO_SPECTATOR".into(), "off".into())); // the probe itself is not agent activity
    match Rpc::spawn(cmd, args, &env, None).and_then(|r| { let h = r.handshake(); r.kill(); h }) {
        Ok((init, tools)) => info_from(&init, &tools, t0.elapsed().as_millis()),
        Err(e) => ProbeInfo { error: Some(e), ms: t0.elapsed().as_millis(), ..Default::default() },
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Status {
    Missing { searched: String },
    Stopped,
    Running { pid: u32, version: String, tools: usize, actions: usize, bin: String },
    Crashed { error: String, restarts: u32 },
}

/// The supervised engine the GUI's own agent and buttons use.
pub struct Supervisor {
    pub bin: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub log: Option<PathBuf>,
    rpc: Mutex<Option<Arc<Rpc>>>,
    tools: Mutex<Vec<Value>>,
    status: Mutex<Status>,
    restarts: AtomicU64,
}

impl Supervisor {
    pub fn new(bin: Option<PathBuf>, env: Vec<(String, String)>, log: Option<PathBuf>) -> Supervisor {
        let status = if bin.is_some() { Status::Stopped } else { Status::Missing { searched: format!("$KIMO_MCP_BIN, next to the app, bin/, PATH ({})", engine_file_name()) } };
        Supervisor { bin, env, log, rpc: Mutex::new(None), tools: Mutex::new(vec![]), status: Mutex::new(status), restarts: AtomicU64::new(0) }
    }
    pub fn status(&self) -> Status {
        if let Some(r) = self.rpc.lock().unwrap().as_ref() {
            if !r.alive() { *self.status.lock().unwrap() = Status::Crashed { error: "the engine exited".into(), restarts: self.restarts.load(Ordering::SeqCst) as u32 }; }
        }
        self.status.lock().unwrap().clone()
    }
    pub fn start(&self) -> Status {
        let Some(bin) = self.bin.clone() else { return self.status() };
        self.stop();
        match Rpc::spawn(&bin, &[], &self.env, self.log.as_deref()).and_then(|r| { let h = r.handshake()?; Ok((r, h)) }) {
            Ok((r, (init, tools))) => {
                let i = info_from(&init, &tools, 0);
                *self.status.lock().unwrap() = Status::Running { pid: r.pid, version: i.version, tools: i.tools, actions: i.actions, bin: bin.display().to_string() };
                *self.tools.lock().unwrap() = tools;
                *self.rpc.lock().unwrap() = Some(Arc::new(r));
            }
            Err(e) => *self.status.lock().unwrap() = Status::Crashed { error: e, restarts: self.restarts.load(Ordering::SeqCst) as u32 },
        }
        self.status()
    }
    pub fn stop(&self) {
        if let Some(r) = self.rpc.lock().unwrap().take() { r.kill(); }
        if self.bin.is_some() { *self.status.lock().unwrap() = Status::Stopped; }
    }
    /// The live connection, restarting a crashed engine once per call (the project in it is gone; the GUI says so).
    fn conn(&self) -> Result<Arc<Rpc>, String> {
        if let Some(r) = self.rpc.lock().unwrap().as_ref() { if r.alive() { return Ok(r.clone()); } }
        self.restarts.fetch_add(1, Ordering::SeqCst);
        match self.start() { Status::Running { .. } => self.running_connection(), s => Err(format!("engine not running: {}", serde_json::to_string(&s).unwrap_or_default())) }
    }
    // The connection can disappear if stop races with a successful start.
    fn running_connection(&self) -> Result<Arc<Rpc>, String> {
        self.rpc.lock().unwrap().clone().ok_or_else(|| "engine not running".into())
    }
    pub fn tools(&self) -> Vec<Value> { self.tools.lock().unwrap().clone() }
    /// tools/call; returns the MCP result object ({content, isError, structuredContent?}).
    pub fn call(&self, name: &str, args: &Value) -> Result<Value, String> {
        self.conn()?.request("tools/call", json!({"name": name, "arguments": args}), Duration::from_secs(600))
    }
}

impl Drop for Supervisor { fn drop(&mut self) { self.stop(); } }

/// The text of an MCP tool result (images become "[image]").
pub fn result_text(r: &Value) -> String {
    r.get("content").and_then(|c| c.as_array()).map(|a| a.iter().map(|b| match b.get("type").and_then(|t| t.as_str()) {
        Some("text") => b.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        Some(t) => format!("[{t}]"),
        None => String::new(),
    }).collect::<Vec<_>>().join("\n")).unwrap_or_default()
}
pub fn is_error(r: &Value) -> bool { r.get("isError").and_then(|v| v.as_bool()).unwrap_or(false) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actions_from_group_descriptions() {
        let t = vec![json!({"name":"project","description":"Project settings ... [71 actions]"}), json!({"name":"search_tools","description":"Search"}), json!({"name":"x","description":"weird [abc]"})];
        assert_eq!(count_actions(&t), 73);
    }
    #[test]
    fn result_text_joins_blocks() {
        let r = json!({"content":[{"type":"text","text":"a"},{"type":"image","data":"..."}],"isError":false});
        assert_eq!(result_text(&r), "a\n[image]");
        assert!(!is_error(&r));
    }
    #[cfg(unix)]
    fn fake_server() -> PathBuf {
        // a stand-in MCP server: answers initialize, tools/list, tools/call; ignores notifications
        let d = std::env::temp_dir().join(format!("kimo-fake-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("fake-mcp.py");
        std::fs::write(&p, r#"#!/usr/bin/env python3
import sys, json, os
for line in sys.stdin:
    m = json.loads(line)
    if "id" not in m: continue
    meth = m["method"]
    if meth == "initialize" and os.environ.get("KIMO_TEST_INIT_ERROR"):
        print(json.dumps({"id":m["id"],"error":{"message":"fixture initialization rejected"}}),flush=True)
        continue
    if meth == "initialize": r = {"protocolVersion":"2024-11-05","serverInfo":{"name":"kimo","version":"9.9.9"}}
    elif meth == "tools/list": r = {"tools":[{"name":"project","description":"P [3 actions]"},{"name":"search_tools","description":"s"}]}
    elif meth == "tools/call":
        if m["params"]["name"] == "die": sys.exit(3)
        r = {"content":[{"type":"text","text":"called " + m["params"]["name"]}],"isError":False}
    else: r = None
    sys.stdout.write(json.dumps({"jsonrpc":"2.0","id":m["id"],"result":r}) + "\n"); sys.stdout.flush()
"#).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }
    #[cfg(unix)]
    #[test]
    fn probe_fake_server() {
        let p = probe(&fake_server(), &[], &[]);
        assert!(p.ok, "{:?}", p.error);
        assert_eq!((p.version.as_str(), p.tools, p.actions), ("9.9.9", 2, 4));
    }
    #[test]
    fn probe_missing_binary_is_an_error_not_a_panic() {
        let p = probe(Path::new("/nonexistent/kimo-mcp"), &[], &[]);
        assert!(!p.ok && p.error.unwrap().contains("could not start"));
    }
    #[cfg(unix)]
    #[test]
    fn supervisor_calls_and_restarts_after_crash() {
        let s = Supervisor::new(Some(fake_server()), vec![], None);
        assert!(matches!(s.start(), Status::Running { tools: 2, .. }));
        let r = s.call("project", &json!({"action":"undo"})).unwrap();
        assert_eq!(result_text(&r), "called project");
        assert!(s.call("die", &json!({})).is_err());
        std::thread::sleep(Duration::from_millis(100));
        assert!(matches!(s.status(), Status::Crashed { .. }));
        let r = s.call("timeline", &json!({})).unwrap(); // restarted on demand
        assert_eq!(result_text(&r), "called timeline");
        s.stop();
    }
    /// Required real-engine handshake: what an external client sees on tools/list.
    #[test]
    fn probe_real_engine() {
        let bin = std::env::var_os("KIMO_MCP_BIN").map(PathBuf::from).expect("build kimo-mcp and set KIMO_MCP_BIN before running desktop tests");
        let p = probe(&bin, &[], &[]);
        assert!(p.ok, "{:?}", p.error);
        assert!(p.tools >= 16, "tools/list returned {}", p.tools);
        assert!(p.names.iter().any(|n| n == "project"));
        assert!(p.actions > 100);
        eprintln!("real engine {} answered: {} tools, {} actions, {} ms", p.version, p.tools, p.actions, p.ms);
    }
    #[test]
    fn malformed_result_blocks_and_probe_defaults_are_explicit() {
        assert_eq!(engine_file_name_for(true), "kimo-mcp.exe");
        assert_eq!(engine_file_name_for(false), "kimo-mcp");
        assert_eq!(count_actions(&[json!({}), json!({"description":"["}), json!({"description":"[2 widgets]"})]), 3);
        assert_eq!(result_text(&json!({"content":[{}, {"type":"text"}, {"type":"audio"}]})), "\n\n[audio]");
        assert_eq!(result_text(&Value::Null), "");
        assert!(!is_error(&json!({"isError":"true"})));
        assert!(is_error(&json!({"isError":true})));
        let info = info_from(&json!({}), &[json!({}), json!({"name":"valid"})], 19);
        assert_eq!(info.version, "?");
        assert_eq!(info.protocol, "?");
        assert_eq!(info.names, vec!["valid"]);
        assert_eq!((info.tools, info.actions, info.ms), (2, 2, 19));
        assert_eq!(serde_json::to_value(info).unwrap()["ok"], true);
        let missing = Supervisor::new(None, vec![], None);
        assert!(matches!(missing.start(), Status::Missing { .. }));
        assert!(missing.call("project", &json!({})).unwrap_err().contains("engine not running"));
        assert!(missing.tools().is_empty());
        let broken = Supervisor::new(Some(PathBuf::from("/nonexistent/kimo-engine-test")), vec![], None);
        assert!(matches!(broken.start(), Status::Crashed { .. }));
        broken.stop();
        assert!(matches!(broken.status(), Status::Stopped));
    }
    #[test]
    fn engine_discovery_precedence_in_an_isolated_process() {
        const FIXTURE: &str = "KIMO_ENGINE_DISCOVERY_TEST";
        if let Some(root) = std::env::var_os(FIXTURE).map(PathBuf::from) {
            let app = root.join("app");
            let resource = root.join("resource");
            let name = engine_file_name();
            let candidates = vec![root.join("override"), app.join(name), app.join("bin").join(name),
                app.join("../lib/kimo").join(name), resource.join(name), resource.join("bin").join(name),
                app.join("target/release").join(name), app.join("target/dist").join(name), app.join("target/debug").join(name)];
            for path in &candidates {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, "fixture").unwrap();
            }
            for expected in &candidates {
                assert_eq!(find_engine(Some(&app), Some(&resource)).as_ref(), Some(expected));
                std::fs::remove_file(expected).unwrap();
            }
            // This branch runs only in the isolated child test process.
            std::env::remove_var("KIMO_MCP_BIN");
            let path_bin = root.join("path").join(name);
            std::fs::create_dir_all(path_bin.parent().unwrap()).unwrap();
            std::fs::write(&path_bin, "fixture").unwrap();
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            assert_eq!(find_engine(None, None), Some(path_bin.clone()));
            std::fs::remove_file(path_bin).unwrap();
            assert_eq!(find_engine(None, None), None);
            return;
        }
        let root = std::env::temp_dir().join(format!("kimo-engine-discovery-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "engine::tests::engine_discovery_precedence_in_an_isolated_process", "--nocapture"])
            .env(FIXTURE, &root).env("KIMO_MCP_BIN", root.join("override"))
            .env("PATH", root.join("path")).env("PATHEXT", ".EXE").output().unwrap();
        assert!(output.status.success(), "{} {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rpc_faults_clear_pending_requests_and_preserve_engine_errors() {
        let dir=std::env::temp_dir().join(format!("kimo-rpc-faults-{}",std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script=dir.join("server.py");
        std::fs::write(&script,r#"import json, os, sys, time
mode=sys.argv[1]
print('fixture stderr',file=sys.stderr,flush=True)
if mode == 'closed':
    os.close(0)
    open(sys.argv[2],'w').close()
    time.sleep(30)
for line in sys.stdin:
    m=json.loads(line)
    if 'id' not in m: continue
    method=m['method']
    if method == 'never': continue
    if method == 'bad-bytes':
        sys.stdout.buffer.write(b'\xff\n'); sys.stdout.buffer.flush(); continue
    response={'id':m['id']}
    if method == 'error':
        print('not json',flush=True)
        print(json.dumps({'method':'notification'}),flush=True)
        print(json.dumps({'id':999999,'result':'unrelated'}),flush=True)
        response['error']={'message':'specific engine failure'}
    elif method == 'empty-error': response['error']={}
    elif method == 'initialize' and mode == 'reject-init': response['error']={'message':'initialization rejected'}
    elif method == 'tools/list' and mode == 'reject-list': response['error']={'message':'tool listing rejected'}
    elif method == 'tools/list': response['result']={'tools':None}
    print(json.dumps(response),flush=True)
"#).unwrap();
        let log=dir.join("stderr.log");
        let args=vec![script.display().to_string(),"normal".into()];
        let rpc=Rpc::spawn(Path::new("python3"),&args,&[("KIMO_RPC_FIXTURE".into(),"1".into())],Some(&log)).unwrap();
        let timeout=Duration::from_secs(2);
        assert_eq!(rpc.request("error",json!({}),timeout).unwrap_err(),"specific engine failure");
        assert_eq!(rpc.request("empty-error",json!({}),timeout).unwrap_err(),"engine error");
        assert_eq!(rpc.request("no-result",json!({}),timeout).unwrap(),Value::Null);
        assert!(rpc.handshake().unwrap().1.is_empty());
        assert!(rpc.request("never",json!({}),Duration::from_millis(20)).unwrap_err().contains("no answer"));
        assert!(rpc.pending.lock().unwrap().is_empty());
        assert_eq!(rpc.request("bad-bytes",json!({}),timeout).unwrap_err(),"the engine exited");
        assert_eq!(rpc.request("after-exit",json!({}),timeout).unwrap_err(),"the engine is not running");
        rpc.kill();
        assert!(std::fs::read_to_string(&log).unwrap().contains("fixture stderr"));
        for (mode,message) in [("reject-init","initialization rejected"),("reject-list","tool listing rejected")] {
            let args=vec![script.display().to_string(),mode.into()];
            let rpc=Rpc::spawn(Path::new("python3"),&args,&[],Some(&dir)).unwrap();
            assert_eq!(rpc.handshake().unwrap_err(),message);
        }
        let ready=dir.join("closed-ready");
        let args=vec![script.display().to_string(),"closed".into(),ready.display().to_string()];
        let rpc=Rpc::spawn(Path::new("python3"),&args,&[],None).unwrap();
        let deadline=std::time::Instant::now()+Duration::from_secs(2);
        while !ready.exists() && std::time::Instant::now()<deadline { std::thread::sleep(Duration::from_millis(5)); }
        assert!(ready.exists(),"fixture must close stdin before transport assertion");
        assert_eq!(rpc.request("closed",json!({}),timeout).unwrap_err(),"the engine closed its input");
        assert!(rpc.pending.lock().unwrap().is_empty());
        rpc.notify("closed-notification");
        rpc.kill();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_child_pipes_and_stopped_connection_are_reported() {
        for (piped_input,expected) in [(false,"no stdin"),(true,"no stdout")] {
            let child=Command::new("python3").args(["-c","import time; time.sleep(30)"])
                .stdin(if piped_input { Stdio::piped() } else { Stdio::null() })
                .stdout(Stdio::null()).spawn().unwrap();
            let started=std::time::Instant::now();
            let error=Rpc::from_child(child).err().expect("missing pipes must be rejected");
            assert_eq!(error,expected);
            assert!(started.elapsed()<Duration::from_secs(2),"failed connection must reap its child promptly");
        }
        let supervisor=Supervisor::new(None,vec![],None);
        let error=supervisor.running_connection().err().expect("missing connection must be rejected");
        assert_eq!(error,"engine not running");
    }

    #[cfg(unix)]
    #[test]
    fn supervisor_preserves_handshake_failure() {
        let s=Supervisor::new(Some(fake_server()),vec![("KIMO_TEST_INIT_ERROR".into(),"1".into())],None);
        let state=serde_json::to_value(s.start()).unwrap();
        assert_eq!(state["state"],"crashed");
        assert_eq!(state["error"],"fixture initialization rejected");
        assert!(s.rpc.lock().unwrap().is_none());
        assert!(s.tools().is_empty());
    }

    #[test]
    fn real_engine_crash_is_reported_and_next_call_restarts_it() {
        let bin = std::env::var_os("KIMO_MCP_BIN").map(PathBuf::from).expect("real engine required");
        let supervisor = Supervisor::new(Some(bin), vec![("KIMO_SPECTATOR".into(), "off".into()), ("KIMO_OBSERVE".into(), "off".into())], None);
        assert!(matches!(supervisor.start(), Status::Running { .. }));
        let old = supervisor.rpc.lock().unwrap().as_ref().unwrap().clone();
        old.kill();
        assert!(!old.alive());
        let crashed = serde_json::to_value(supervisor.status()).unwrap();
        assert_eq!(crashed["state"], "crashed");
        assert_eq!(crashed["error"], "the engine exited");
        assert_eq!(crashed["restarts"], 0);
        let result = supervisor.call("project", &json!({"action":"kimo_info"})).unwrap();
        assert!(!is_error(&result), "{result}");
        assert!(matches!(supervisor.status(), Status::Running { .. }));
        assert_eq!(supervisor.restarts.load(Ordering::SeqCst), 1);
        assert!(!Arc::ptr_eq(&old, supervisor.rpc.lock().unwrap().as_ref().unwrap()));
        supervisor.stop();
        assert!(matches!(supervisor.status(), Status::Stopped));
    }

    #[test]
    fn real_engine_startup_failure_is_a_crashed_supervisor() {
        let bin=std::env::var_os("KIMO_MCP_BIN").map(PathBuf::from).expect("real engine required");
        let log=std::env::temp_dir().join(format!("kimo-engine-startup-failure-{}.log",std::process::id()));
        let supervisor=Supervisor::new(Some(bin),vec![("KIMO_MEMORY_BUDGET_BYTES".into(),"invalid-budget".into()),("KIMO_SPECTATOR".into(),"off".into())],Some(log.clone()));
        let status=serde_json::to_value(supervisor.start()).unwrap();
        assert_eq!(status["state"],"crashed");
        let error=status["error"].as_str().unwrap();
        assert!(["the engine exited","the engine is not running","the engine closed its input"].contains(&error),"{error}");
        assert!(supervisor.rpc.lock().unwrap().is_none());
        assert!(std::fs::read_to_string(&log).unwrap().contains("KIMO_MEMORY_BUDGET_BYTES must be integer"));
        std::fs::remove_file(log).unwrap();
    }

}
