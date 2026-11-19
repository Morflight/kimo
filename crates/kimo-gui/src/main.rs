//! kimo: the desktop app people download. One window: a video editor (media bin, viewer, timeline, inspector, export)
//! on top of the kimo engine (kimo-mcp, launched and supervised as a sidecar over stdio), an Agent tab that runs Claude
//! Code, Codex or Antigravity on the user's OWN subscription (or an API key), and a Live view of every edit any agent
//! makes. While the app is open, `kimo-gui mcp` (what those agents launch) joins this app's engine through the
//! loopback bridge, so there is one project, one history and one Undo for everyone.
//! Loopback only; the webview can call only the commands listed in build.rs, granted to the main window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use kimo_gui::agent::{self, Chat};
use kimo_gui::bridge;
use kimo_gui::cli_agents::{self, Ev, Kind, KimoLaunch, Slot};
use kimo_gui::crash;
use kimo_gui::download::{self, Part};
use kimo_gui::engine::{self, Status, Supervisor};
use kimo_gui::keys::{self, KeyRing, Secret};
use kimo_gui::ledger::{self, As, Ledger};
use kimo_gui::mcp_config::{self, Launch};
use kimo_gui::recovery;
use kimo_gui::settings::Settings;
use kimo_gui::telemetry::{self, External, Source};
use kimo_gui::viewer::{self, Viewer};
use kimo_gui::{agents, engine_env, find_fonts, part_satisfied, PARTS_JSON};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

struct AppState {
    sup: Arc<Supervisor>,
    ledger: Arc<Ledger>,
    keys: KeyRing,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    chat: Mutex<Chat>,
    external: Mutex<Option<External>>,
    data_dir: PathBuf,
    fonts: Option<PathBuf>,
    parts: Vec<Part>,
    cancel_dl: Arc<AtomicBool>,
    viewer: Arc<Viewer>,
    bridge: Mutex<Option<bridge::Server>>,
    /// Subscription agents: the running turn (for Stop), whether one is running, and each agent's session id.
    turn: Slot,
    busy: AtomicBool,
    sessions: Mutex<HashMap<String, String>>,
    /// Agents currently attached through the bridge (label → open connections).
    attached: Mutex<HashMap<String, u32>>,
    project_path: Mutex<Option<String>>,
    /// Autosave / crash recovery: this session's record, and what the previous (crashed) session left to restore.
    rec: Mutex<recovery::State>,
    offer: Mutex<Option<recovery::Offer>>,
    /// Updates: the newest check (`{"state": "available"|"none"|"off"|"error", ...}`), and set while an update installs
    /// (the exit then leaves recovery "not clean", so unsaved work is offered back after the restart).
    update: Mutex<Value>,
    updating: AtomicBool,
}

impl AppState {
    fn launch(&self) -> Option<Launch> {
        let bin = self.sup.bin.clone()?;
        Some(Launch { command: bin.display().to_string(), args: vec![], env: engine_env(self.fonts.as_deref(), &self.data_dir, &self.parts) })
    }
    /// What agents launch for kimo: this very app binary in `mcp` mode (joins this app's engine).
    fn kimo_launch(&self) -> KimoLaunch {
        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "kimo-gui".into());
        KimoLaunch { command: exe, env: vec![] }
    }
    fn save(&self) { let _ = self.settings.lock().unwrap().save(&self.settings_path); }
    fn provider_cfg(&self, id: &str) -> Option<(&'static keys::Provider, String, String)> {
        let p = keys::provider(id)?;
        let s = self.settings.lock().unwrap();
        let base = s.base_urls.get(id).filter(|b| !b.is_empty()).cloned().unwrap_or_else(|| p.base_url.to_string());
        let model = s.models.get(id).filter(|m| !m.is_empty()).cloned().unwrap_or_else(|| p.default_model.to_string());
        Some((p, base, model))
    }
    fn app_tools(&self) -> As { As { ledger: self.ledger.clone(), actor: "app" } }
    fn workspace(&self) -> PathBuf { let w = self.data_dir.join("agent-workspace"); let _ = std::fs::create_dir_all(&w); w }
    fn remember_project(&self, path: &str) {
        *self.project_path.lock().unwrap() = Some(path.to_string());
        self.settings.lock().unwrap().touch_recent(path);
        self.save();
        self.mark_saved();
    }
    /// Everything up to now is in the user's own file (saved, opened or a new empty project): nothing to recover.
    /// Autosave right now (with history), whatever the timer says. true when written.
    fn autosave_now(&self) -> bool {
        if !matches!(self.sup.status(), Status::Running { .. }) { return false; }
        let rev = self.ledger.revision();
        let pp = self.project_path.lock().unwrap().clone();
        let path = recovery::autosave_path(&self.data_dir, pp.as_deref(), std::process::id());
        if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); }
        // the project and its undo history (`<autosave>.history`), so Undo still works after a crash restore
        let ok = self.sup.call("project", &recovery::autosave_call(&path)).map(|r| !engine::is_error(&r)).unwrap_or(false);
        if !ok { return false; }
        let mut r = self.rec.lock().unwrap();
        r.autosave = Some(path.display().to_string()); r.autosaved_at = ledger::now_ms() as u64; r.autosaved_rev = rev; r.project_path = pp;
        recovery::store(&self.data_dir, &r);
        true
    }
    fn mark_saved(&self) {
        let mut r = self.rec.lock().unwrap();
        r.saved_rev = self.ledger.revision();
        r.project_path = self.project_path.lock().unwrap().clone();
        recovery::store(&self.data_dir, &r);
    }
}

fn blocking<R: tauri::Runtime, T: Send + 'static>(app: tauri::AppHandle<R>, f: impl FnOnce(&AppState) -> Result<T, String> + Send + 'static) -> impl std::future::Future<Output = Result<T, String>> {
    async move { tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>())).await.map_err(|e| e.to_string())? }
}

/// An engine result for the webview: ok flag, the text, and the text parsed as JSON when it is JSON.
fn shaped(r: Value) -> Value {
    let text = engine::result_text(&r);
    let data = serde_json::from_str::<Value>(&text).ok();
    json!({"ok": !engine::is_error(&r), "text": text, "data": data})
}

// ---------- state ----------
#[tauri::command]
fn app_state(st: State<'_, AppState>) -> Value {
    let s = st.settings.lock().unwrap().clone();
    let keys: Vec<Value> = keys::PROVIDERS.iter().map(|p| {
        let k = st.keys.get(p.id);
        json!({"provider": p.id, "set": k.is_some(), "hint": k.map(|k| k.hint()), "remembered": s.remembered.iter().any(|r| r == p.id)})
    }).collect();
    let launch = st.launch();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "engine": st.sup.status(),
        "mcp_enabled": s.mcp_enabled,
        "external_port": st.external.lock().unwrap().as_ref().map(|e| e.port),
        "bridge_port": st.bridge.lock().unwrap().as_ref().map(|b| b.info.port),
        "attached": *st.attached.lock().unwrap(),
        "snippets": launch.as_ref().map(|l| mcp_config::snippets(&Launch { command: st.kimo_launch().command, args: vec!["mcp".into()], env: l.env.clone() }, cfg!(windows))),
        "kimo_launch": {"command": st.kimo_launch().command, "args": ["mcp"]},
        "settings": s,
        "update": *st.update.lock().unwrap(),
        "providers": keys::PROVIDERS,
        "keys": keys,
        "key_store": st.keys.store_label(),
        "data_dir": st.data_dir.display().to_string(),
        "project_path": *st.project_path.lock().unwrap(),
        "busy": st.busy.load(Ordering::SeqCst),
        "os": std::env::consts::OS,
        "fonts": st.fonts.as_ref().map(|f| f.display().to_string()),
    })
}

#[tauri::command]
async fn engine_restart<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Status, String> {
    blocking(app, |st| { let s = st.sup.start(); st.ledger.reset(); *st.project_path.lock().unwrap() = None; Ok(s) }).await
}

// ---------- the engine, for the editor ----------
/// An edit or read the user makes in the editor (Live and Changes show it as "you").
#[tauri::command]
async fn engine_call<R: tauri::Runtime>(app: tauri::AppHandle<R>, tool: String, args: Value) -> Result<Value, String> {
    blocking(app, move |st| st.ledger.call("you", &tool, &args).map(shaped)).await
}

/// The window's own refresh reads (not listed in Live).
#[tauri::command]
async fn engine_read<R: tauri::Runtime>(app: tauri::AppHandle<R>, tool: String, args: Value) -> Result<Value, String> {
    blocking(app, move |st| {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("");
        if !ledger::is_read_only(&tool, action) { return Err(format!("{tool}.{action} changes the project; use engine_call")); }
        st.ledger.call("app", &tool, &args).map(shaped)
    }).await
}

#[tauri::command]
fn changes_list(st: State<'_, AppState>) -> Vec<ledger::Change> { st.ledger.changes() }

#[tauri::command]
async fn change_undo<R: tauri::Runtime>(app: tauri::AppHandle<R>, id: u64) -> Result<Value, String> {
    blocking(app, move |st| st.ledger.undo_change("you", id)).await
}

// ---------- projects ----------
#[tauri::command]
async fn project_open<R: tauri::Runtime>(app: tauri::AppHandle<R>, path: String) -> Result<String, String> {
    blocking(app, move |st| {
        let r = st.ledger.call("you", "project", &json!({"action": "load_project", "args": {"path": path}}))?;
        if engine::is_error(&r) { return Err(engine::result_text(&r)); }
        st.ledger.reset();
        *st.chat.lock().unwrap() = Chat::default();
        st.remember_project(&path);
        Ok(engine::result_text(&r))
    }).await
}

#[tauri::command]
async fn project_save<R: tauri::Runtime>(app: tauri::AppHandle<R>, path: String) -> Result<String, String> {
    blocking(app, move |st| {
        let path = if Path::new(&path).extension().is_none() { format!("{path}.kimo") } else { path };
        let r = st.ledger.call("you", "project", &json!({"action": "save_project", "args": {"path": path}}))?;
        if engine::is_error(&r) { return Err(engine::result_text(&r)); }
        st.remember_project(&path);
        Ok(path)
    }).await
}

/// A fresh, empty project (the engine restarts; agents attached through the bridge see the new project too).
#[tauri::command]
async fn project_new<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Status, String> {
    let a2 = app.clone();
    let s = blocking(app, |st| { let s = st.sup.start(); st.ledger.reset(); *st.project_path.lock().unwrap() = None; *st.chat.lock().unwrap() = Chat::default(); st.sessions.lock().unwrap().clear(); st.mark_saved(); Ok(s) }).await?;
    let _ = a2.emit("project-changed", json!({"actor": "you", "action": "new_project"}));
    Ok(s)
}

// ---------- native dialogs ----------
const MEDIA_EXT: &[&str] = &["mp4", "mov", "m4v", "mkv", "webm", "avi", "mxf", "mts", "m2ts", "mpg", "mpeg", "wmv", "flv", "3gp",
    "mp3", "wav", "aac", "m4a", "flac", "ogg", "opus", "aiff", "aif", "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "exr", "svg"];

/// kind: "media" (several files) | "project" (open) | "save_project" | "export" (save). Returns absolute paths.
#[tauri::command]
async fn pick<R: tauri::Runtime>(app: tauri::AppHandle<R>, kind: String, suggest: Option<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let d = app.dialog().file();
        let to = |p: tauri_plugin_dialog::FilePath| p.into_path().ok().map(|p| p.display().to_string());
        let out: Vec<String> = match kind.as_str() {
            "media" => d.set_title("Import media").add_filter("Video, audio and images", MEDIA_EXT).add_filter("All files", &["*"])
                .blocking_pick_files().unwrap_or_default().into_iter().filter_map(to).collect(),
            "project" => d.set_title("Open a kimo project").add_filter("kimo project", &["kimo", "json"]).blocking_pick_file().and_then(to).into_iter().collect(),
            "save_project" => d.set_title("Save project").add_filter("kimo project", &["kimo"]).set_file_name(suggest.unwrap_or_else(|| "project.kimo".into()))
                .blocking_save_file().and_then(to).into_iter().collect(),
            "export" => d.set_title("Export video").add_filter("MP4 video", &["mp4"]).set_file_name(suggest.unwrap_or_else(|| "export.mp4".into()))
                .blocking_save_file().and_then(to).into_iter().collect(),
            _ => return Err("unknown dialog".into()),
        };
        Ok(out)
    }).await.map_err(|e| e.to_string())?
}

/// A small JPEG of a media file (first second) for the media bin, made with ffmpeg into the viewer cache.
#[tauri::command]
async fn media_thumb<R: tauri::Runtime>(app: tauri::AppHandle<R>, path: String) -> Result<Option<String>, String> {
    blocking(app, move |st| {
        use sha2::{Digest, Sha512};
        let p = Path::new(&path);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "mp3" | "wav" | "aac" | "m4a" | "flac" | "ogg" | "opus" | "aiff" | "aif") { return Ok(None); }
        let mtime = std::fs::metadata(p).and_then(|m| m.modified()).ok().map(|t| format!("{t:?}")).unwrap_or_default();
        let h: String = Sha512::digest(format!("{path}|{mtime}").as_bytes()).iter().take(12).map(|b| format!("{b:02x}")).collect();
        let name = format!("{h}.jpg");
        let out = st.viewer.dir().join(&name);
        if !out.is_file() {
            let Some(ff) = viewer::find_ffmpeg(&st.data_dir) else { return Ok(None) };
            let _ = std::fs::create_dir_all(st.viewer.dir());
            let mut c = std::process::Command::new(ff);
            c.args(["-v", "error", "-y", "-ss", "0.8", "-i"]).arg(p).args(["-frames:v", "1", "-vf", "scale=320:-2", "-q:v", "5"]).arg(&out)
                .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            #[cfg(windows)]
            { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); }
            let ok = c.status().map(|s| s.success()).unwrap_or(false) && out.is_file();
            if !ok {
                // very short clips: take the first frame instead
                let mut c2 = std::process::Command::new(viewer::find_ffmpeg(&st.data_dir).unwrap());
                c2.args(["-v", "error", "-y", "-i"]).arg(p).args(["-frames:v", "1", "-vf", "scale=320:-2", "-q:v", "5"]).arg(&out)
                    .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
                #[cfg(windows)]
                { use std::os::windows::process::CommandExt; c2.creation_flags(0x0800_0000); }
                if !(c2.status().map(|s| s.success()).unwrap_or(false) && out.is_file()) { return Ok(None); }
            }
        }
        Ok(Some(viewer::cache_url(&name)))
    }).await
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> { cli_agents::open_url(&url) }

#[tauri::command]
fn reveal(path: String) -> Result<(), String> { cli_agents::reveal(Path::new(&path)) }

// ---------- subscription agents ----------
#[tauri::command]
async fn cli_status<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Vec<cli_agents::Status>, String> {
    blocking(app, |_| {
        let hs: Vec<_> = Kind::all().into_iter().map(|k| std::thread::spawn(move || cli_agents::status(k))).collect();
        Ok(hs.into_iter().filter_map(|h| h.join().ok()).collect())
    }).await
}

/// Opens a terminal running the CLI's own login (the user signs in there; kimo never sees the credentials).
#[tauri::command]
fn cli_signin(id: String) -> Result<String, String> {
    let k = Kind::from_id(&id).ok_or("unknown agent")?;
    let bin = cli_agents::find(k).ok_or_else(|| format!("{} is not installed yet.", k.name()))?;
    let cmd = match k { Kind::ClaudeCode => format!("\"{}\"", bin.display()), Kind::Codex => format!("\"{}\" login", bin.display()), Kind::Antigravity => format!("\"{}\"", bin.display()) };
    cli_agents::open_terminal(&cmd)?;
    Ok(match k {
        Kind::ClaudeCode => "A terminal opened with Claude Code. Type /login, pick your Claude subscription and finish in the browser, then close the terminal.".into(),
        Kind::Codex => "A terminal opened with `codex login`. Choose “Sign in with ChatGPT”, finish in the browser, then close the terminal.".into(),
        Kind::Antigravity => "A terminal opened with Antigravity. Sign in with your Google account when it asks, then close the terminal.".into(),
    })
}

/// Opens a terminal running the npm install (visible to the user), or the download page.
#[tauri::command]
fn cli_install(id: String) -> Result<String, String> {
    let k = Kind::from_id(&id).ok_or("unknown agent")?;
    match k.install_cmd() {
        Some(c) if agents::which("npm").is_some() => { cli_agents::open_terminal(c)?; Ok(format!("Installing {} in a terminal window. When it finishes, press Check again.", k.name())) }
        _ => { cli_agents::open_url(k.install_url())?; Ok(format!("Opened the {} download page in your browser.", k.name())) }
    }
}

/// Add kimo to the CLI's own MCP list too, so the user's own sessions (terminal/IDE) can use it.
#[tauri::command]
async fn cli_register<R: tauri::Runtime>(app: tauri::AppHandle<R>, id: String) -> Result<String, String> {
    blocking(app, move |st| {
        let k = Kind::from_id(&id).ok_or("unknown agent")?;
        let bin = cli_agents::find(k).ok_or_else(|| format!("{} is not installed.", k.name()))?;
        cli_agents::register(k, &bin, &st.kimo_launch())
    }).await
}

/// The models (and effort levels) a subscription agent offers, straight from its CLI.
#[tauri::command]
async fn cli_models<R: tauri::Runtime>(app: tauri::AppHandle<R>, id: String) -> Result<cli_agents::Models, String> {
    blocking(app, move |_| { let k = Kind::from_id(&id).ok_or("unknown agent")?; Ok(cli_agents::models(k)) }).await
}

#[tauri::command]
fn agent_select(st: State<'_, AppState>, agent: String, model: Option<String>, effort: Option<String>, select: Option<bool>) -> Result<(), String> {
    if agent != "api" && Kind::from_id(&agent).is_none() { return Err("unknown agent".into()); }
    let mut s = st.settings.lock().unwrap();
    if select != Some(false) { s.agent = agent.clone(); }
    if let Some(e) = effort { s.agent_efforts.insert(agent.clone(), e.trim().to_string()); }
    if let Some(m) = model { s.agent_models.insert(agent, m.trim().to_string()); }
    drop(s);
    st.save();
    Ok(())
}

fn emit_ev<R: tauri::Runtime>(app: &tauri::AppHandle<R>, agent: &str, turn: u64, ev: &Ev) {
    let _ = app.emit("agent-event", json!({"agent": agent, "turn": turn, "ev": ev}));
}

/// One message to the selected agent. Streams `agent-event`s; returns the final reply.
#[tauri::command]
async fn agent_send<R: tauri::Runtime>(app: tauri::AppHandle<R>, text: String, turn: u64) -> Result<Value, String> {
    let a2 = app.clone();
    blocking(app, move |st| {
        if text.trim().is_empty() { return Err("Type what you want to change.".into()); }
        if st.busy.swap(true, Ordering::SeqCst) { return Err("The agent is still working on the last message. Press Stop to end it.".into()); }
        let _guard = scopeguard(&st.busy);
        let agent = st.settings.lock().unwrap().agent.clone();
        if agent == "api" { return api_chat(&a2, st, &text, turn); }
        let k = Kind::from_id(&agent).ok_or("Pick an agent first.")?;
        let bin = cli_agents::find(k).ok_or_else(|| format!("{} is not installed on this computer. Open Connections to install it.", k.name()))?;
        if k == Kind::Antigravity && !cli_agents::registered(k) {
            emit_ev(&a2, &agent, turn, &Ev::Status { text: "Adding kimo to Antigravity's MCP servers (one time)…".into() });
            cli_agents::register(k, &bin, &st.kimo_launch()).map_err(|e| format!("Could not add kimo to Antigravity: {e}"))?;
        }
        let model = st.settings.lock().unwrap().agent_models.get(&agent).cloned().filter(|m| !m.is_empty());
        let effort = st.settings.lock().unwrap().agent_efforts.get(&agent).cloned().filter(|e| !e.is_empty());
        let session = st.sessions.lock().unwrap().get(&agent).cloned();
        let launch = st.kimo_launch();
        let workdir = st.workspace();
        let t = cli_agents::Turn { kind: k, bin: &bin, prompt: &text, session: session.as_deref(), model: model.as_deref(), effort: effort.as_deref(), launch: &launch, workdir: &workdir };
        let using = model.as_deref().map(|m| format!(" · {m}")).unwrap_or_default() + &effort.as_deref().map(|e| format!(" · {e} effort")).unwrap_or_default();
        emit_ev(&a2, &agent, turn, &Ev::Status { text: format!("{} is working on your {}{using}", k.name(), k.plan()) });
        let mut o = cli_agents::run(&t, &st.turn, &mut |ev| emit_ev(&a2, &agent, turn, &ev));
        // a resumed session the CLI no longer knows: start a fresh one once
        if o.error.as_deref().map(|e| e.to_ascii_lowercase().contains("session") || e.to_ascii_lowercase().contains("conversation")).unwrap_or(false) && session.is_some() && o.reply.is_empty() {
            st.sessions.lock().unwrap().remove(&agent);
            let t2 = cli_agents::Turn { session: None, ..t };
            o = cli_agents::run(&t2, &st.turn, &mut |ev| emit_ev(&a2, &agent, turn, &ev));
        }
        if let Some(s) = &o.session { st.sessions.lock().unwrap().insert(agent.clone(), s.clone()); }
        if let Some(e) = o.error { return Err(e); }
        Ok(json!({"reply": o.reply, "session": o.session}))
    }).await
}

struct Release<'a>(&'a AtomicBool);
impl Drop for Release<'_> { fn drop(&mut self) { self.0.store(false, Ordering::SeqCst); } }
fn scopeguard(b: &AtomicBool) -> Release<'_> { Release(b) }

/// The API-key agent (kept for people without a subscription CLI), speaking the same agent-event stream.
fn api_chat<R: tauri::Runtime>(app: &tauri::AppHandle<R>, st: &AppState, text: &str, turn: u64) -> Result<Value, String> {
    let provider = st.settings.lock().unwrap().provider.clone();
    let (p, base, model) = st.provider_cfg(&provider).ok_or("Pick a provider in Connections first.")?;
    let key = st.keys.get(&provider).ok_or_else(|| format!("Add your {} key in Connections, or pick Claude Code, Codex or Antigravity to use your subscription.", p.name))?;
    if model.is_empty() { return Err("Type a model name for this provider in Connections.".into()); }
    let llm = agent::Llm { wire: p.wire, base_url: base, model, key };
    let mut chat = st.chat.lock().map_err(|_| "chat is busy")?;
    let tools = As { ledger: st.ledger.clone(), actor: "kimo-chat" };
    let reply = chat.send(&llm, &tools, text, &mut |ev| {
        let e = match ev {
            agent::Event::Thinking { step } => Ev::Status { text: format!("thinking (step {})", step + 1) },
            agent::Event::ToolStart { call, tool, args, .. } => Ev::ToolStart { id: call, tool, input: args },
            agent::Event::ToolEnd { call, ok, result, .. } => Ev::ToolEnd { id: call, ok, output: result },
            agent::Event::Reply { text } => Ev::Done { text, cost_usd: None, turns: None },
            agent::Event::Error { message } => Ev::Error { message },
        };
        emit_ev(app, "api", turn, &e);
    })?;
    Ok(json!({"reply": reply}))
}

#[tauri::command]
fn agent_stop(st: State<'_, AppState>) -> bool { cli_agents::stop(&st.turn) }

/// Forget the conversation (the next message starts a new session with the agent).
#[tauri::command]
fn agent_reset(st: State<'_, AppState>) {
    st.sessions.lock().unwrap().clear();
    *st.chat.lock().unwrap() = Chat::default();
}

// ---------- MCP switch (external engines) ----------
#[tauri::command]
async fn mcp_probe<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<engine::ProbeInfo, String> {
    blocking(app, |st| {
        let l = st.launch().ok_or("The kimo engine (kimo-mcp) was not found next to the app.")?;
        Ok(engine::probe(Path::new(&l.command), &l.args, &l.env))
    }).await
}

fn start_external<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<u16, String> {
    let st = app.state::<AppState>();
    if let Some(e) = st.external.lock().unwrap().as_ref() { return Ok(e.port); }
    let (a1, a2) = (app.clone(), app.clone());
    let ext = telemetry::listen_external(
        Arc::new(move |src, ev| forward(&a1, src, ev)),
        Arc::new(move |s| { let _ = a2.emit("spectator-status", s); }),
    )?;
    let port = ext.port;
    *st.external.lock().unwrap() = Some(ext);
    Ok(port)
}

#[tauri::command]
async fn mcp_set<R: tauri::Runtime>(app: tauri::AppHandle<R>, enabled: bool) -> Result<Value, String> {
    let a2 = app.clone();
    blocking(app, move |st| {
        if !enabled {
            if let Some(e) = st.external.lock().unwrap().take() { e.stop(); }
            st.settings.lock().unwrap().mcp_enabled = false;
            st.save();
            return Ok(json!({"enabled": false}));
        }
        let l = st.launch().ok_or("The kimo engine (kimo-mcp) was not found next to the app, so there is nothing to connect to.")?;
        let probe = engine::probe(Path::new(&l.command), &l.args, &l.env);
        if !probe.ok { return Ok(json!({"enabled": false, "probe": probe})); }
        let port = start_external(&a2)?;
        st.settings.lock().unwrap().mcp_enabled = true;
        st.save();
        Ok(json!({"enabled": true, "probe": probe, "external_port": port, "snippets": mcp_config::snippets(&l, cfg!(windows))}))
    }).await
}

// ---------- API keys (optional) ----------
#[tauri::command]
fn key_set(st: State<'_, AppState>, provider: String, key: String, remember: bool) -> Result<keys::KeyStatus, String> {
    keys::provider(&provider).ok_or("unknown provider")?;
    let k = Secret::new(key);
    if k.is_empty() { return Err("Paste a key first.".into()); }
    let status = st.keys.set(&provider, k, remember);
    {
        let mut s = st.settings.lock().unwrap();
        s.remembered.retain(|p| p != &provider);
        if status.remembered { s.remembered.push(provider.clone()); }
        s.provider = provider;
    }
    st.save();
    Ok(status)
}

#[tauri::command]
fn key_clear(st: State<'_, AppState>, provider: String) -> keys::KeyStatus {
    let r = st.keys.clear(&provider);
    st.settings.lock().unwrap().remembered.retain(|p| p != &provider);
    st.save();
    r
}

#[tauri::command]
async fn key_test<R: tauri::Runtime>(app: tauri::AppHandle<R>, provider: String, key: Option<String>) -> Result<String, String> {
    blocking(app, move |st| {
        let (p, base, _) = st.provider_cfg(&provider).ok_or("unknown provider")?;
        let k = key.map(Secret::new).filter(|k| !k.is_empty()).or_else(|| st.keys.get(&provider)).ok_or("Paste a key first.")?;
        keys::test_connection(p.wire, &base, &k)
    }).await
}

#[tauri::command]
fn settings_set(st: State<'_, AppState>, provider: String, model: Option<String>, base_url: Option<String>) -> Result<(), String> {
    keys::provider(&provider).ok_or("unknown provider")?;
    let mut s = st.settings.lock().unwrap();
    s.provider = provider.clone();
    if let Some(m) = model { s.models.insert(provider.clone(), m.trim().to_string()); }
    if let Some(b) = base_url { s.base_urls.insert(provider, b.trim().to_string()); }
    drop(s);
    st.save();
    Ok(())
}

// ---------- crash recovery ----------
/// Unsaved work the previous session left when it did not close normally (PC crash, power cut), if any.
#[tauri::command]
fn recovery_status(st: State<'_, AppState>) -> Option<recovery::Offer> { st.offer.lock().unwrap().clone() }

/// Open the autosaved work. Save then writes to the user's original file (when there was one).
#[tauri::command]
async fn recovery_restore<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Value, String> {
    let a2 = app.clone();
    let v = blocking(app, |st| {
        let o = st.offer.lock().unwrap().take().ok_or("nothing to restore")?;
        // with its undo history: Ctrl+Z after the restore undoes the edits made before the crash
        let r = st.ledger.call("you", "project", &recovery::restore_call(&o.autosave))?;
        if engine::is_error(&r) { return Err(engine::result_text(&r)); }
        st.ledger.reset();
        *st.project_path.lock().unwrap() = o.project_path.clone();
        // restored work is unsaved work: keep it recoverable until the user saves
        let mut rec = st.rec.lock().unwrap(); rec.project_path = o.project_path.clone(); rec.saved_rev = 0; recovery::store(&st.data_dir, &rec);
        Ok(json!({"project_path": o.project_path, "autosaved_at": o.autosaved_at}))
    }).await?;
    let _ = a2.emit("project-changed", json!({"actor": "you", "action": "restore"}));
    Ok(v)
}

#[tauri::command]
fn recovery_discard(st: State<'_, AppState>) {
    if let Some(o) = st.offer.lock().unwrap().take() { recovery::remove_autosave(&o.autosave); }
    let mut r = st.rec.lock().unwrap();
    *r = recovery::State { clean: false, pid: std::process::id(), ..Default::default() };
    recovery::store(&st.data_dir, &r);
}

// ---------- crash reports (opt-in) ----------
/// Crashes nobody answered yet (newest first): the window asks once whether to send a report.
#[tauri::command]
fn crash_pending(st: State<'_, AppState>) -> Vec<crash::Report> { crash::pending(&crash::dir(&st.data_dir)) }

/// The person's answer. `send`: open the pre-filled report in their browser, where they read and submit it themselves
/// (kimo itself sends nothing). Either way the log moves to `crashes/answered` and is not asked about again.
#[tauri::command]
fn crash_answer(st: State<'_, AppState>, file: String, send: bool) -> Result<(), String> {
    let d = crash::dir(&st.data_dir);
    if send {
        let r = crash::pending(&d).into_iter().find(|r| r.file == file).ok_or("That crash log is no longer there.")?;
        cli_agents::open_url(&crash::issue_url(&r, agents::home_dir().as_deref()))?;
    }
    crash::answer(&d, &file)
}

// ---------- updates (GitHub releases, signed) ----------
/// The updater's public key from the bundle config (the release workflow sets it; a dev or self-built app has none).
fn updater_pubkey<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    app.config().plugins.0.get("updater")?.get("pubkey")?.as_str().map(str::trim).filter(|k| !k.is_empty()).map(String::from)
}

/// Ask the release feed whether a newer kimo exists. Records the answer and announces `update-available` when there is one.
async fn check_update<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Value {
    let mut v = if updater_pubkey(app).is_none() {
        json!({"state": "off", "reason": "this build has no update key (only release builds update themselves)"})
    } else {
        match app.updater() {
            Err(e) => json!({"state": "error", "error": e.to_string()}),
            Ok(u) => match u.check().await {
                Ok(Some(up)) => json!({"state": "available", "version": up.version, "current": up.current_version, "notes": up.body, "date": up.date.map(|d| d.to_string())}),
                Ok(None) => json!({"state": "none", "current": env!("CARGO_PKG_VERSION")}),
                Err(e) => json!({"state": "error", "error": e.to_string()}),
            },
        }
    };
    v["checked_at"] = json!(ledger::now_ms() as u64);
    *app.state::<AppState>().update.lock().unwrap() = v.clone();
    if v["state"] == "available" { let _ = app.emit("update-available", &v); }
    v
}

#[tauri::command]
async fn update_check<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Value, String> { Ok(check_update(&app).await) }

/// Install the available update, on the person's click: download and verify it (signature), autosave the project with its
/// history, stop the engine and agents (the installer replaces their files), install, restart. Unsaved work is offered
/// back after the restart, exactly like after a crash.
#[tauri::command]
async fn update_install<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    if updater_pubkey(&app).is_none() { return Err("This build does not update itself. Download the newest kimo from getkimo.tech.".into()); }
    let up = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?.ok_or("kimo is already up to date.")?;
    let (h, mut done) = (app.clone(), 0usize);
    let bytes = up.download(move |n, total| { done += n; let _ = h.emit("update-progress", json!({"done": done, "total": total})); }, || {}).await.map_err(|e| e.to_string())?;
    let a2 = app.clone();
    blocking(app.clone(), move |st| {
        st.updating.store(true, Ordering::SeqCst);
        st.autosave_now();
        cli_agents::stop(&st.turn);
        if let Some(b) = st.bridge.lock().unwrap().take() { b.remove_file(); }
        st.sup.stop();
        let _ = a2.emit("update-installing", json!({"version": up.version}));
        up.install(bytes).map_err(|e| { st.updating.store(false, Ordering::SeqCst); st.sup.start(); e.to_string() })
    }).await?;
    // Windows: the installer has already closed kimo and reopens it. Linux: the AppImage / package is replaced, restart into it.
    app.restart();
}

#[tauri::command]
fn onboarded_set(st: State<'_, AppState>) { st.settings.lock().unwrap().onboarded = true; st.save(); }

// ---------- viewer ----------
#[tauri::command]
async fn viewer_frame<R: tauri::Runtime>(app: tauri::AppHandle<R>, time: f64) -> Result<viewer::Shown, String> {
    blocking(app, move |st| st.viewer.frame(&st.app_tools(), time)).await
}

#[tauri::command]
async fn viewer_draft<R: tauri::Runtime>(app: tauri::AppHandle<R>, start: f64, seconds: f64) -> Result<viewer::Draft, String> {
    blocking(app, move |st| st.viewer.draft(&st.app_tools(), start, seconds)).await
}

// ---------- real-time preview (the engine's preview_stream: frames + audio chunks ahead of the playhead) ----------
/// Start a preview session at `time` (scale 0.5 half / 0.25 quarter of the project size).
#[tauri::command]
async fn preview_start<R: tauri::Runtime>(app: tauri::AppHandle<R>, time: f64, scale: Option<f64>, rate: Option<f64>, playing: Option<bool>, warming: Option<bool>) -> Result<Value, String> {
    blocking(app, move |st| st.viewer.stream(&st.app_tools(), "start", json!({"time": time, "scale": scale.unwrap_or(0.5), "rate": rate.unwrap_or(1.0), "playing": playing.unwrap_or(false), "warming": warming.unwrap_or(false)}))).await
}

/// The poll: the playhead (and play state / speed) in, the frames and audio chunks ready ahead out. `seek` starts a new run.
#[tauri::command]
async fn preview_update<R: tauri::Runtime>(app: tauri::AppHandle<R>, time: f64, playing: Option<bool>, rate: Option<f64>, seek: Option<bool>) -> Result<Value, String> {
    let mut a = json!({"time": time});
    if let Some(p) = playing { a["playing"] = json!(p); }
    if let Some(r) = rate { a["rate"] = json!(r); }
    blocking(app, move |st| st.viewer.stream(&st.app_tools(), if seek == Some(true) { "seek" } else { "update" }, a)).await
}

/// One draft frame for scrubbing, now.
#[tauri::command]
async fn preview_frame<R: tauri::Runtime>(app: tauri::AppHandle<R>, time: f64) -> Result<Value, String> {
    blocking(app, move |st| st.viewer.stream(&st.app_tools(), "frame", json!({"time": time}))).await
}

#[tauri::command]
async fn preview_stop<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Value, String> {
    blocking(app, move |st| st.viewer.stream(&st.app_tools(), "stop", json!({}))).await
}

#[tauri::command]
fn open_spectator<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("spectator") { return w.set_focus().map_err(|e| e.to_string()); }
    tauri::WebviewWindowBuilder::new(&app, "spectator", tauri::WebviewUrl::App("index.html".into()))
        .title("kimo \u{b7} live graph").inner_size(1376.0, 768.0).min_inner_size(1200.0, 700.0)
        .build().map(|_| ()).map_err(|e| e.to_string())
}

// ---------- first-run setup ----------
#[tauri::command]
fn setup_status(st: State<'_, AppState>) -> Value {
    let parts: Vec<Value> = st.parts.iter().map(|p| json!({
        "id": p.id, "name": p.name, "note": p.note, "size": p.size, "optional": p.optional,
        "pinned": download::pinned(p), "installed": download::installed(&st.data_dir, p), "satisfied": part_satisfied(&st.data_dir, p),
    })).collect();
    let ready = st.parts.iter().filter(|p| !p.optional).all(|p| part_satisfied(&st.data_dir, p).is_some());
    json!({"done": st.settings.lock().unwrap().setup_done, "ready": ready, "parts": parts, "dir": st.data_dir.display().to_string()})
}

#[tauri::command]
async fn setup_start<R: tauri::Runtime>(app: tauri::AppHandle<R>, skip_optional: bool) -> Result<Value, String> {
    let a2 = app.clone();
    blocking(app, move |st| {
        st.cancel_dl.store(false, Ordering::SeqCst);
        let mut results = vec![];
        for p in st.parts.iter().filter(|p| !(skip_optional && p.optional)) {
            if download::installed(&st.data_dir, p) { results.push(json!({"id": p.id, "ok": true, "already": true})); continue; }
            if !download::pinned(p) {
                let sys = part_satisfied(&st.data_dir, p);
                let _ = a2.emit("setup-progress", json!({"id": p.id, "phase": if sys.is_some() { "system" } else { "unavailable" }}));
                results.push(json!({"id": p.id, "ok": sys.is_some(), "system": sys.is_some(), "error": if sys.is_some() { Value::Null } else { json!("not published for download yet") }}));
                continue;
            }
            let a = a2.clone();
            match download::fetch(&st.data_dir, p, &st.cancel_dl, &mut |pr| { let _ = a.emit("setup-progress", &pr); }) {
                Ok(_) => results.push(json!({"id": p.id, "ok": true})),
                Err(e) => { let msg = e.to_string(); let _ = a2.emit("setup-progress", json!({"id": p.id, "phase": "error", "error": msg})); results.push(json!({"id": p.id, "ok": false, "error": msg})); if e == download::Failure::Cancelled { break; } }
            }
        }
        let ready = st.parts.iter().filter(|p| !p.optional).all(|p| part_satisfied(&st.data_dir, p).is_some());
        if ready { st.settings.lock().unwrap().setup_done = true; st.save(); st.sup.start(); st.ledger.reset(); }
        Ok(json!({"results": results, "ready": ready}))
    }).await
}

#[tauri::command]
fn setup_pause(st: State<'_, AppState>) { st.cancel_dl.store(true, Ordering::SeqCst); }

#[tauri::command]
fn setup_skip(st: State<'_, AppState>) { st.settings.lock().unwrap().setup_done = true; st.save(); }

// ---------- live feed ----------
/// Engine telemetry: raw to the live-graph window; activity from OTHER engines (an agent that started its own
/// kimo-mcp because it was configured before this app existed) is listed in Live as an external engine.
fn forward<R: tauri::Runtime>(app: &tauri::AppHandle<R>, src: Source, ev: Value) {
    if let Source::External { conn } = src { let _ = app.emit("external-event", json!({"conn": conn, "t": ledger::now_ms() as u64, "ev": ev})); }
    let _ = app.emit("engine-event", ev);
}

// ---------- `kimo-gui mcp` ----------
/// `kimo-gui mcp [--client NAME]`: what Claude Code / Codex / Antigravity launch. Joins the open kimo app's engine
/// (same project, live, undoable); with no app open, runs a standalone engine over stdio exactly as before.
fn run_mcp(args: &[String]) -> ! {
    let mut client = "agent".to_string();
    let mut rest = vec![];
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--client" { if let Some(c) = it.next() { client = c.clone(); } } else { rest.push(a.clone()); }
    }
    // a small diagnostic log next to bridge.json: which way this agent connected, and why it stopped
    let log = |msg: &str| {
        if let Some(d) = bridge::data_dir() {
            let _ = std::fs::create_dir_all(&d);
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("mcp-clients.log")) {
                use std::io::Write;
                let _ = writeln!(f, "{} pid={} client={client} {msg}", ledger::now_ms(), std::process::id());
            }
        }
    };
    log("start");
    if std::env::var_os("KIMO_STANDALONE").is_none() {
        match bridge::data_dir() {
            Some(dir) => match bridge::connect(&dir, &client) {
                Ok(s) => { log("attached to the open kimo app"); let code = bridge::relay_stdio(s); log("detached"); std::process::exit(code); }
                Err(e) => log(&format!("standalone engine ({e})")),
            },
            None => log("standalone engine (no data dir)"),
        }
    }
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let Some(bin) = engine::find_engine(exe_dir.as_deref(), None) else {
        eprintln!("kimo: the engine (kimo-mcp) was not found next to this app or on PATH");
        std::process::exit(127);
    };
    let mut c = std::process::Command::new(&bin);
    c.args(&rest);
    if std::env::var_os("KIMO_FONTS").is_none() { if let Some(f) = find_fonts(exe_dir.as_deref(), None, Some(&bin)) { c.env("KIMO_FONTS", f); } }
    let st = c.status();
    log(&format!("standalone engine ended: {st:?}"));
    std::process::exit(st.ok().and_then(|s| s.code()).unwrap_or(1));
}

fn bridge_handler<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> bridge::Handler {
    Arc::new(move |client, method, params| {
        let st = app.state::<AppState>();
        match method {
            "initialize" => Ok(bridge::initialize_result(&params, env!("CARGO_PKG_VERSION"))),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": st.ledger.tools()})),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).ok_or((-32602, "missing tool name".to_string()))?;
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                st.ledger.call(client, name, &args).map_err(|e| (-32603, e))
            }
            "resources/list" => Ok(json!({"resources": []})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
            "prompts/list" => Ok(json!({"prompts": []})),
            _ => Err((-32601, format!("method not found: {method}"))),
        }
    })
}

fn command_handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
            app_state, engine_restart, engine_call, engine_read, changes_list, change_undo,
            project_open, project_save, project_new, pick, media_thumb, open_url, reveal,
            cli_status, cli_signin, cli_install, cli_register, cli_models, agent_select, agent_send, agent_stop, agent_reset,
            mcp_set, mcp_probe, key_set, key_clear, key_test, settings_set, onboarded_set,
            recovery_status, recovery_restore, recovery_discard, crash_pending, crash_answer, update_check, update_install,
            open_spectator, setup_status, setup_start, setup_pause, setup_skip, viewer_frame, viewer_draft,
            preview_start, preview_update, preview_frame, preview_stop
        ]
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(|a| a == "mcp").unwrap_or(false) { run_mcp(&argv[1..]); }
    if argv.first().map(|a| a == "--version").unwrap_or(false) { println!("kimo-gui {}", env!("CARGO_PKG_VERSION")); return; }
    // a panic anywhere in the app leaves a local crash log; the next launch asks whether to send a report (never automatic)
    let data_dir_early = bridge::data_dir().unwrap_or_else(|| std::env::temp_dir().join("kimo"));
    crash::install(crash::dir(&data_dir_early), env!("CARGO_PKG_VERSION"));
    // `kimo-gui --crash-test`: a deliberate panic, to check the crash log and the next launch's prompt end to end
    if argv.first().map(|a| a == "--crash-test").unwrap_or(false) { panic!("kimo --crash-test: a deliberate crash to check crash reports"); }

    tauri::Builder::default()
        // must be the first plugin: a second launch hands its arguments here and exits
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") { let _ = w.unminimize(); let _ = w.show(); let _ = w.set_focus(); }
            if let Some(f) = argv.iter().skip(1).find(|a| a.to_ascii_lowercase().ends_with(".kimo")) { let _ = app.emit("open-file", f); }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = bridge::data_dir().or_else(|| app.path().app_data_dir().ok()).unwrap_or_else(|| std::env::temp_dir().join("kimo"));
            let config_dir = app.path().app_config_dir().unwrap_or_else(|_| data_dir.clone());
            let _ = std::fs::create_dir_all(&data_dir);
            let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
            let res_dir = app.path().resource_dir().ok();
            let bin = engine::find_engine(exe_dir.as_deref(), res_dir.as_deref());
            let fonts = find_fonts(exe_dir.as_deref(), res_dir.as_deref(), bin.as_deref());
            let parts = download::for_this_os(download::load_parts(PARTS_JSON).unwrap_or_default());
            let settings_path = config_dir.join("settings.json");
            let settings = Settings::load(&settings_path);

            // the app's own engine streams telemetry to a private loopback port (for the live-graph window)
            let h2 = handle.clone();
            let private = telemetry::listen_private(Arc::new(move |src, ev| forward(&h2, src, ev)))?;
            let mut env = engine_env(fonts.as_deref(), &data_dir, &parts);
            env.push(("KIMO_SPECTATOR".into(), format!("tcp://127.0.0.1:{private}")));
            env.push(("KIMO_INLINE_IMAGES".into(), "0".into()));
            // background proxies for heavy imports live in the app's data folder, so they survive restarts and project saves
            env.push(("KIMO_PROXY_CACHE".into(), data_dir.join("proxy-cache").join("edit").display().to_string()));
            env.push(("KIMO_ANALYSIS_PROXY_CACHE".into(), data_dir.join("proxy-cache").join("analysis").display().to_string()));
            let sup = Arc::new(Supervisor::new(bin, env, Some(data_dir.join("engine.log"))));
            let h3 = handle.clone();
            let ledger = Arc::new(Ledger::new(sup.clone(), Arc::new(move |kind, v| { let _ = h3.emit(kind, v); })));

            let keyring = KeyRing::new(Box::new(keys::OsKeychain));
            for p in &settings.remembered { let _ = keyring.restore(p); }
            let mcp_on = settings.mcp_enabled;
            let vdir = data_dir.join("viewer-cache");
            let _ = std::fs::remove_dir_all(&vdir);
            let viewer = Arc::new(Viewer::new(vdir).with_ffmpeg(viewer::find_ffmpeg(&data_dir)));
            // what the previous session left (offered once the window is up), then start this session's record
            let prev = recovery::load(&data_dir);
            let offer = recovery::offer(&prev);
            // an offer not yet answered stays on record (a second crash before Restore/Discard must not lose it)
            let rec_now = if offer.is_some() { recovery::State { clean: false, pid: std::process::id(), ..prev } } else { recovery::State { clean: false, pid: std::process::id(), ..Default::default() } };
            recovery::store(&data_dir, &rec_now);
            app.manage(AppState {
                sup: sup.clone(), ledger, keys: keyring, settings: Mutex::new(settings), settings_path,
                chat: Mutex::new(Chat::default()), external: Mutex::new(None), data_dir: data_dir.clone(), fonts, parts,
                cancel_dl: Arc::new(AtomicBool::new(false)), viewer, bridge: Mutex::new(None),
                turn: Arc::new(Mutex::new(None)), busy: AtomicBool::new(false), sessions: Mutex::new(HashMap::new()),
                attached: Mutex::new(HashMap::new()), project_path: Mutex::new(None),
                rec: Mutex::new(rec_now), offer: Mutex::new(offer),
                update: Mutex::new(json!({"state": "unchecked"})), updating: AtomicBool::new(false),
            });
            // updates: release builds ask GitHub releases once, shortly after start (the person installs with a click)
            if !cfg!(debug_assertions) && app.state::<AppState>().settings.lock().unwrap().update_check {
                let h7 = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(std::time::Duration::from_secs(8))).await;
                    let v = check_update(&h7).await;
                    if v["state"] == "error" { eprintln!("kimo: update check failed: {}", v["error"]); }
                });
            }
            // autosave (with undo history): every minute after an edit, or after 20 edits, straight to the engine (not an edit itself)
            let h6 = handle.clone();
            std::thread::spawn(move || {
                let (mut last_rev, mut last_t) = (0u64, std::time::Instant::now());
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let st = h6.state::<AppState>();
                    let rev = st.ledger.revision();
                    if !recovery::due(rev, last_rev, last_t.elapsed().as_secs_f64(), 60.0, 20) { continue; }
                    if !matches!(st.sup.status(), Status::Running { .. }) { continue; }
                    let ok = st.autosave_now();
                    last_t = std::time::Instant::now();
                    if !ok { continue; }
                    last_rev = rev;
                    let r = st.rec.lock().unwrap().clone();
                    let _ = h6.emit("autosaved", json!({"at": r.autosaved_at, "path": r.autosave}));
                }
            });
            // the bridge: agents launched with `kimo-gui mcp` join this engine
            let h4 = handle.clone();
            let on_conn: bridge::OnConn = Arc::new(move |label, up| {
                let st = h4.state::<AppState>();
                let mut a = st.attached.lock().unwrap();
                let n = a.entry(label.to_string()).or_insert(0);
                if up { *n += 1 } else { *n = n.saturating_sub(1) }
                if *n == 0 { a.remove(label); }
                let snapshot = a.clone();
                drop(a);
                let _ = h4.emit("agent-connection", json!({"client": label, "up": up, "attached": snapshot}));
            });
            match bridge::serve(Some(&data_dir), bridge_handler(handle.clone()), on_conn) {
                Ok(s) => *app.state::<AppState>().bridge.lock().unwrap() = Some(s),
                Err(e) => eprintln!("kimo: bridge not started: {e}"),
            }
            if mcp_on { if let Err(e) = start_external(&handle) { eprintln!("kimo: {e}"); } }
            let h5 = handle.clone();
            // started by double-clicking a .kimo file: open it once the engine is up
            let open_file = std::env::args().skip(1).find(|a| a.to_ascii_lowercase().ends_with(".kimo") && Path::new(a).is_file());
            std::thread::spawn(move || {
                let s = sup.start();
                if let (Some(f), Status::Running { .. }) = (&open_file, &s) {
                    let st = h5.state::<AppState>();
                    if let Ok(r) = st.ledger.call("you", "project", &json!({"action": "load_project", "args": {"path": f}})) {
                        if !engine::is_error(&r) { st.ledger.reset(); st.remember_project(f); }
                    }
                }
                let _ = h5.emit("engine-status", &s);
            });
            Ok(())
        })
        // kimo://localhost/<hex>.png|mp4 (http://kimo.localhost on Windows): the viewer cache, nothing else
        .register_uri_scheme_protocol("kimo", |ctx, req| {
            let dir = ctx.app_handle().state::<AppState>().viewer.dir().to_path_buf();
            let range = req.headers().get("range").and_then(|v| v.to_str().ok()).map(str::to_string);
            let (status, headers, body) = viewer::respond(&dir, req.uri().path(), range.as_deref());
            let mut b = tauri::http::Response::builder().status(status);
            for (k, v) in headers { b = b.header(k, v); }
            b.body(body).unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
        })
        .invoke_handler(command_handler())
        .build(tauri::generate_context!())
        .expect("kimo failed to start")
        .run(|app, ev| {
            if let tauri::RunEvent::Exit = ev {
                let st = app.state::<AppState>();
                cli_agents::stop(&st.turn);
                // an update restart is not a clean exit: unsaved work (autosaved just before) is offered back afterwards
                if !st.updating.load(Ordering::SeqCst) { let mut r = st.rec.lock().unwrap(); r.clean = true; recovery::store(&st.data_dir, &r); }
                if let Some(b) = st.bridge.lock().unwrap().take() { b.remove_file(); }
                st.sup.stop();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(root: &Path) -> AppState {
        let sup=Arc::new(Supervisor::new(None,vec![],None));
        let ledger=Arc::new(Ledger::new(sup.clone(),Arc::new(|_,_|{})));
        AppState {
            sup,ledger,keys:KeyRing::new(Box::new(keys::MemoryStore::default())),
            settings:Mutex::new(Settings::default()),settings_path:root.join("settings.json"),
            chat:Mutex::new(Chat::default()),external:Mutex::new(None),data_dir:root.to_owned(),fonts:None,parts:vec![],
            cancel_dl:Arc::new(AtomicBool::new(false)),viewer:Arc::new(Viewer::new(root.join("cache"))),bridge:Mutex::new(None),
            turn:Arc::new(Mutex::new(None)),busy:AtomicBool::new(false),sessions:Mutex::new(HashMap::new()),
            attached:Mutex::new(HashMap::new()),project_path:Mutex::new(None),rec:Mutex::new(Default::default()),offer:Mutex::new(None),
            update:Mutex::new(json!({"state":"unchecked"})),updating:AtomicBool::new(false),
        }
    }

    #[test]
    fn command_state_preserves_settings_keys_and_recovery_in_isolation() {
        let root=std::env::temp_dir().join(format!("kimo-main-state-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);
        let app=tauri::test::mock_app();assert!(app.manage(state(&root)));
        assert!(changes_list(app.state()).is_empty());
        assert!(!agent_stop(app.state()));
        assert!(agent_select(app.state(),"unknown".into(),None,None,None).is_err());
        let original=app.state::<AppState>().settings.lock().unwrap().agent.clone();
        agent_select(app.state(),"codex".into(),Some(" model ".into()),Some(" high ".into()),Some(false)).unwrap();
        assert_eq!(app.state::<AppState>().settings.lock().unwrap().agent,original);
        agent_select(app.state(),"api".into(),None,None,None).unwrap();
        assert_eq!(app.state::<AppState>().settings.lock().unwrap().agent,"api");
        assert_eq!(app.state::<AppState>().settings.lock().unwrap().agent_models["codex"],"model");
        assert!(settings_set(app.state(),"unknown".into(),None,None).is_err());
        settings_set(app.state(),"openai".into(),Some(" custom ".into()),Some(" http://fixture.invalid ".into())).unwrap();
        assert_eq!(app.state::<AppState>().provider_cfg("openai").unwrap().1,"http://fixture.invalid");
        assert_eq!(app.state::<AppState>().provider_cfg("openai").unwrap().2,"custom");
        assert!(app.state::<AppState>().provider_cfg("unknown").is_none());
        assert!(key_set(app.state(),"unknown".into(),"fixture-key".into(),false).is_err());
        assert!(key_set(app.state(),"openai".into(),"  ".into(),false).is_err());
        key_set(app.state(),"openai".into(),"fixture-secret-key".into(),true).unwrap();
        let snapshot=app_state(app.state());
        assert!(!snapshot.to_string().contains("fixture-secret-key"));
        assert!(snapshot["keys"].as_array().unwrap().iter().any(|k|k["provider"]=="openai"&&k["set"]==true&&k["remembered"]==true));
        key_set(app.state(),"openai".into(),"replacement-fixture-key".into(),false).unwrap();
        assert!(app.state::<AppState>().settings.lock().unwrap().remembered.is_empty());
        key_clear(app.state(),"openai".into());
        assert!(app.state::<AppState>().keys.get("openai").is_none());
        onboarded_set(app.state());setup_pause(app.state());setup_skip(app.state());
        assert!(app.state::<AppState>().cancel_dl.load(Ordering::SeqCst));
        assert_eq!(setup_status(app.state())["done"],true);
        assert_eq!(setup_status(app.state())["ready"],true);
        app.state::<AppState>().sessions.lock().unwrap().insert("codex".into(),"fixture-session".into());
        agent_reset(app.state());assert!(app.state::<AppState>().sessions.lock().unwrap().is_empty());
        assert!(recovery_status(app.state()).is_none());recovery_discard(app.state());
        assert!(!recovery::load(&root).clean);
        assert!(crash_pending(app.state()).is_empty());
        assert!(crash_answer(app.state(),"missing.log".into(),true).is_err());
        let st=app.state::<AppState>();
        assert!(st.launch().is_none());assert!(!st.autosave_now());
        assert_eq!(st.workspace(),root.join("agent-workspace"));assert!(st.workspace().is_dir());
        st.remember_project("fixture.kimo");assert_eq!(st.project_path.lock().unwrap().as_deref(),Some("fixture.kimo"));
        assert_eq!(recovery::load(&root).project_path.as_deref(),Some("fixture.kimo"));
        assert!(st.ledger.call("you","timeline",&json!({"action":"add_track","args":{}})).is_err());
        let _=st.app_tools();
        drop(st);drop(app);std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn updater_key_configuration_requires_nonempty_text() {
        for (config,expected) in [(Value::Null,None),(json!({}),None),(json!({"pubkey":false}),None),(json!({"pubkey":""}),None),(json!({"pubkey":"   "}),None),(json!({"pubkey":" fixture-public-key "}),Some("fixture-public-key"))] {
            let mut context=tauri::test::mock_context(tauri::test::noop_assets());
            context.config_mut().plugins.0.insert("updater".into(),config);
            let app=tauri::test::mock_builder().build(context).unwrap();
            assert_eq!(updater_pubkey(app.handle()).as_deref(),expected);
        }
    }

    #[test]
    fn ipc_rejects_missing_arguments_before_mutating_state_and_redacts_keys() {
        let root=std::env::temp_dir().join(format!("kimo-main-ipc-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);std::fs::create_dir_all(&root).unwrap();
        let app=tauri::test::mock_builder().invoke_handler(command_handler()).manage(state(&root))
            .build(tauri::test::mock_context(tauri::test::noop_assets())).unwrap();
        let webview=tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
        let request=|command:&str,body:Value| tauri::test::get_ipc_response(&webview,tauri::webview::InvokeRequest {
            cmd:command.into(),callback:tauri::ipc::CallbackFn(0),error:tauri::ipc::CallbackFn(1),
            url:"tauri://localhost".parse().unwrap(),body:tauri::ipc::InvokeBody::Json(body),headers:Default::default(),invoke_key:tauri::test::INVOKE_KEY.into(),
        }).map(|response|response.deserialize::<Value>().unwrap());
        let before=request("app_state",json!({})).unwrap();
        for command in ["engine_call","engine_read","change_undo","project_open","project_save","pick","media_thumb","open_url","reveal","cli_signin","cli_install","cli_register","cli_models","agent_select","agent_send","mcp_set","key_set","key_clear","key_test","settings_set","crash_answer","viewer_frame","viewer_draft","preview_start","preview_update","preview_frame","setup_start"] {
            let error=request(command,json!({})).unwrap_err();assert!(error.to_string().contains("invalid args"),"{command}: {error}");
        }
        assert_eq!(request("app_state",json!({})).unwrap(),before);
        assert!(request("unknown-command",json!({})).unwrap_err().to_string().contains("not found"));
        assert_eq!(request("agent_select",json!({"agent":"unknown"})).unwrap_err(),json!("unknown agent"));
        request("agent_select",json!({"agent":"api","select":true})).unwrap();
        request("key_set",json!({"provider":"openai","key":"ipc-fixture-secret","remember":false})).unwrap();
        let after=request("app_state",json!({})).unwrap();assert_eq!(after["settings"]["agent"],"api");assert!(!after.to_string().contains("ipc-fixture-secret"));
        assert_eq!(request("changes_list",json!({})).unwrap(),json!([]));
        assert_eq!(request("recovery_status",json!({})).unwrap(),Value::Null);
        assert_eq!(request("crash_pending",json!({})).unwrap(),json!([]));
        assert_eq!(request("agent_stop",json!({})).unwrap(),false);
        request("agent_reset",json!({})).unwrap();request("onboarded_set",json!({})).unwrap();request("setup_pause",json!({})).unwrap();request("setup_skip",json!({})).unwrap();
        assert!(app.state::<AppState>().cancel_dl.load(Ordering::SeqCst));
        assert_eq!(request("setup_status",json!({})).unwrap()["done"],true);
        assert_eq!(request("update_check",json!({})).unwrap()["state"],"off");
        assert!(request("update_install",json!({})).unwrap_err().to_string().contains("does not update itself"));
        assert!(request("recovery_restore",json!({})).is_err());
        assert!(request("mcp_probe",json!({})).is_err());assert!(request("preview_stop",json!({})).is_err());
        request("recovery_discard",json!({})).unwrap();
        drop(webview);drop(app);std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bridge_dispatch_and_live_events_preserve_source_and_protocol_errors() {
        use tauri::Listener;
        let root=std::env::temp_dir().join(format!("kimo-main-bridge-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);std::fs::create_dir_all(&root).unwrap();
        let app=tauri::test::mock_app();assert!(app.manage(state(&root)));
        let received=Arc::new(Mutex::new(Vec::new()));
        for name in ["engine-event","external-event","agent-event"] {
            let captured=received.clone();app.listen(name,move |e|captured.lock().unwrap().push((name,serde_json::from_str::<Value>(e.payload()).unwrap())));
        }
        let event=json!({"type":"project_changed","rev":7});
        forward(app.handle(),Source::Gui,event.clone());
        forward(app.handle(),Source::External{conn:9},event.clone());
        emit_ev(app.handle(),"codex",3,&Ev::Text{ text:"fixture".into() });
        let received=received.lock().unwrap();
        assert_eq!(received.iter().filter(|(n,v)|*n=="engine-event"&&*v==event).count(),2);
        let external=&received.iter().find(|(n,_)|*n=="external-event").unwrap().1;
        assert_eq!(external["conn"],9);assert_eq!(external["ev"],event);assert!(external["t"].as_u64().unwrap()>0);
        let agent=&received.iter().find(|(n,_)|*n=="agent-event").unwrap().1;
        assert_eq!(agent["agent"],"codex");assert_eq!(agent["turn"],3);assert_eq!(agent["ev"]["text"],"fixture");
        drop(received);
        let handler=bridge_handler(app.handle().clone());
        assert_eq!(handler("fixture","ping",json!({})).unwrap(),json!({}));
        assert_eq!(handler("fixture","tools/list",json!({})).unwrap(),json!({"tools":[]}));
        for (method,key) in [("resources/list","resources"),("resources/templates/list","resourceTemplates"),("prompts/list","prompts")] {
            assert_eq!(handler("fixture",method,json!({})).unwrap()[key],json!([]));
        }
        assert_eq!(handler("fixture","initialize",json!({"protocolVersion":"2025-03-26"})).unwrap()["serverInfo"]["version"],env!("CARGO_PKG_VERSION"));
        assert_eq!(handler("fixture","tools/call",json!({})).unwrap_err().0,-32602);
        assert_eq!(handler("fixture","tools/call",json!({"name":"timeline"})).unwrap_err().0,-32603);
        assert_eq!(handler("fixture","tools/call",json!({"name":"timeline","arguments":{"action":"add_track"}})).unwrap_err().0,-32603);
        assert_eq!(handler("fixture","unknown",json!({})).unwrap_err(),(-32601,"method not found: unknown".into()));
        tauri::async_runtime::block_on(async {
            assert_eq!(mcp_set(app.handle().clone(),false).await.unwrap(),json!({"enabled":false}));
            assert!(mcp_set(app.handle().clone(),true).await.is_err());
            let update=update_check(app.handle().clone()).await.unwrap();assert_eq!(update["state"],"off");assert!(update["checked_at"].as_u64().unwrap()>0);
            assert_eq!(*app.state::<AppState>().update.lock().unwrap(),update);
            assert!(update_install(app.handle().clone()).await.unwrap_err().contains("does not update itself"));
        });
        drop(handler);drop(app);std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn async_commands_reject_invalid_requests_and_release_busy_state() {
        let root=std::env::temp_dir().join(format!("kimo-main-errors-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);std::fs::create_dir_all(&root).unwrap();
        let app=tauri::test::mock_app();assert!(app.manage(state(&root)));
        tauri::async_runtime::block_on(async {
            let h=app.handle();
            assert_eq!(blocking(h.clone(),|_| Ok(7)).await.unwrap(),7);
            assert_eq!(blocking::<_,()>(h.clone(),|_| Err("fixture refusal".into())).await.unwrap_err(),"fixture refusal");
            assert!(blocking::<_,()>(h.clone(),|_| panic!("fixture worker panic")).await.unwrap_err().contains("panic"));
            assert!(engine_read(h.clone(),"timeline".into(),json!({"action":"add_track"})).await.unwrap_err().contains("use engine_call"));
            assert!(engine_read(h.clone(),"project".into(),json!({"action":"get_project"})).await.is_err());
            assert!(engine_call(h.clone(),"timeline".into(),json!({"action":"add_track"})).await.is_err());
            assert!(project_open(h.clone(),root.join("missing.kimo").display().to_string()).await.is_err());
            assert!(project_save(h.clone(),root.join("missing").display().to_string()).await.is_err());
            assert!(recovery_restore(h.clone()).await.unwrap_err().contains("nothing to restore"));
            assert!(key_test(h.clone(),"unknown".into(),None).await.is_err());
            assert!(key_test(h.clone(),"openai".into(),Some(" ".into())).await.unwrap_err().contains("Paste a key"));
            assert!(cli_register(h.clone(),"unknown".into()).await.is_err());
            assert!(cli_models(h.clone(),"unknown".into()).await.is_err());
            assert!(cli_install("unknown".into()).is_err());assert!(cli_signin("unknown".into()).is_err());
            assert!(mcp_probe(h.clone()).await.is_err());
            assert!(agent_send(h.clone(),"  ".into(),1).await.unwrap_err().contains("Type what"));
            app.state::<AppState>().busy.store(true,Ordering::SeqCst);
            assert!(agent_send(h.clone(),"fixture".into(),1).await.unwrap_err().contains("still working"));
            app.state::<AppState>().busy.store(false,Ordering::SeqCst);
            app.state::<AppState>().settings.lock().unwrap().agent="unknown".into();
            assert!(agent_send(h.clone(),"fixture".into(),1).await.unwrap_err().contains("Pick an agent"));
            assert!(!app.state::<AppState>().busy.load(Ordering::SeqCst));
            {let st=app.state::<AppState>();let mut settings=st.settings.lock().unwrap();settings.agent="api".into();settings.provider="unknown".into();}
            assert!(agent_send(h.clone(),"fixture".into(),1).await.unwrap_err().contains("Pick a provider"));
            app.state::<AppState>().settings.lock().unwrap().provider="openai".into();
            assert!(agent_send(h.clone(),"fixture".into(),1).await.unwrap_err().contains("key"));
            assert!(!app.state::<AppState>().busy.load(Ordering::SeqCst));
            assert!(viewer_frame(h.clone(),0.).await.is_err());assert!(viewer_draft(h.clone(),0.,1.).await.is_err());
            assert!(preview_start(h.clone(),0.,None,None,None,None).await.is_err());
            assert!(preview_start(h.clone(),0.,Some(0.25),Some(2.),Some(true),Some(true)).await.is_err());
            assert!(preview_update(h.clone(),0.,None,None,None).await.is_err());
            assert!(preview_update(h.clone(),1.,Some(true),Some(2.),Some(true)).await.is_err());
            assert!(preview_frame(h.clone(),0.).await.is_err());assert!(preview_stop(h.clone()).await.is_err());
        });
        drop(app);std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn real_engine_autosave_tracks_unsaved_edits_and_discard_removes_only_its_snapshot() {
        let root=std::env::temp_dir().join(format!("kimo-main-autosave-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);std::fs::create_dir_all(&root).unwrap();
        let bin=PathBuf::from(std::env::var_os("KIMO_MCP_BIN").expect("build kimo-mcp and set KIMO_MCP_BIN for native command tests"));
        assert!(bin.is_file());
        let mut st=state(&root);
        st.sup=Arc::new(Supervisor::new(Some(bin.clone()),vec![("KIMO_SPECTATOR".into(),"off".into())],None));
        st.ledger=Arc::new(Ledger::new(st.sup.clone(),Arc::new(|name,value| {assert!(!name.is_empty());assert!(value.is_object() || value.is_array());})));
        assert!(matches!(st.sup.start(),Status::Running{..}));
        assert_eq!(st.launch().unwrap().command,bin.display().to_string());
        let app=tauri::test::mock_app();assert!(app.manage(st));
        let st=app.state::<AppState>();
        let added=st.ledger.call("you","timeline",&json!({"action":"add_track","args":{}})).unwrap();
        assert!(!engine::is_error(&added));assert!(!changes_list(app.state()).is_empty());
        assert!(st.autosave_now());
        let record=recovery::load(&root);let snapshot=record.autosave.as_ref().unwrap();
        assert!(Path::new(snapshot).is_file());assert_eq!(record.autosaved_rev,st.ledger.revision());
        assert!(record.autosaved_at>0);
        let project=root.join("fixture.kimo");st.remember_project(project.to_str().unwrap());
        assert_eq!(recovery::load(&root).saved_rev,st.ledger.revision());
        assert!(st.autosave_now());assert!(Path::new(&format!("{}.autosave",project.display())).is_file());
        let snapshot_json=app_state(app.state());assert!(snapshot_json["snippets"].is_object());
        *st.offer.lock().unwrap()=Some(recovery::Offer{autosave:snapshot.clone(),project_path:None,autosaved_at:record.autosaved_at});
        assert_eq!(recovery_status(app.state()).unwrap().autosave,*snapshot);
        recovery_discard(app.state());assert!(recovery_status(app.state()).is_none());assert!(!Path::new(snapshot).exists());
        assert!(Path::new(&format!("{}.autosave",project.display())).exists());
        tauri::async_runtime::block_on(async {
            let h=app.handle();
            let saved=project_save(h.clone(),root.join("native-saved").display().to_string()).await.unwrap();
            assert!(saved.ends_with("native-saved.kimo"));assert!(Path::new(&saved).is_file());
            assert!(project_open(h.clone(),root.join("absent.kimo").display().to_string()).await.is_err());
            project_open(h.clone(),saved.clone()).await.unwrap();
            assert_eq!(st.project_path.lock().unwrap().as_deref(),Some(saved.as_str()));
            let before=engine_read(h.clone(),"project".into(),json!({"action":"get_project","args":{}})).await.unwrap();
            let count=before["data"]["tracks"].as_array().unwrap().len();
            let added=engine_call(h.clone(),"timeline".into(),json!({"action":"add_track","args":{}})).await.unwrap();
            assert_eq!(added["ok"],true);
            let id=changes_list(app.state()).last().unwrap().id;
            change_undo(h.clone(),id).await.unwrap();
            let after=engine_read(h.clone(),"project".into(),json!({"action":"get_project","args":{}})).await.unwrap();
            assert_eq!(after["data"]["tracks"].as_array().unwrap().len(),count);
            assert!(matches!(project_new(h.clone()).await.unwrap(),Status::Running{..}));
            assert!(st.project_path.lock().unwrap().is_none());assert!(changes_list(app.state()).is_empty());
            assert!(viewer_frame(h.clone(),0.).await.unwrap().empty);
            assert!(matches!(engine_restart(h.clone()).await.unwrap(),Status::Running{..}));
            assert_eq!(setup_start(h.clone(),false).await.unwrap()["ready"],true);
            let offered=format!("{}.autosave",project.display());
            *st.offer.lock().unwrap()=Some(recovery::Offer{autosave:offered.clone(),project_path:Some(project.display().to_string()),autosaved_at:123});
            let restored=recovery_restore(h.clone()).await.unwrap();
            assert_eq!(restored,json!({"project_path":project.display().to_string(),"autosaved_at":123}));
            assert!(recovery_status(app.state()).is_none());assert!(Path::new(&offered).is_file());
            assert_eq!(recovery::load(&root).saved_rev,0);
            let recovered=engine_read(h.clone(),"project".into(),json!({"action":"get_project","args":{}})).await.unwrap();
            assert_eq!(recovered["data"]["tracks"].as_array().unwrap().len(),count);
            assert!(mcp_probe(h.clone()).await.unwrap().ok);
            let connected=mcp_set(h.clone(),true).await.unwrap();assert_eq!(connected["enabled"],true);
            let port=connected["external_port"].as_u64().unwrap() as u16;
            assert_eq!(start_external(h).unwrap(),port,"enabling again reuses this listener");
            assert!(st.settings.lock().unwrap().mcp_enabled);assert!(connected["snippets"].is_object());
            assert_eq!(mcp_set(h.clone(),false).await.unwrap(),json!({"enabled":false}));
            assert!(st.external.lock().unwrap().is_none());assert!(!st.settings.lock().unwrap().mcp_enabled);

        });
        st.sup.stop();drop(st);drop(app);std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn setup_reports_installed_unavailable_and_failed_parts_without_claiming_ready() {
        let root=std::env::temp_dir().join(format!("kimo-main-setup-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("installed.bin"),b"fixture").unwrap();
        let installed=Part{id:"fixture-installed".into(),name:"Installed".into(),urls:vec![],sha512:download::sha512_file(&root.join("installed.bin")).unwrap(),size:7,dest:"installed.bin".into(),executable:false,optional:false,note:"local fixture".into(),os:String::new()};
        let unavailable=Part{id:"fixture-unavailable".into(),name:"Unavailable".into(),sha512:String::new(),size:0,dest:"missing.bin".into(),..installed.clone()};
        let optional=Part{id:"fixture-optional".into(),name:"Optional".into(),dest:"optional.bin".into(),optional:true,..installed.clone()};
        let mut st=state(&root);st.parts=vec![installed,unavailable,optional];
        let app=tauri::test::mock_app();assert!(app.manage(st));
        let status=setup_status(app.state());assert_eq!(status["ready"],false);
        assert_eq!(status["parts"][0]["installed"],true);assert_eq!(status["parts"][0]["satisfied"],"downloaded");
        assert_eq!(status["parts"][1]["pinned"],false);assert_eq!(status["parts"][1]["satisfied"],Value::Null);
        tauri::async_runtime::block_on(async {
            let skipped=setup_start(app.handle().clone(),true).await.unwrap();
            assert_eq!(skipped["ready"],false);assert_eq!(skipped["results"].as_array().unwrap().len(),2);
            assert_eq!(skipped["results"][0],json!({"id":"fixture-installed","ok":true,"already":true}));
            assert_eq!(skipped["results"][1],json!({"id":"fixture-unavailable","ok":false,"system":false,"error":"not published for download yet"}));
            let attempted=setup_start(app.handle().clone(),false).await.unwrap();
            assert_eq!(attempted["ready"],false);assert_eq!(attempted["results"].as_array().unwrap().len(),3);
            assert_eq!(attempted["results"][2]["ok"],false);assert!(attempted["results"][2]["error"].as_str().unwrap().contains("no mirror listed"));
        });
        assert!(!app.state::<AppState>().settings.lock().unwrap().setup_done);
        assert!(!root.join("optional.bin").exists());drop(app);std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn agent_busy_guard_releases_on_success_and_unwind() {
        let busy=AtomicBool::new(true);
        { let _guard=scopeguard(&busy); assert!(busy.load(Ordering::SeqCst)); }
        assert!(!busy.load(Ordering::SeqCst));
        busy.store(true,Ordering::SeqCst);
        let failed=std::panic::catch_unwind(|| { let _guard=scopeguard(&busy); panic!("fixture failure"); });
        assert!(failed.is_err());assert!(!busy.load(Ordering::SeqCst));
    }

    #[test]
    fn shaped_results_keep_text_and_error_status() {
        assert_eq!(shaped(json!({"content":[{"type":"text","text":"{\"value\":7}"}]})),json!({"ok":true,"text":"{\"value\":7}","data":{"value":7}}));
        assert_eq!(shaped(json!({"isError":true,"content":[{"type":"text","text":"refused"}]})),json!({"ok":false,"text":"refused","data":null}));
    }
}
