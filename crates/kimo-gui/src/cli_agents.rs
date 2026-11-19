//! Coding agents on the user's OWN subscription: kimo runs the official CLIs the user installed and signed in to
//! (Claude Code with a Claude Pro/Max plan, Codex with a ChatGPT plan, Antigravity with a Google account) as headless
//! child processes, with the kimo engine attached over MCP through `kimo-gui mcp` (which joins the open app's engine).
//! kimo never reads, copies or stores their login tokens: the CLIs authenticate themselves exactly as in a terminal.
//! Sign-in is detected only by whether the CLI's own credential file EXISTS (never opened) or by the CLI's own status
//! command, and signing in happens in a terminal window running the CLI's own login.
//!
//! Each CLI's live output is a JSON-lines stream; the pure `parse_*` functions below turn one line into neutral
//! `Ev`ents (text, tool start/end, session id, final reply, error) so the chat UI is the same for all three.
use crate::agents::{home_dir, which};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind { ClaudeCode, Codex, Antigravity }

impl Kind {
    pub fn all() -> [Kind; 3] { [Kind::ClaudeCode, Kind::Codex, Kind::Antigravity] }
    pub fn id(self) -> &'static str { match self { Kind::ClaudeCode => "claude-code", Kind::Codex => "codex", Kind::Antigravity => "antigravity" } }
    pub fn from_id(s: &str) -> Option<Kind> { Kind::all().into_iter().find(|k| k.id() == s) }
    pub fn name(self) -> &'static str { match self { Kind::ClaudeCode => "Claude Code", Kind::Codex => "Codex", Kind::Antigravity => "Antigravity" } }
    pub fn plan(self) -> &'static str { match self { Kind::ClaudeCode => "Claude Pro or Max plan", Kind::Codex => "ChatGPT Plus, Pro, Business or Enterprise plan", Kind::Antigravity => "Google account" } }
    pub fn bin_name(self) -> &'static str { match self { Kind::ClaudeCode => "claude", Kind::Codex => "codex", Kind::Antigravity => "agy" } }
    /// The terminal command that installs the CLI (npm), or None when it ships as its own installer.
    pub fn install_cmd(self) -> Option<&'static str> {
        match self { Kind::ClaudeCode => Some("npm install -g @anthropic-ai/claude-code"), Kind::Codex => Some("npm install -g @openai/codex"), Kind::Antigravity => None }
    }
    pub fn install_url(self) -> &'static str {
        match self { Kind::ClaudeCode => "https://docs.claude.com/en/docs/claude-code/setup", Kind::Codex => "https://developers.openai.com/codex/cli", Kind::Antigravity => "https://antigravity.google/download" }
    }
    /// What the sign-in terminal runs: the CLI's own login.
    pub fn login_cmd(self) -> &'static str { match self { Kind::ClaudeCode => "claude /login", Kind::Codex => "codex login", Kind::Antigravity => "agy" } }
}

/// Where each CLI usually lives besides PATH (GUI apps on macOS/Linux often start with a short PATH).
fn environment_dir(key: &str) -> Option<PathBuf> { std::env::var_os(key).map(PathBuf::from) }

fn extra_dirs(k: Kind) -> Vec<PathBuf> {
    extra_dirs_for(k, cfg!(windows), home_dir().as_deref(), &environment_dir)
}

fn extra_dirs_for(k: Kind, windows: bool, home: Option<&Path>, env: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<PathBuf> {
    let mut v = vec![];
    if let Some(h) = home {
        v.push(h.join(".local").join("bin"));
        v.push(h.join(".npm-global").join("bin"));
        v.push(h.join(".bun").join("bin"));
        v.push(h.join(".volta").join("bin"));
        if k == Kind::ClaudeCode { v.push(h.join(".claude").join("local")); }
        if k == Kind::Antigravity { v.push(h.join(".antigravity").join("bin")); v.push(h.join(".gemini").join("antigravity-cli").join("bin")); }
    }
    if windows {
        if let Some(a) = env("APPDATA") { v.push(a.join("npm")); }
        if let Some(l) = env("LOCALAPPDATA") {
            v.push(l.join("agy").join("bin"));
            v.push(l.join("Programs").join("Antigravity").join("bin"));
            v.push(l.join("Programs").join("claude"));
        }
    } else {
        v.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    }
    v
}

/// Copies that other apps install and keep up to date, outside PATH: the Claude desktop app's own Claude Code, npm's
/// package folder when its `claude`/`codex` shortcut is missing, and the Codex app's CLI. Newest version first.
fn bundled(k: Kind) -> Vec<PathBuf> {
    bundled_for(k, cfg!(windows), home_dir().as_deref(), &environment_dir)
}

fn bundled_for(k: Kind, windows: bool, home: Option<&Path>, env: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = vec![];
    let exe = |n: &str| if windows { format!("{n}.exe") } else { n.to_string() };
    let versions = |root: PathBuf, bin: &str| version_candidates(root, bin, |path| std::fs::read_dir(path));
    match k {
        Kind::ClaudeCode => {
            // Claude desktop app: <LOCALAPPDATA>/Claude-Data/<profile>/claude-code/<version>/<build>/claude.exe (Windows),
            // ~/Library/Application Support/Claude/claude-code/<version>/claude (macOS), ~/.config/Claude/claude-code (Linux)
            let mut roots = vec![];
            if let Some(l) = env("LOCALAPPDATA") {
                if let Ok(rd) = std::fs::read_dir(l.join("Claude-Data")) { for p in rd.flatten() { roots.push(p.path().join("claude-code")); } }
                roots.push(l.join("AnthropicClaude").join("claude-code"));
            }
            if let Some(a) = env("APPDATA") { roots.push(a.join("Claude").join("claude-code")); }
            if let Some(h) = &home {
                roots.push(h.join("Library").join("Application Support").join("Claude").join("claude-code"));
                roots.push(h.join(".config").join("Claude").join("claude-code"));
                roots.push(h.join(".local").join("share").join("claude").join("versions"));
            }
            for r in roots { v.extend(versions(r, &exe("claude"))); }
            if let Some(a) = env("APPDATA") {
                let pkg = a.join("npm").join("node_modules").join("@anthropic-ai").join("claude-code");
                v.push(pkg.join("node_modules").join("@anthropic-ai").join("claude-code-win32-x64").join("claude.exe"));
                v.push(pkg.join("bin").join("claude.exe"));
            }
        }
        Kind::Codex => {
            if let Some(h) = &home { v.push(h.join(".codex").join("plugins").join(".plugin-appserver").join(exe("codex"))); v.push(h.join(".codex").join("bin").join(exe("codex"))); }
            if let Some(l) = env("LOCALAPPDATA") { v.push(l.join("Programs").join("Codex").join("resources").join(exe("codex"))); v.push(l.join("Programs").join("codex").join("resources").join(exe("codex"))); }
            v.push(PathBuf::from("/Applications/Codex.app/Contents/Resources/codex"));
        }
        Kind::Antigravity => {
            if let Some(h) = &home { v.push(h.join(".gemini").join("antigravity-cli").join("bin").join(exe("agy"))); }
        }
    }
    v.into_iter().filter(|p| p.is_file()).collect()
}

/// Candidate layouts remain usable when a version directory disappears or cannot be listed.
fn version_candidates(root: PathBuf, bin: &str, read: fn(&Path) -> std::io::Result<std::fs::ReadDir>) -> Vec<PathBuf> {
    let mut ds: Vec<_> = read(&root).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    ds.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    ds.reverse();
    let mut out = vec![];
    for d in ds {
        out.push(d.join(bin));
        if let Ok(sub) = read(&d) { for s in sub.flatten() { out.push(s.path().join(bin)); } }
    }
    out
}

/// A copy counts only when it actually runs (`--version` answers): a half-finished install is skipped, not shown as
/// installed. Results are remembered for the session; a copy that disappears is looked for again.
fn works(p: &Path) -> bool { run_short(p, &["--version"], 15).map(|(ok, _)| ok).unwrap_or(false) }

pub fn find(k: Kind) -> Option<PathBuf> {
    static CACHE: std::sync::OnceLock<Mutex<std::collections::HashMap<&'static str, PathBuf>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Some(p) = cache.lock().unwrap().get(k.id()).cloned() { if p.is_file() { return Some(p); } }
    let mut cands: Vec<PathBuf> = vec![];
    if let Some(p) = std::env::var_os(format!("KIMO_{}_BIN", k.bin_name().to_ascii_uppercase())).map(PathBuf::from).filter(|p| p.is_file()) { cands.push(p); }
    if let Some(p) = which(k.bin_name()) { cands.push(p); }
    if let Some(p) = extra_candidate(k, &extra_dirs(k), cfg!(windows)) { cands.push(p); }
    cands.extend(bundled(k));
    let mut seen = std::collections::HashSet::new();
    let found = cands.into_iter().filter(|p| seen.insert(p.clone())).find(|p| works(p))?;
    cache.lock().unwrap().insert(k.id(), found.clone());
    Some(found)
}

fn extra_candidate(k: Kind, dirs: &[PathBuf], windows: bool) -> Option<PathBuf> {
    let dirs: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
    let sep = if windows { ";" } else { ":" };
    let pathext = if windows { Some(".EXE;.CMD;.BAT") } else { None };
    crate::agents::which_in(k.bin_name(), &dirs.join(sep), pathext)
}

/// A child with no console window on Windows (the app is a GUI; a flashing console would be alarming).
fn quiet(c: &mut Command) -> &mut Command {
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); }
    c
}

/// Run `bin args` with a timeout; Some((exit ok, stdout+stderr)).
fn run_short(bin: &Path, args: &[&str], secs: u64) -> Option<(bool, String)> {
    let mut c = Command::new(bin);
    quiet(&mut c).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().ok()?;
    // drain both pipes while it runs: a big answer (a model catalog) would otherwise fill the pipe and stall the child
    let drain = |mut pipe: Box<dyn Read + Send>| std::thread::spawn(move || { let mut s = String::new(); let _ = pipe.read_to_string(&mut s); s });
    let ho = drain(Box::new(child.stdout.take().expect("stdout was configured as piped")));
    let he = drain(Box::new(child.stderr.take().expect("stderr was configured as piped")));
    let t0 = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let mut out = ho.join().unwrap_or_default();
                out.push_str(&he.join().unwrap_or_default());
                return Some((st.success(), out));
            }
            Ok(None) if t0.elapsed() < Duration::from_secs(secs) => std::thread::sleep(Duration::from_millis(50)),
            _ => { let _ = child.kill(); let _ = child.wait(); return None; }
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct Status {
    pub id: &'static str,
    pub name: &'static str,
    pub plan: &'static str,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// Some(true): signed in. Some(false): definitely not. None: cannot tell without running it (Test checks for real).
    pub signed_in: Option<bool>,
    pub sign_in_note: String,
    pub install_cmd: Option<&'static str>,
    pub install_url: &'static str,
    pub login_cmd: &'static str,
    /// kimo is registered in the CLI's own MCP config (so the user's own sessions can use it too).
    pub registered: bool,
}

fn first_line(s: &str) -> Option<String> { s.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(80).collect()) }

/// Read-only: never opens a credential file, only checks that it exists.
pub fn status(k: Kind) -> Status {
    status_for_platform(k, cfg!(target_os = "macos"))
}
fn status_for_platform(k: Kind, macos: bool) -> Status {
    let path = find(k);
    let version = path.as_ref().and_then(|p| run_short(p, &["--version"], 8)).and_then(|(_, o)| first_line(&o));
    let home = home_dir();
    let exists = |rel: &[&str]| home.as_ref().map(|h| rel.iter().fold(h.clone(), |p, s| p.join(s)).is_file()).unwrap_or(false);
    let (signed_in, note) = match k {
        Kind::ClaudeCode => {
            if std::env::var_os("ANTHROPIC_API_KEY").is_some() { (Some(true), "Signed in through ANTHROPIC_API_KEY in your environment (that bills the API, not your plan).".to_string()) }
            else if exists(&[".claude", ".credentials.json"]) { (Some(true), "Signed in (Claude Code's own login).".into()) }
            else if macos && path.is_some() { (None, "Claude Code keeps its login in the macOS Keychain; press Test to check.".into()) }
            else { (Some(false), "Not signed in yet. Sign in once with your Claude plan.".into()) }
        }
        Kind::Codex => match path.as_ref().and_then(|p| run_short(p, &["login", "status"], 10)) {
            Some((true, out)) => (Some(true), first_line(&out).unwrap_or_else(|| "Signed in".into())),
            Some((false, out)) => (Some(false), first_line(&out).unwrap_or_else(|| "Not signed in".into())),
            None if exists(&[".codex", "auth.json"]) => (Some(true), "Signed in (Codex's own login).".into()),
            None => (Some(false), "Not signed in yet. Sign in once with ChatGPT.".into()),
        },
        Kind::Antigravity => {
            if exists(&[".gemini", "oauth_creds.json"]) || exists(&[".gemini", "google_accounts.json"]) { (Some(true), "Signed in with your Google account.".into()) }
            else { (Some(false), "Not signed in yet. Open Antigravity once and sign in with Google.".into()) }
        }
    };
    Status {
        id: k.id(), name: k.name(), plan: k.plan(), installed: path.is_some(), path: path.map(|p| p.display().to_string()), version,
        signed_in: if path_missing(&k) { Some(false) } else { signed_in }, sign_in_note: note,
        install_cmd: k.install_cmd(), install_url: k.install_url(), login_cmd: k.login_cmd(), registered: registered(k),
    }
}
fn path_missing(k: &Kind) -> bool { find(*k).is_none() }

/// Is a `kimo` server in the CLI's user-level MCP config? (read-only, text match on the server name)
pub fn registered(k: Kind) -> bool {
    let Some(h) = home_dir() else { return false };
    match k {
        Kind::ClaudeCode => std::fs::read_to_string(h.join(".claude.json")).ok().and_then(|s| crate::agents::claude_entry(&s)).is_some(),
        Kind::Codex => std::fs::read_to_string(crate::agents::codex_config_path(&h)).ok().and_then(|s| crate::agents::codex_entry(&s)).is_some(),
        Kind::Antigravity => [h.join(".gemini").join("config").join("mcp_config.json"), h.join(".gemini").join("antigravity").join("mcp_config.json"), h.join(".gemini").join("antigravity-cli").join("mcp_config.json"), h.join(".gemini").join("settings.json")]
            .iter().filter_map(|p| std::fs::read_to_string(p).ok()).any(|s| serde_json::from_str::<Value>(&s).ok().and_then(|v| v.get("mcpServers").and_then(|m| m.get("kimo")).cloned()).is_some()),
    }
}

/// The launch every agent uses for kimo: this app's own binary in `mcp` mode (joins the open app, else standalone).
#[derive(Clone, Debug)]
pub struct KimoLaunch { pub command: String, pub env: Vec<(String, String)> }
impl KimoLaunch {
    pub fn args(&self, k: Kind) -> Vec<String> { vec!["mcp".into(), "--client".into(), k.id().into()] }
}

/// Register kimo in the CLI's own user-level MCP config, using the CLI's own `mcp add` (argv, no shell).
pub fn register(k: Kind, bin: &Path, l: &KimoLaunch) -> Result<String, String> {
    let mut args: Vec<String> = match k {
        Kind::ClaudeCode => {
            let mut o = serde_json::Map::new();
            o.insert("type".into(), json!("stdio")); o.insert("command".into(), json!(l.command)); o.insert("args".into(), json!(l.args(k)));
            if !l.env.is_empty() { o.insert("env".into(), json!(l.env.iter().cloned().collect::<std::collections::BTreeMap<_, _>>())); }
            return run_register(bin, &["mcp".into(), "add-json".into(), "--scope".into(), "user".into(), "kimo".into(), Value::Object(o).to_string()]);
        }
        Kind::Codex => vec!["mcp".into(), "add".into(), "kimo".into()],
        Kind::Antigravity => vec!["mcp".into(), "add".into()],
    };
    for (key, v) in &l.env { args.push("--env".into()); args.push(format!("{key}={v}")); }
    if k == Kind::Antigravity { args.push("kimo".into()); }
    args.push("--".into());
    args.push(l.command.clone());
    args.extend(l.args(k));
    run_register(bin, &args)
}
fn run_register(bin: &Path, args: &[String]) -> Result<String, String> {
    let mut c = Command::new(bin);
    let out = quiet(&mut c).args(args).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).trim().to_string();
    if out.status.success() { Ok(if text.is_empty() { "Added kimo.".into() } else { text }) } else { Err(if text.is_empty() { format!("exited with {}", out.status) } else { text }) }
}

/// A launch request, kept separate from process creation so platform quoting can be checked without opening user apps.
struct DesktopCommand {
    program: PathBuf,
    args: Vec<std::ffi::OsString>,
    raw: Option<String>,
    quiet: bool,
}

fn desktop_command(program: &str, args: &[&str]) -> DesktopCommand {
    DesktopCommand { program: program.into(), args: args.iter().map(|arg| std::ffi::OsString::from(*arg)).collect(), raw: None, quiet: false }
}

fn spawn_desktop_command(plan: DesktopCommand) -> Result<Child, String> {
    let mut command = Command::new(plan.program);
    command.args(plan.args);
    if plan.quiet { quiet(&mut command); }
    #[cfg(windows)]
    if let Some(raw) = plan.raw { use std::os::windows::process::CommandExt; command.raw_arg(raw); }
    #[cfg(not(windows))]
    let _ = plan.raw;
    command.spawn().map_err(|error| error.to_string())
}

fn terminal_command_for(os: &str, cmdline: &str, find: &dyn Fn(&str) -> Option<PathBuf>) -> Result<DesktopCommand, String> {
    match os {
        "windows" => {
            let mut plan = desktop_command("cmd", &[]);
            plan.raw = Some(format!("/c start \"kimo\" cmd /k \"{cmdline}\""));
            Ok(plan)
        }
        "macos" => {
            let script = format!("tell application \"Terminal\" to do script \"{}\"\ntell application \"Terminal\" to activate", cmdline.replace('\\', "\\\\").replace('"', "\\\""));
            Ok(desktop_command("osascript", &["-e", &script]))
        }
        _ => {
            let hold = format!("{cmdline}; echo; read -p 'Press Enter to close'");
            for (terminal, pre) in [("x-terminal-emulator", vec!["-e"]), ("gnome-terminal", vec!["--"]), ("konsole", vec!["-e"]), ("xfce4-terminal", vec!["-x"]), ("kitty", vec![]), ("alacritty", vec!["-e"]), ("xterm", vec!["-e"])] {
                if find(terminal).is_some() {
                    let mut plan = desktop_command(terminal, &pre);
                    plan.args.extend(["sh", "-c", &hold].map(std::ffi::OsString::from));
                    return Ok(plan);
                }
            }
            Err("No terminal app found. Run this yourself: ".to_string() + cmdline)
        }
    }
}

/// Open a visible terminal running `cmdline` (sign in / install). The user sees and drives it.
pub fn open_terminal(cmdline: &str) -> Result<(), String> {
    terminal_command_for(std::env::consts::OS, cmdline, &which).and_then(spawn_desktop_command).map(drop)
}

fn url_command_for(os: &str, url: &str) -> Result<DesktopCommand, String> {
    if !url.starts_with("https://") { return Err("only https links".into()); }
    Ok(match os {
        "windows" => { let mut plan = desktop_command("rundll32", &["url.dll,FileProtocolHandler", url]); plan.quiet = true; plan }
        "macos" => desktop_command("open", &[url]),
        _ => desktop_command("xdg-open", &[url]),
    })
}

/// Open an https URL in the user's browser.
pub fn open_url(url: &str) -> Result<(), String> {
    url_command_for(std::env::consts::OS, url).and_then(spawn_desktop_command).map(drop)
}

fn reveal_command_for(os: &str, path: &Path) -> DesktopCommand {
    match os {
        "windows" => { let mut plan = desktop_command("explorer", &[]); plan.raw = Some(format!("/select,\"{}\"", path.display())); plan }
        "macos" => { let mut plan = desktop_command("open", &["-R"]); plan.args.push(path.as_os_str().to_owned()); plan }
        _ => { let mut plan = desktop_command("xdg-open", &[]); plan.args.push(path.parent().unwrap_or(path).as_os_str().to_owned()); plan }
    }
}

/// Show a file in the system file manager.
pub fn reveal(path: &Path) -> Result<(), String> {
    spawn_desktop_command(reveal_command_for(std::env::consts::OS, path)).map(drop)
}

// ------------------------------------------------------------------------------------------------------------------
// One turn

/// Neutral events the chat UI renders, whichever CLI produced them.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ev {
    Session { id: String },
    /// Streaming text (append).
    Delta { text: String },
    /// A whole text block (replace the streamed block with it).
    Text { text: String },
    Thinking { text: String },
    ToolStart { id: String, tool: String, input: Value },
    ToolEnd { id: String, ok: bool, output: String },
    Status { text: String },
    Done { text: String, cost_usd: Option<f64>, turns: Option<u64> },
    Error { message: String },
}

pub const SYSTEM: &str = "You are the editing agent inside kimo, an AI video editor app open on the user's computer. \
The user is watching their project live: every change you make shows up in their timeline and viewer at once and can be \
undone there. Edit ONLY through the kimo MCP tools (server kimo): each tool is a group taking an object with an action name and its args. \
Start with project.timeline_summary (or project.get_project) to see the edit, and use search_tools / kimo_describe when you \
are unsure of an action or its arguments. Make exactly the change asked for, the smallest edit that does it. Do not read, \
write or run anything on the computer other than kimo tools, and do not export or delete files unless asked. When done, \
reply in one or two plain sentences saying what you changed. kimo shows you every edit: each call that changes the project returns `observe` (what it touched, before/after frames as an image, brightness numbers, and findings: mistakes with severity and the exact fix call). Read it after every edit and fix any error or warning before moving on. For anything that must hold, pass expect: [...] on the edit (visible/hidden at a time, inside title_safe, luma range, no_black_frames, readable, max_findings); if it fails the edit is rolled back and you are told why. Run review before export. Use observe: \"full\" for a sharper look, or render_frame/contact_sheet for any moment you want to see.";

/// Display name of a tool: `mcp__kimo__timeline` → `timeline`.
pub fn short_tool(name: &str) -> String {
    name.strip_prefix("mcp__kimo__").or_else(|| name.strip_prefix("kimo__")).or_else(|| name.strip_prefix("kimo.")).unwrap_or(name).to_string()
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(|b| b.get("text").and_then(|t| t.as_str()).map(String::from).unwrap_or_else(|| b.as_str().map(String::from).unwrap_or_default())).collect::<Vec<_>>().join("\n"),
        Value::Null => String::new(),
        o => o.get("text").and_then(|t| t.as_str()).map(String::from).unwrap_or_else(|| o.to_string()),
    }
}
fn cut(s: &str, n: usize) -> String { if s.chars().count() > n { format!("{}…", s.chars().take(n).collect::<String>()) } else { s.to_string() } }

/// Claude Code `-p --output-format stream-json --verbose --include-partial-messages`.
pub fn parse_claude(v: &Value) -> Vec<Ev> {
    let mut out = vec![];
    match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        "system" if v["subtype"] == "init" => {
            if let Some(id) = v["session_id"].as_str() { out.push(Ev::Session { id: id.into() }); }
            if let Some(servers) = v["mcp_servers"].as_array() {
                for s in servers.iter().filter(|s| s["name"] == "kimo") {
                    let st = s["status"].as_str().unwrap_or("");
                    if st != "connected" { out.push(Ev::Error { message: format!("Claude Code could not start the kimo tools ({st}). Restart kimo and try again.") }); }
                }
            }
        }
        "stream_event" => {
            let e = &v["event"];
            if e["type"] == "content_block_delta" {
                if let Some(t) = e["delta"]["text"].as_str() { out.push(Ev::Delta { text: t.into() }); }
                if let Some(t) = e["delta"]["thinking"].as_str() { out.push(Ev::Thinking { text: t.into() }); }
            }
        }
        "assistant" => for b in v["message"]["content"].as_array().into_iter().flatten() {
            match b["type"].as_str() {
                Some("text") => out.push(Ev::Text { text: b["text"].as_str().unwrap_or("").into() }),
                Some("tool_use") => out.push(Ev::ToolStart { id: b["id"].as_str().unwrap_or("").into(), tool: short_tool(b["name"].as_str().unwrap_or("")), input: b["input"].clone() }),
                _ => {}
            }
        },
        "user" => for b in v["message"]["content"].as_array().into_iter().flatten() {
            if b["type"] == "tool_result" {
                out.push(Ev::ToolEnd { id: b["tool_use_id"].as_str().unwrap_or("").into(), ok: !b["is_error"].as_bool().unwrap_or(false), output: cut(&text_of(&b["content"]), 600) });
            }
        },
        "result" => {
            if let Some(id) = v["session_id"].as_str() { out.push(Ev::Session { id: id.into() }); }
            let text = v["result"].as_str().unwrap_or("").to_string();
            if v["is_error"].as_bool().unwrap_or(false) || v["subtype"].as_str().map(|s| s.starts_with("error")).unwrap_or(false) {
                out.push(Ev::Error { message: if text.is_empty() { format!("Claude Code stopped: {}", v["subtype"].as_str().unwrap_or("error")) } else { text } });
            } else {
                out.push(Ev::Done { text, cost_usd: v["total_cost_usd"].as_f64(), turns: v["num_turns"].as_u64() });
            }
        }
        _ => {}
    }
    out
}

/// Codex `exec --json` (thread/turn/item events).
pub fn parse_codex(v: &Value) -> Vec<Ev> {
    let mut out = vec![];
    let item = &v["item"];
    let id = item["id"].as_str().unwrap_or("").to_string();
    match v["type"].as_str().unwrap_or("") {
        "thread.started" => if let Some(t) = v["thread_id"].as_str() { out.push(Ev::Session { id: t.into() }); },
        "item.started" | "item.updated" if item["type"] == "mcp_tool_call" && v["type"] == "item.started" => {
            out.push(Ev::ToolStart { id, tool: short_tool(item["tool"].as_str().unwrap_or("")), input: item["arguments"].clone() });
        }
        "item.started" if item["type"] == "command_execution" => {
            out.push(Ev::ToolStart { id, tool: "shell".into(), input: json!({"command": item["command"]}) });
        }
        "item.completed" => match item["type"].as_str().unwrap_or("") {
            "agent_message" => out.push(Ev::Text { text: item["text"].as_str().unwrap_or("").into() }),
            "reasoning" => out.push(Ev::Thinking { text: item["text"].as_str().unwrap_or("").into() }),
            "mcp_tool_call" => {
                let failed = item["status"] == "failed" || !item["error"].is_null();
                let output = if !item["error"].is_null() { text_of(&item["error"]["message"]) } else { text_of(&item["result"]["content"]) };
                out.push(Ev::ToolEnd { id, ok: !failed, output: cut(&output, 600) });
            }
            "command_execution" => out.push(Ev::ToolEnd { id, ok: item["exit_code"].as_i64() == Some(0), output: cut(item["aggregated_output"].as_str().unwrap_or(""), 600) }),
            "error" => out.push(Ev::Error { message: item["message"].as_str().unwrap_or("error").into() }),
            _ => {}
        },
        "turn.failed" => out.push(Ev::Error { message: v["error"]["message"].as_str().unwrap_or("Codex stopped with an error").into() }),
        "error" => out.push(Ev::Error { message: v["message"].as_str().unwrap_or("Codex error").into() }),
        "turn.completed" => out.push(Ev::Status { text: "turn complete".into() }),
        _ => {}
    }
    out
}

/// Antigravity CLI `-p --output-format stream-json` (init / step_update / result).
pub fn parse_agy(v: &Value) -> Vec<Ev> {
    let mut out = vec![];
    match v["event"].as_str().unwrap_or("") {
        "init" => if let Some(id) = v["conversation_id"].as_str() { out.push(Ev::Session { id: id.into() }); },
        "step_update" => {
            let s = &v["step_update"];
            let ty = s["step_type"].as_str().unwrap_or("");
            let state = s["state"].as_str().unwrap_or("");
            let id = format!("s{}", s["step_index"].as_u64().unwrap_or(0));
            match ty {
                "user_input" => {}
                "agent_response" => { if let Some(t) = s["text_delta"].as_str() { if !t.is_empty() { out.push(Ev::Delta { text: t.into() }); } } }
                "thinking" | "planner_thinking" => { if let Some(t) = s["text_delta"].as_str() { out.push(Ev::Thinking { text: t.into() }); } }
                "tool" => {
                    // agy 1.4+: {"step_type":"tool","tool_info":{"name","parameters","output"|"error"}}; MCP calls are
                    // name=call_mcp_tool with parameters {ServerName, ToolName, Arguments}
                    let ti = &s["tool_info"];
                    let name = ti["name"].as_str().or(s["tool_name"].as_str()).unwrap_or("tool");
                    let p = &ti["parameters"];
                    let (tool, input) = if name == "call_mcp_tool" {
                        let server = p["ServerName"].as_str().unwrap_or("");
                        let t = short_tool(p["ToolName"].as_str().unwrap_or("mcp"));
                        (if server.is_empty() || server == "kimo" { t } else { format!("{server}.{t}") }, p["Arguments"].clone())
                    } else { (name.to_string(), p.clone()) };
                    match state {
                        "ACTIVE" | "RUNNING" | "PENDING" => out.push(Ev::ToolStart { id, tool, input }),
                        "DONE" => {
                            out.push(Ev::ToolStart { id: id.clone(), tool, input });
                            out.push(Ev::ToolEnd { id, ok: true, output: cut(&text_of(&ti["output"]), 600) });
                        }
                        "ERROR" | "FAILED" | "CANCELED" | "CANCELLED" => {
                            out.push(Ev::ToolStart { id: id.clone(), tool, input });
                            let msg = ti["error"]["message"].as_str().map(String::from).unwrap_or_else(|| text_of(&ti["error"]));
                            out.push(Ev::ToolEnd { id, ok: false, output: cut(&msg, 600) });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        "result" => {
            let r = &v["result"];
            if let Some(id) = r["conversation_id"].as_str() { out.push(Ev::Session { id: id.into() }); }
            let text = r["response"].as_str().unwrap_or("").trim_end().to_string();
            if r["status"] == "SUCCESS" { out.push(Ev::Done { text, cost_usd: None, turns: r["num_turns"].as_u64() }); }
            else { out.push(Ev::Error { message: if text.is_empty() { format!("Antigravity stopped: {}", r["status"].as_str().unwrap_or("error")) } else { text } }); }
        }
        "error" => out.push(Ev::Error { message: v["error"]["message"].as_str().or(v["message"].as_str()).unwrap_or("Antigravity error").into() }),
        _ => {}
    }
    out
}

pub struct Turn<'a> {
    pub kind: Kind,
    pub bin: &'a Path,
    pub prompt: &'a str,
    pub session: Option<&'a str>,
    pub model: Option<&'a str>,
    /// Reasoning effort (low … max), or None for the CLI's own default.
    pub effort: Option<&'a str>,
    pub launch: &'a KimoLaunch,
    /// The agent's working directory: an empty kimo workspace folder, never the user's files.
    pub workdir: &'a Path,
}

/// The argv for one turn, plus what goes on stdin (the prompt is piped, never put on a command line, except for
/// Antigravity, whose print mode takes it as an argument).
pub fn command(t: &Turn) -> Result<(Vec<String>, Option<String>), String> {
    let first = t.session.is_none();
    let prompt = if first && t.kind != Kind::ClaudeCode { format!("{SYSTEM}\n\nUser request: {}", t.prompt) } else { t.prompt.to_string() };
    let mut a: Vec<String> = vec![];
    match t.kind {
        Kind::ClaudeCode => {
            let cfg = t.workdir.join(".kimo-mcp.json");
            let mut server = serde_json::Map::new();
            server.insert("type".into(), json!("stdio")); server.insert("command".into(), json!(t.launch.command)); server.insert("args".into(), json!(t.launch.args(t.kind)));
            if !t.launch.env.is_empty() { server.insert("env".into(), json!(t.launch.env.iter().cloned().collect::<std::collections::BTreeMap<_, _>>())); }
            std::fs::write(&cfg, json!({"mcpServers": {"kimo": server}}).to_string()).map_err(|e| format!("could not write {}: {e}", cfg.display()))?;
            a.extend(["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages", "--strict-mcp-config", "--mcp-config"].map(String::from));
            a.push(cfg.display().to_string());
            a.extend(["--allowedTools", "mcp__kimo", "--disallowedTools", "Bash,Edit,Write,MultiEdit,NotebookEdit,WebFetch,WebSearch,Task", "--append-system-prompt", SYSTEM].map(String::from));
            if let Some(s) = t.session { a.push("--resume".into()); a.push(s.into()); }
            if let Some(m) = t.model.filter(|m| !m.is_empty()) { a.push("--model".into()); a.push(m.into()); }
            if let Some(e) = t.effort.filter(|e| !e.is_empty()) { a.push("--effort".into()); a.push(e.into()); }
            Ok((a, Some(prompt)))
        }
        Kind::Codex => {
            a.push("exec".into());
            if let Some(s) = t.session { a.push("resume".into()); a.push(s.into()); }
            a.extend(["--json", "--skip-git-repo-check"].map(String::from));
            if t.session.is_none() { a.extend(["--sandbox".into(), "read-only".into(), "-C".into(), t.workdir.display().to_string()]); }
            // TOML literal strings ('...') keep Windows backslashes as they are
            a.push("-c".into()); a.push(format!("mcp_servers.kimo.command='{}'", t.launch.command));
            a.push("-c".into()); a.push(format!("mcp_servers.kimo.args=[{}]", t.launch.args(t.kind).iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(",")));
            for (k, v) in &t.launch.env { a.push("-c".into()); a.push(format!("mcp_servers.kimo.env.{k}='{v}'")); }
            a.extend(["-c", "mcp_servers.kimo.startup_timeout_sec=30", "-c", "mcp_servers.kimo.tool_timeout_sec=900", "-c", "approval_policy=\"never\""].map(String::from));
            if let Some(m) = t.model.filter(|m| !m.is_empty()) { a.push("-m".into()); a.push(m.into()); }
            if let Some(e) = t.effort.filter(|e| !e.is_empty()) { a.push("-c".into()); a.push(format!("model_reasoning_effort=\"{e}\"")); }
            a.push("-".into()); // prompt from stdin
            Ok((a, Some(prompt)))
        }
        Kind::Antigravity => {
            a.extend(["-p".into(), prompt, "--output-format".into(), "stream-json".into(), "--disable-slash-commands".into()]);
            if let Some(s) = t.session { a.push("--conversation".into()); a.push(s.into()); }
            if let Some(m) = t.model.filter(|m| !m.is_empty()) { a.push("--model".into()); a.push(m.into()); }
            if let Some(e) = t.effort.filter(|e| !e.is_empty()) { a.push("--effort".into()); a.push(e.into()); }
            Ok((a, None))
        }
    }
}

/// The running child of the current turn, so Stop can end it.
pub type Slot = Arc<Mutex<Option<Child>>>;

pub struct Outcome { pub session: Option<String>, pub reply: String, pub error: Option<String> }

/// Run one turn to the end, streaming events. Blocking; call it on a worker thread.
pub fn run(t: &Turn, slot: &Slot, emit: &mut dyn FnMut(Ev)) -> Outcome {
    let (args, stdin) = match command(t) { Ok(x) => x, Err(e) => return Outcome { session: None, reply: String::new(), error: Some(e) } };
    let mut c = Command::new(t.bin);
    quiet(&mut c).args(&args).current_dir(t.workdir).stdout(Stdio::piped()).stderr(Stdio::piped())
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .env("NO_COLOR", "1").env("CI", "1");
    let mut child = match c.spawn() { Ok(ch) => ch, Err(e) => return Outcome { session: None, reply: String::new(), error: Some(format!("Could not start {}: {e}", t.kind.name())) } };
    if let (Some(p), Some(mut w)) = (stdin, child.stdin.take()) { let _ = w.write_all(p.as_bytes()); drop(w); }
    // Both pipes belong to this successful spawn; an absent pipe is an internal invariant violation.
    let stdout = child.stdout.take().expect("stdout was configured as piped");
    let stderr = child.stderr.take().expect("stderr was configured as piped");
    *slot.lock().unwrap() = Some(child);
    let err_buf = Arc::new(Mutex::new(String::new()));
    let eb = err_buf.clone();
    let et = std::thread::spawn(move || { for l in BufReader::new(stderr).lines().map_while(Result::ok) { let mut b = eb.lock().unwrap(); if b.len() < 8000 { b.push_str(&l); b.push('\n'); } } });
    let mut o = Outcome { session: t.session.map(String::from), reply: String::new(), error: None };
    let mut streamed = String::new();
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { continue };
            let evs = match t.kind { Kind::ClaudeCode => parse_claude(&v), Kind::Codex => parse_codex(&v), Kind::Antigravity => parse_agy(&v) };
            for ev in evs {
                match &ev {
                    Ev::Session { id } => o.session = Some(id.clone()),
                    Ev::Delta { text } => streamed.push_str(text),
                    Ev::Text { text } => { o.reply = text.clone(); streamed.clear(); }
                    Ev::Done { text, .. } => if !text.is_empty() { o.reply = text.clone(); } else if o.reply.is_empty() { o.reply = streamed.clone(); },
                    Ev::Error { message } => o.error = Some(message.clone()),
                    _ => {}
                }
                emit(ev);
            }
    }
    let status = slot.lock().unwrap().take().map(|mut ch| ch.wait());
    let _ = et.join();
    if o.reply.is_empty() { o.reply = streamed; }
    let killed = status.is_none();
    if o.error.is_none() {
        match status {
            Some(Ok(st)) if !st.success() && o.reply.is_empty() => {
                let e = err_buf.lock().unwrap().clone();
                o.error = Some(explain(t.kind, &e).unwrap_or_else(|| format!("{} exited with {st}. {}", t.kind.name(), cut(e.trim(), 400))));
            }
            None if killed => o.error = Some("Stopped.".into()),
            _ => {}
        }
    }
    o
}

/// Turn common CLI failures into a plain next step.
pub fn explain(k: Kind, stderr: &str) -> Option<String> {
    let s = stderr.to_ascii_lowercase();
    if s.contains("login") || s.contains("log in") || s.contains("not logged") || s.contains("unauthorized") || s.contains("401") || s.contains("authenticat") || s.contains("sign in") {
        return Some(format!("{} is not signed in. Open Connections and press Sign in, then try again.", k.name()));
    }
    if s.contains("rate limit") || s.contains("usage limit") || s.contains("quota") || s.contains("429") {
        return Some(format!("Your {} plan's usage limit was reached. Try again later or switch agents.", k.name()));
    }
    None
}

// ------------------------------------------------------------------------------------------------------------------
// Models

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ModelOpt {
    /// What goes to the CLI (`--model` / `-m`).
    pub id: String,
    pub label: String,
    /// Reasoning-effort levels this model takes (empty = the agent's general list).
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Models {
    pub models: Vec<ModelOpt>,
    /// Effort levels the CLI accepts.
    pub efforts: Vec<String>,
    /// Where the list came from, or why it is short.
    pub source: String,
}

const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// `agy models`: one `id<TAB>Label` per line (stderr's "Fetching…" line has no tab).
pub fn parse_agy_models(text: &str) -> Vec<ModelOpt> {
    text.lines().filter_map(|l| {
        let (id, label) = l.trim().split_once('\t')?;
        let id = id.trim();
        (!id.is_empty() && !id.contains(' ')).then(|| ModelOpt { id: id.into(), label: label.trim().into(), efforts: vec![], default_effort: None })
    }).collect()
}

/// `codex debug models`: {"models":[{slug, display_name, visibility, supported_reasoning_levels:[{effort}], default_reasoning_level}]}.
/// Only models Codex itself lists in its picker (visibility "list").
pub fn parse_codex_models(text: &str) -> Vec<ModelOpt> {
    let Some(i) = text.find('{') else { return vec![] };
    let Some(Ok(v)) = serde_json::Deserializer::from_str(&text[i..]).into_iter::<Value>().next() else { return vec![] };
    v["models"].as_array().into_iter().flatten().filter(|m| m["visibility"].as_str().map(|s| s == "list").unwrap_or(true)).filter_map(|m| {
        let id = m["slug"].as_str()?.to_string();
        let efforts = m["supported_reasoning_levels"].as_array().map(|a| a.iter().filter_map(|e| e["effort"].as_str().or(e.as_str()).map(String::from)).collect()).unwrap_or_default();
        Some(ModelOpt { label: m["display_name"].as_str().unwrap_or(&id).to_string(), id, efforts, default_effort: m["default_reasoning_level"].as_str().map(String::from) })
    }).collect()
}

#[cfg(test)]
mod model_catalog_edge_tests {
    use super::*;
    #[test]
    fn incomplete_catalog_entries_are_not_offered_as_models() {
        assert!(parse_codex_models("prefix {broken").is_empty());
        assert!(parse_codex_models("{}").is_empty());
        let models = parse_codex_models(r#"{"models":[{}, {"slug":7}, {"slug":"fallback","supported_reasoning_levels":["low",{}, {"effort":"high"}]}, {"slug":"shown","visibility":"list"}]} trailing diagnostic"#);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "fallback"); assert_eq!(models[0].label, "fallback");
        assert_eq!(models[0].efforts, vec!["low", "high"]); assert_eq!(models[0].default_effort, None);
        assert_eq!(models[1].id, "shown"); assert!(models[1].efforts.is_empty());
        let models = parse_agy_models("\tlabel without id\nnot an id\tLabel\n valid-id\t Label \n");
        assert_eq!(models.len(), 1); assert_eq!(models[0].id, "valid-id"); assert_eq!(models[0].label, "Label");
    }
}

/// The models a signed-in CLI offers. Claude Code has no list command, so its documented aliases are offered (each
/// always means the newest model of that family); any full model name can still be typed.
pub fn models(k: Kind) -> Models {
    static CACHE: std::sync::OnceLock<Mutex<std::collections::HashMap<&'static str, Models>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Some(m) = cache.lock().unwrap().get(k.id()) { return m.clone(); }
    let efforts: Vec<String> = EFFORTS.iter().map(|s| s.to_string()).collect();
    let opt = |id: &str, label: &str| ModelOpt { id: id.into(), label: label.into(), efforts: vec![], default_effort: None };
    let m = match k {
        Kind::ClaudeCode => Models { models: vec![opt("fable", "Fable (newest)"), opt("opus", "Opus (newest)"), opt("sonnet", "Sonnet (newest)"), opt("haiku", "Haiku (newest)")], efforts, source: "Claude Code's model aliases".into() },
        Kind::Codex => {
            let list = find(k).and_then(|b| run_short(&b, &["debug", "models"], 30)).map(|(_, o)| parse_codex_models(&o)).unwrap_or_default();
            let source = if list.is_empty() { "Codex did not list its models; type one".into() } else { "from your Codex account".into() };
            Models { models: list, efforts, source }
        }
        Kind::Antigravity => {
            let list = find(k).and_then(|b| run_short(&b, &["models"], 30)).map(|(_, o)| parse_agy_models(&o)).unwrap_or_default();
            let source = if list.is_empty() { "Antigravity did not list its models; type one".into() } else { "from your Antigravity account".into() };
            Models { models: list, efforts, source }
        }
    };
    if !m.models.is_empty() { cache.lock().unwrap().insert(k.id(), m.clone()); }
    m
}

/// Stop the running turn (if any).
pub fn stop(slot: &Slot) -> bool { stop_with(slot, Child::kill) }

fn stop_with(slot: &Slot, kill: fn(&mut Child) -> std::io::Result<()>) -> bool {
    let mut child = {
        let mut running = slot.lock().unwrap();
        let Some(child) = running.as_mut() else { return false };
        if kill(child).is_err() { return false; }
        // Removing the killed child lets run distinguish an explicit Stop from a provider failure.
        // Reap it here: run no longer owns a handle to wait on.
        running.take().unwrap()
    };
    let _ = child.wait();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_stream() {
        let lines = [
            json!({"type":"system","subtype":"init","session_id":"S1","mcp_servers":[{"name":"kimo","status":"connected"}]}),
            json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Adding "}}}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"Adding a title."},{"type":"tool_use","id":"tu1","name":"mcp__kimo__text","input":{"action":"add_text","args":{"text":"Hi"}}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu1","content":[{"type":"text","text":"{\"clip_id\":\"clip3\"}"}]}]}}),
            json!({"type":"result","subtype":"success","is_error":false,"result":"Added a title.","session_id":"S1","total_cost_usd":0.01,"num_turns":2}),
        ];
        let evs: Vec<Ev> = lines.iter().flat_map(parse_claude).collect();
        assert_eq!(evs[0], Ev::Session { id: "S1".into() });
        assert_eq!(evs[1], Ev::Delta { text: "Adding ".into() });
        assert_eq!(evs[3], Ev::ToolStart { id: "tu1".into(), tool: "text".into(), input: json!({"action":"add_text","args":{"text":"Hi"}}) });
        assert_eq!(evs[4], Ev::ToolEnd { id: "tu1".into(), ok: true, output: "{\"clip_id\":\"clip3\"}".into() });
        assert!(matches!(&evs[6], Ev::Done { text, turns: Some(2), .. } if text == "Added a title."));
        let bad = parse_claude(&json!({"type":"system","subtype":"init","session_id":"S","mcp_servers":[{"name":"kimo","status":"failed"}]}));
        assert!(matches!(&bad[1], Ev::Error { .. }));
        let err = parse_claude(&json!({"type":"result","subtype":"error_max_turns","is_error":true,"session_id":"S"}));
        assert!(matches!(&err[1], Ev::Error { message } if message.contains("error_max_turns")));
    }

    #[test]
    fn codex_stream() {
        let lines = [
            json!({"type":"thread.started","thread_id":"T1"}),
            json!({"type":"turn.started"}),
            json!({"type":"item.completed","item":{"id":"i0","type":"reasoning","text":"Plan"}}),
            json!({"type":"item.started","item":{"id":"i1","type":"mcp_tool_call","server":"kimo","tool":"timeline","arguments":{"action":"add_track"},"status":"in_progress"}}),
            json!({"type":"item.completed","item":{"id":"i1","type":"mcp_tool_call","server":"kimo","tool":"timeline","arguments":{"action":"add_track"},"result":{"content":[{"type":"text","text":"{\"track_id\":\"track1\"}"}]},"error":null,"status":"completed"}}),
            json!({"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"Added a track."}}),
            json!({"type":"turn.completed","usage":{}}),
        ];
        let evs: Vec<Ev> = lines.iter().flat_map(parse_codex).collect();
        assert_eq!(evs[0], Ev::Session { id: "T1".into() });
        assert_eq!(evs[1], Ev::Thinking { text: "Plan".into() });
        assert_eq!(evs[2], Ev::ToolStart { id: "i1".into(), tool: "timeline".into(), input: json!({"action":"add_track"}) });
        assert_eq!(evs[3], Ev::ToolEnd { id: "i1".into(), ok: true, output: "{\"track_id\":\"track1\"}".into() });
        assert_eq!(evs[4], Ev::Text { text: "Added a track.".into() });
        let failed = parse_codex(&json!({"type":"item.completed","item":{"id":"i9","type":"mcp_tool_call","tool":"timeline","error":{"message":"no such clip"},"status":"failed"}}));
        assert_eq!(failed, vec![Ev::ToolEnd { id: "i9".into(), ok: false, output: "no such clip".into() }]);
        assert!(matches!(&parse_codex(&json!({"type":"turn.failed","error":{"message":"usage limit"}}))[0], Ev::Error { .. }));
    }

    #[test]
    fn antigravity_stream_as_captured_from_agy_1_4_6() {
        // exact lines from `agy -p ... --output-format stream-json` (agy 1.4.6)
        let lines = [
            r#"{"event":"init","conversation_id":"66e9","init":{"cwd":"C:\\x","tools":["call_mcp_tool"],"permission_mode":"always-proceed"}}"#,
            r#"{"event":"step_update","step_update":{"conversation_id":"66e9","step_index":0,"state":"DONE","step_type":"user_input"}}"#,
            r#"{"event":"step_update","step_update":{"conversation_id":"66e9","step_index":1,"state":"ACTIVE","step_type":"agent_response","text_delta":"ok"}}"#,
            r#"{"event":"step_update","step_update":{"conversation_id":"66e9","step_index":1,"state":"DONE","step_type":"agent_response","text_delta":"\n","duration_seconds":4.7}}"#,
            r#"{"event":"result","result":{"conversation_id":"66e9","status":"SUCCESS","response":"ok\n","duration_seconds":5.4,"num_turns":1}}"#,
        ];
        let evs: Vec<Ev> = lines.iter().flat_map(|l| parse_agy(&serde_json::from_str(l).unwrap())).collect();
        assert_eq!(evs[0], Ev::Session { id: "66e9".into() });
        assert_eq!(evs[1], Ev::Delta { text: "ok".into() });
        assert_eq!(evs[2], Ev::Delta { text: "\n".into() });
        assert_eq!(evs[4], Ev::Done { text: "ok".into(), cost_usd: None, turns: Some(1) });
        // tool steps exactly as agy 1.5.5 streams them
        let start = parse_agy(&serde_json::from_str(r#"{"event":"step_update","step_update":{"step_index":4,"state":"ACTIVE","step_type":"tool","tool_name":"call_mcp_tool","tool_info":{"name":"call_mcp_tool","parameters":{"Arguments":{"action":"add_track"},"ServerName":"kimo","ToolName":"timeline"}}}}"#).unwrap());
        assert_eq!(start, vec![Ev::ToolStart { id: "s4".into(), tool: "timeline".into(), input: json!({"action":"add_track"}) }]);
        let done = parse_agy(&serde_json::from_str(r#"{"event":"step_update","step_update":{"step_index":4,"state":"DONE","step_type":"tool","tool_name":"call_mcp_tool","tool_info":{"name":"call_mcp_tool","parameters":{"Arguments":{"action":"add_track"},"ServerName":"kimo","ToolName":"timeline"},"output":"{\"track_id\":\"track1\"}"}}}"#).unwrap());
        assert_eq!(done[1], Ev::ToolEnd { id: "s4".into(), ok: true, output: "{\"track_id\":\"track1\"}".into() });
        let err = parse_agy(&serde_json::from_str(r#"{"event":"step_update","step_update":{"step_index":4,"state":"ERROR","step_type":"tool","tool_info":{"name":"call_mcp_tool","parameters":{"ServerName":"kimo","ToolName":"project"},"error":{"type":"TOOL_ERROR","message":"server name kimo failed to load"}}}}"#).unwrap());
        assert_eq!(err[1], Ev::ToolEnd { id: "s4".into(), ok: false, output: "server name kimo failed to load".into() });
        let other = parse_agy(&serde_json::from_str(r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"tool","tool_info":{"name":"read_file","parameters":{"Path":"x"}}}}"#).unwrap());
        assert!(matches!(&other[0], Ev::ToolStart { tool, .. } if tool == "read_file"));
        assert!(parse_agy(&json!({"event":"step_update","step_update":{"step_index":13,"state":"DONE","step_type":"system_message"}})).is_empty());
    }

    fn turn<'a>(k: Kind, l: &'a KimoLaunch, w: &'a Path, s: Option<&'a str>) -> Turn<'a> { Turn { kind: k, bin: Path::new("x"), prompt: "make it warmer", session: s, model: None, effort: None, launch: l, workdir: w } }

    #[test]
    fn commands_attach_kimo_and_keep_the_prompt_off_the_command_line() {
        let w = std::env::temp_dir().join(format!("kimo-cli-{}", std::process::id()));
        std::fs::create_dir_all(&w).unwrap();
        let l = KimoLaunch { command: r"C:\Program Files\kimo\kimo-gui.exe".into(), env: vec![] };
        let (a, stdin) = command(&turn(Kind::ClaudeCode, &l, &w, None)).unwrap();
        assert!(a.windows(2).any(|p| p[0] == "--allowedTools" && p[1] == "mcp__kimo"));
        assert!(a.contains(&"--strict-mcp-config".to_string()));
        assert!(!a.iter().any(|x| x.contains("warmer")), "prompt goes on stdin");
        assert_eq!(stdin.as_deref(), Some("make it warmer"));
        let cfg: Value = serde_json::from_str(&std::fs::read_to_string(w.join(".kimo-mcp.json")).unwrap()).unwrap();
        assert_eq!(cfg["mcpServers"]["kimo"]["args"], json!(["mcp", "--client", "claude-code"]));
        let (a, _) = command(&turn(Kind::ClaudeCode, &l, &w, Some("S1"))).unwrap();
        assert!(a.windows(2).any(|p| p[0] == "--resume" && p[1] == "S1"));

        let (a, stdin) = command(&turn(Kind::Codex, &l, &w, None)).unwrap();
        assert_eq!(a[0], "exec");
        assert!(a.contains(&r"mcp_servers.kimo.command='C:\Program Files\kimo\kimo-gui.exe'".to_string()));
        assert!(a.contains(&"mcp_servers.kimo.args=['mcp','--client','codex']".to_string()));
        assert_eq!(a.last().unwrap(), "-");
        assert!(stdin.unwrap().contains("User request: make it warmer"), "first turn carries the kimo instructions");
        let (a, _) = command(&turn(Kind::Codex, &l, &w, Some("T1"))).unwrap();
        assert_eq!(&a[..3], ["exec", "resume", "T1"]);

        let (a, stdin) = command(&turn(Kind::Antigravity, &l, &w, Some("C1"))).unwrap();
        assert!(stdin.is_none());
        assert_eq!(a[1], "make it warmer");
        assert!(a.windows(2).any(|p| p[0] == "--conversation" && p[1] == "C1"));
        let _ = std::fs::remove_dir_all(&w);
    }

    /// `cargo test --lib -- --ignored --nocapture where_are_the_agents`: what this computer has (no network, no login).
    #[test]
    #[ignore]
    fn where_are_the_agents() {
        for k in Kind::all() { let s = status(k); eprintln!("{:12} installed={} signed_in={:?} version={:?} path={:?}", s.id, s.installed, s.signed_in, s.version, s.path); let m = models(k); eprintln!("   models ({}): {:?}", m.source, m.models.iter().map(|o| format!("{} [{}]", o.id, o.efforts.join("/"))).collect::<Vec<_>>()); }
    }

    #[test]
    fn model_and_effort_reach_each_cli() {
        let w = std::env::temp_dir().join(format!("kimo-cli-m-{}", std::process::id()));
        std::fs::create_dir_all(&w).unwrap();
        let l = KimoLaunch { command: "kimo-gui".into(), env: vec![] };
        let t = |k| Turn { kind: k, bin: Path::new("x"), prompt: "p", session: None, model: Some("m1"), effort: Some("high"), launch: &l, workdir: &w };
        let (a, _) = command(&t(Kind::ClaudeCode)).unwrap();
        assert!(a.windows(2).any(|p| p == ["--model", "m1"]) && a.windows(2).any(|p| p == ["--effort", "high"]));
        let (a, _) = command(&t(Kind::Codex)).unwrap();
        assert!(a.windows(2).any(|p| p == ["-m", "m1"]) && a.contains(&"model_reasoning_effort=\"high\"".to_string()));
        assert_eq!(a.last().unwrap(), "-", "the prompt marker stays last");
        let (a, _) = command(&t(Kind::Antigravity)).unwrap();
        assert!(a.windows(2).any(|p| p == ["--model", "m1"]) && a.windows(2).any(|p| p == ["--effort", "high"]));
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn model_lists_as_the_clis_print_them() {
        // `agy models` (1.5.5): progress line on stderr, then id<TAB>label
        let agy = parse_agy_models("Fetching available models...
gemini-3.8-flash-high	Gemini 3.8 Flash (High)
claude-opus-4-6-thinking	Claude Opus 4.6 (Thinking)
");
        assert_eq!(agy.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["gemini-3.8-flash-high", "claude-opus-4-6-thinking"]);
        assert_eq!(agy[1].label, "Claude Opus 4.6 (Thinking)");
        // `codex debug models` (0.167): hidden models are left out, efforts come per model
        let codex = parse_codex_models(r#"{"models":[{"slug":"gpt-reserve","display_name":"GPT-Reserve","visibility":"hide"},{"slug":"gpt-6-luna","display_name":"GPT-6-Luna","visibility":"list","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"high"}]}]}
warning: something on stderr"#);
        assert_eq!(codex, vec![ModelOpt { id: "gpt-6-luna".into(), label: "GPT-6-Luna".into(), efforts: vec!["low".into(), "high".into()], default_effort: Some("medium".into()) }]);
        assert!(parse_codex_models("not json").is_empty());
    }

    #[test]
    fn explains_sign_in_and_limits() {
        assert!(explain(Kind::Codex, "Error: Not logged in. Run codex login").unwrap().contains("Sign in"));
        assert!(explain(Kind::ClaudeCode, "API Error: 429 rate limit").unwrap().contains("usage limit"));
        assert!(explain(Kind::Antigravity, "boom").is_none());
        assert_eq!(short_tool("mcp__kimo__timeline"), "timeline");
    }
    #[test]
    fn sparse_streams_and_error_payloads_have_stable_neutral_events() {
        for (input, expected) in [
            (json!("plain"),"plain"), (json!([{"text":"one"},"two",{}]),"one\ntwo\n"),
            (Value::Null,""), (json!({"text":"nested"}),"nested"), (json!({"code":7}),"{\"code\":7}"),
        ] { assert_eq!(text_of(&input), expected); }
        assert_eq!(cut("é".repeat(601).as_str(),600),format!("{}…","é".repeat(600)));
        for prefix in ["mcp__kimo__","kimo__","kimo.",""] { assert_eq!(short_tool(&format!("{prefix}timeline")),"timeline"); }
        for input in [json!({}),json!({"type":"stream_event"}),json!({"type":"assistant"}),json!({"type":"user"})] {assert!(parse_claude(&input).is_empty());}
        assert_eq!(parse_claude(&json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"thinking":"Plan"}}})),vec![Ev::Thinking{text:"Plan".into()}]);
        assert_eq!(parse_claude(&json!({"type":"assistant","message":{"content":[{"type":"text"},{"type":"tool_use"},{}]}})),vec![Ev::Text{text:"".into()},Ev::ToolStart{id:"".into(),tool:"".into(),input:Value::Null}]);
        assert_eq!(parse_claude(&json!({"type":"user","message":{"content":[{"type":"tool_result","is_error":true},{}]}})),vec![Ev::ToolEnd{id:"".into(),ok:false,output:"".into()}]);
        assert_eq!(parse_claude(&json!({"type":"result","is_error":true})),vec![Ev::Error{message:"Claude Code stopped: error".into()}]);
        assert_eq!(parse_claude(&json!({"type":"result","subtype":"error_limit","result":"limit"})),vec![Ev::Error{message:"limit".into()}]);
        assert_eq!(parse_claude(&json!({"type":"result"})),vec![Ev::Done{text:"".into(),cost_usd:None,turns:None}]);
        assert!(matches!(&parse_claude(&json!({"type":"system","subtype":"init","mcp_servers":[{"name":"other"},{"name":"kimo"}]}))[0],Ev::Error{message} if message.contains("()")));
        for input in [json!({}),json!({"type":"thread.started"}),json!({"type":"item.updated","item":{"type":"mcp_tool_call"}}),json!({"type":"item.completed","item":{}})] {assert!(parse_codex(&input).is_empty());}
        assert_eq!(parse_codex(&json!({"type":"item.started","item":{"type":"command_execution","command":"fixture"}})),vec![Ev::ToolStart{id:"".into(),tool:"shell".into(),input:json!({"command":"fixture"})}]);
        for (exit,ok) in [(0,true),(1,false)] {assert_eq!(parse_codex(&json!({"type":"item.completed","item":{"type":"command_execution","exit_code":exit,"aggregated_output":"output"}})),vec![Ev::ToolEnd{id:"".into(),ok,output:"output".into()}]);}
        assert_eq!(parse_codex(&json!({"type":"item.completed","item":{"type":"error"}})),vec![Ev::Error{message:"error".into()}]);
        assert_eq!(parse_codex(&json!({"type":"turn.failed"})),vec![Ev::Error{message:"Codex stopped with an error".into()}]);
        assert_eq!(parse_codex(&json!({"type":"error"})),vec![Ev::Error{message:"Codex error".into()}]);
        assert_eq!(parse_codex(&json!({"type":"error","message":"specific"})),vec![Ev::Error{message:"specific".into()}]);
        assert_eq!(parse_codex(&json!({"type":"item.completed","item":{"type":"mcp_tool_call","status":"failed"}})),vec![Ev::ToolEnd{id:"".into(),ok:false,output:"".into()}]);
        assert_eq!(parse_codex(&json!({"type":"item.completed","item":{"type":"mcp_tool_call","status":"completed","error":null,"result":{"content":"ok"}}})),vec![Ev::ToolEnd{id:"".into(),ok:true,output:"ok".into()}]);
    }

    #[test]
    fn antigravity_sparse_states_and_cancellations_are_not_successes() {
        for input in [json!({}),json!({"event":"init"}),json!({"event":"step_update"}),json!({"event":"step_update","step_update":{"step_type":"agent_response"}}),json!({"event":"step_update","step_update":{"step_type":"agent_response","text_delta":""}})] {assert!(parse_agy(&input).is_empty());}
        for kind in ["thinking","planner_thinking"] {
            assert_eq!(parse_agy(&json!({"event":"step_update","step_update":{"step_type":kind,"text_delta":"plan"}})),vec![Ev::Thinking{text:"plan".into()}]);
            assert!(parse_agy(&json!({"event":"step_update","step_update":{"step_type":kind}})).is_empty());
        }
        for state in ["RUNNING","PENDING"] {
            assert_eq!(parse_agy(&json!({"event":"step_update","step_update":{"step_type":"tool","state":state,"tool_name":"legacy"}})),vec![Ev::ToolStart{id:"s0".into(),tool:"legacy".into(),input:Value::Null}]);
        }
        assert_eq!(parse_agy(&json!({"event":"step_update","step_update":{"step_type":"tool","state":"ACTIVE"}})),vec![Ev::ToolStart{id:"s0".into(),tool:"tool".into(),input:Value::Null}]);
        assert!(parse_agy(&json!({"event":"step_update","step_update":{"step_type":"tool"}})).is_empty());
        for (server,tool) in [("","mcp"),("other","other.mcp")] {
            assert_eq!(parse_agy(&json!({"event":"step_update","step_update":{"step_type":"tool","state":"ACTIVE","tool_info":{"name":"call_mcp_tool","parameters":{"ServerName":server}}}})),vec![Ev::ToolStart{id:"s0".into(),tool:tool.into(),input:Value::Null}]);
        }
        for state in ["FAILED","CANCELED","CANCELLED"] {
            let events=parse_agy(&json!({"event":"step_update","step_update":{"step_type":"tool","state":state,"tool_info":{"error":"cancelled"}}}));
            assert_eq!(events,vec![Ev::ToolStart{id:"s0".into(),tool:"tool".into(),input:Value::Null},Ev::ToolEnd{id:"s0".into(),ok:false,output:"cancelled".into()}]);
        }
        for (result,message) in [(json!({}),"Antigravity stopped: error"),(json!({"status":"FAILED"}),"Antigravity stopped: FAILED"),(json!({"status":"FAILED","response":"specific \n"}),"specific")] {
            assert_eq!(parse_agy(&json!({"event":"result","result":result})),vec![Ev::Error{message:message.into()}]);
        }
        for (input,message) in [(json!({"event":"error"}),"Antigravity error"),(json!({"event":"error","message":"outer"}),"outer"),(json!({"event":"error","error":{"message":"inner"},"message":"outer"}),"inner")] {assert_eq!(parse_agy(&input),vec![Ev::Error{message:message.into()}]);}
    }

    #[test]
    fn desktop_launch_plans_preserve_arguments_and_platform_policy() {
        let args=|plan:&DesktopCommand| plan.args.iter().map(|x| x.to_string_lossy().into_owned()).collect::<Vec<_>>();
        let windows=terminal_command_for("windows","agent login", &|_|None).unwrap();
        assert_eq!(windows.program,Path::new("cmd"));assert!(windows.args.is_empty());
        assert_eq!(windows.raw.as_deref(),Some("/c start \"kimo\" cmd /k \"agent login\""));assert!(!windows.quiet);
        let mac=terminal_command_for("macos",r#"agent "a\b""#, &|_|None).unwrap();
        assert_eq!(mac.program,Path::new("osascript"));
        assert_eq!(args(&mac),vec!["-e",r#"tell application "Terminal" to do script "agent \"a\\b\""
tell application "Terminal" to activate"#]);
        for (name,flag) in [("x-terminal-emulator",Some("-e")),("gnome-terminal",Some("--")),("konsole",Some("-e")),("xfce4-terminal",Some("-x")),("kitty",None),("alacritty",Some("-e")),("xterm",Some("-e"))] {
            let plan=terminal_command_for("linux","agent login",&|candidate| (candidate==name).then(||PathBuf::from(name))).unwrap();
            assert_eq!(plan.program,Path::new(name));
            let mut expected:Vec<&str>=flag.into_iter().collect();expected.extend(["sh","-c","agent login; echo; read -p 'Press Enter to close'"]);
            assert_eq!(args(&plan),expected);assert!(plan.raw.is_none());assert!(!plan.quiet);
        }
        assert_eq!(terminal_command_for("linux","agent",&|_|Some("present".into())).unwrap().program,Path::new("x-terminal-emulator"));
        assert_eq!(terminal_command_for("linux","agent",&|_|None).err().unwrap(),"No terminal app found. Run this yourself: agent");
        for (os,program) in [("windows","rundll32"),("macos","open"),("linux","xdg-open")] {
            let plan=url_command_for(os,"https://example.invalid/a b").unwrap();
            assert_eq!(plan.program,Path::new(program));assert_eq!(plan.quiet,os=="windows");assert!(plan.raw.is_none());
            let expected=if os=="windows" {vec!["url.dll,FileProtocolHandler","https://example.invalid/a b"]} else {vec!["https://example.invalid/a b"]};assert_eq!(args(&plan),expected);
        }
        assert_eq!(open_url("http://example.invalid"),Err("only https links".into()));
        let path=Path::new("folder with spaces/é.kimo");
        let win=reveal_command_for("windows",path);assert_eq!(win.program,Path::new("explorer"));assert_eq!(win.raw,Some(format!("/select,\"{}\"",path.display())));assert!(win.args.is_empty());
        let mac=reveal_command_for("macos",path);assert_eq!(mac.program,Path::new("open"));assert_eq!(mac.args,vec![std::ffi::OsString::from("-R"),path.as_os_str().to_owned()]);
        let linux=reveal_command_for("linux",path);assert_eq!(linux.program,Path::new("xdg-open"));assert_eq!(linux.args,vec![path.parent().unwrap().as_os_str().to_owned()]);
        assert_eq!(reveal_command_for("linux",Path::new("")).args,vec![std::ffi::OsString::new()]);
        // Interior NUL is rejected before any desktop application can be launched.
        assert!(open_url("https://example.invalid/\0").is_err());
        assert!(open_terminal("agent\0").is_err());
        assert!(reveal(Path::new("bad\0parent/file")).is_err());
    }

    #[test]
    fn real_child_streams_preserve_prompts_results_errors_and_stop() {
        fn collect(t: &Turn, slot: &Slot) -> (Outcome, Vec<Ev>) {
            let mut events = vec![];
            let outcome = run(t, slot, &mut |event| events.push(event));
            (outcome, events)
        }
        let dir=std::env::temp_dir().join(format!("kimo-cli-process-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();
        let source=dir.join("fixture.rs");let bin=dir.join("fixture").with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&source,r#"
use std::io::{Read,Write};
fn main() {
    let root=std::env::current_exe().unwrap().parent().unwrap().to_owned();
    let args:Vec<_>=std::env::args().skip(1).collect();
    std::fs::write(root.join("argv"),args.join("\0")).unwrap();
    let mut input=String::new();if !root.join("detached").exists(){std::io::stdin().read_to_string(&mut input).unwrap();}
    std::fs::write(root.join("stdin"),input).unwrap();
    std::io::stdout().write_all(&std::fs::read(root.join("stdout")).unwrap()).unwrap();std::io::stdout().flush().unwrap();
    std::io::stderr().write_all(&std::fs::read(root.join("stderr")).unwrap()).unwrap();
    if root.join("wait").exists(){std::thread::sleep(std::time::Duration::from_secs(30));}
    std::process::exit(std::fs::read_to_string(root.join("exit")).unwrap().trim().parse().unwrap());
}
"#).unwrap();
        let built=Command::new("rustc").args(["--edition=2021","--crate-name","fixture"]).arg(&source).arg("-o").arg(&bin).output().unwrap();
        assert!(built.status.success(),"{}",String::from_utf8_lossy(&built.stderr));
        std::fs::write(dir.join("detached"),"").unwrap();
        for name in ["stdout","stderr"] {std::fs::write(dir.join(name),"").unwrap();}
        std::fs::write(dir.join("exit"),"0").unwrap();
        for quiet in [false,true] {
            let plan=DesktopCommand{program:bin.clone(),args:vec!["space argument".into(),"é".into()],raw:None,quiet};
            assert!(spawn_desktop_command(plan).unwrap().wait().unwrap().success());
            assert_eq!(std::fs::read_to_string(dir.join("argv")).unwrap(),"space argument\0é");
        }
        assert!(spawn_desktop_command(DesktopCommand{program:dir.join("absent"),args:vec![],raw:None,quiet:false}).is_err());
        std::fs::remove_file(dir.join("detached")).unwrap();
        std::fs::write(dir.join("wait"),"").unwrap();
        assert!(run_short(&bin,&[],0).is_none(),"expired subprocess budget must kill and reap the fixture");
        std::fs::remove_file(dir.join("wait")).unwrap();
        let launch=KimoLaunch{command:"fixture-kimo".into(),env:vec![("FIXTURE".into(),"one two".into())]};
        let slot:Slot=Arc::new(Mutex::new(None));
        let cases=[
            (Kind::ClaudeCode,vec![json!({"type":"system","subtype":"init","session_id":"session"}),json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"draft"}}}),json!({"type":"assistant","message":{"content":[{"type":"text","text":"final"}]}}),json!({"type":"result","subtype":"success","result":"final"})]),
            (Kind::Codex,vec![json!({"type":"thread.started","thread_id":"session"}),json!({"type":"item.completed","item":{"type":"reasoning","text":"thinking"}}),json!({"type":"item.completed","item":{"type":"agent_message","text":"final"}}),json!({"type":"turn.completed"})]),
            (Kind::Antigravity,vec![json!({"event":"init","conversation_id":"session"}),json!({"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"draft"}}),json!({"event":"result","result":{"status":"SUCCESS","response":"final"}})])
        ];
        for (kind,events) in cases {
            std::fs::write(dir.join("stdout"),format!("not json\n{}\n",events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"))).unwrap();
            std::fs::write(dir.join("stderr"),"fixture diagnostic\n").unwrap();std::fs::write(dir.join("exit"),"0").unwrap();
            let turn=Turn{kind,bin:&bin,prompt:"Éditer sans shell",session:None,model:Some("fixture-model"),effort:Some("high"),launch:&launch,workdir:&dir};
            let expected=command(&turn).unwrap();
            let (result,got)=collect(&turn,&slot);
            assert_eq!(result.session.as_deref(),Some("session"));assert_eq!(result.reply,"final");assert_eq!(result.error,None);
            assert!(got.iter().any(|e|matches!(e,Ev::Session{id} if id=="session")));
            assert_eq!(std::fs::read_to_string(dir.join("stdin")).unwrap(),expected.1.unwrap_or_default());
            assert_eq!(std::fs::read_to_string(dir.join("argv")).unwrap().split('\0').collect::<Vec<_>>(),expected.0);
            assert!(slot.lock().unwrap().is_none());assert!(!stop(&slot));
        }
        for done in [false,true] {
            let mut events=vec![json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"streamed fallback"}}})];
            if done {events.push(json!({"type":"result","subtype":"success","result":""}));}
            std::fs::write(dir.join("stdout"),events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")+"\n").unwrap();
            std::fs::write(dir.join("stderr"),"diagnostic\n".repeat(1000)).unwrap();
            let fallback=Turn{kind:Kind::ClaudeCode,bin:&bin,prompt:"fixture",session:None,model:None,effort:None,launch:&launch,workdir:&dir};
            let (result,got)=collect(&fallback,&slot);
            assert_eq!(result.reply,"streamed fallback");assert!(result.error.is_none());
            assert!(matches!(&got[0],Ev::Delta{text} if text=="streamed fallback"));
        }
        std::fs::write(dir.join("stdout"),concat!("{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"kept reply\"}]}}\n", "{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"\"}\n")).unwrap();
        let completed=Turn{kind:Kind::ClaudeCode,bin:&bin,prompt:"fixture",session:None,model:None,effort:None,launch:&launch,workdir:&dir};
        let (result,events)=collect(&completed,&slot);
        assert_eq!(result.reply,"kept reply"); assert!(result.error.is_none()); assert_eq!(events.len(),2);
        let turn=Turn{kind:Kind::Codex,bin:&bin,prompt:"fixture",session:Some("previous"),model:None,effort:None,launch:&launch,workdir:&dir};
        for (stderr,expected) in [("Please login","not signed in"),("quota exceeded","usage limit"),("fixture failure","fixture failure")] {
            std::fs::write(dir.join("stdout"),"").unwrap();std::fs::write(dir.join("stderr"),stderr).unwrap();std::fs::write(dir.join("exit"),"7").unwrap();
            let (result,got)=collect(&turn,&slot);
            assert!(result.error.unwrap().contains(expected));assert_eq!(result.session.as_deref(),Some("previous"));assert!(result.reply.is_empty());assert!(got.is_empty());
        }
        std::fs::write(dir.join("stdout"),json!({"type":"error","message":"provider refused"}).to_string()+"\n").unwrap();
        std::fs::write(dir.join("stderr"),"login").unwrap();
        let (result,got)=collect(&turn,&slot);assert_eq!(result.error.as_deref(),Some("provider refused"));assert_eq!(got,vec![Ev::Error{message:"provider refused".into()}]);
        std::fs::write(dir.join("stdout"),"").unwrap();std::fs::write(dir.join("stderr"),"").unwrap();std::fs::write(dir.join("exit"),"0").unwrap();
        for kind in Kind::all() {
            assert_eq!(register(kind,&bin,&launch).unwrap(),"Added kimo.");
            let argv=std::fs::read_to_string(dir.join("argv")).unwrap();
            assert!(argv.contains("kimo"));assert!(argv.contains("fixture-kimo"));
            if kind==Kind::ClaudeCode {
                let config:Value=serde_json::from_str(argv.split('\0').last().unwrap()).unwrap();
                assert_eq!(config["command"],"fixture-kimo");assert_eq!(config["env"]["FIXTURE"],"one two");
            } else {assert!(argv.contains("FIXTURE=one two"));assert!(argv.contains("--client"));}
        }
        let plain_launch=KimoLaunch{command:"fixture-kimo".into(),env:vec![]};
        assert_eq!(register(Kind::ClaudeCode,&bin,&plain_launch).unwrap(),"Added kimo.");
        let argv=std::fs::read_to_string(dir.join("argv")).unwrap();
        let config:Value=serde_json::from_str(argv.split('\0').last().unwrap()).unwrap();
        assert!(config.get("env").is_none());assert_eq!(config["args"],json!(["mcp","--client","claude-code"]));
        std::fs::write(dir.join("stdout"),"registered ").unwrap();std::fs::write(dir.join("stderr"),"fixture").unwrap();
        assert_eq!(run_register(&bin,&[]).unwrap(),"registered fixture");
        std::fs::write(dir.join("exit"),"4").unwrap();assert_eq!(run_register(&bin,&[]).unwrap_err(),"registered fixture");
        std::fs::write(dir.join("stdout"),"").unwrap();std::fs::write(dir.join("stderr"),"").unwrap();
        assert!(run_register(&bin,&[]).unwrap_err().contains("exited with"));assert!(run_register(&dir.join("absent"),&[]).is_err());
        let missing=dir.join("absent");let invalid=Turn{bin:&missing,..turn};
        let (result,got)=collect(&invalid,&slot);assert!(result.error.unwrap().contains("Could not start Codex"));assert!(got.is_empty());
        let unwritable=Turn{kind:Kind::ClaudeCode,workdir:&missing,..turn};
        let (result,got)=collect(&unwritable,&slot);assert!(result.error.unwrap().contains("could not write"));assert!(got.is_empty());
        for scenario in ["models","signin","rediscover-path","rediscover-extra","invalid"] {
            let home=dir.join(format!("home-{scenario}"));std::fs::create_dir_all(&home).unwrap();
            let output=Command::new(std::env::current_exe().unwrap())
                .args(["--exact","cli_agents::tests::discovery_models_and_signin_child","--nocapture"])
                .env("KIMO_CLI_FIXTURE_DIR",&dir).env("KIMO_CLI_FIXTURE_SCENARIO",scenario)
                .env("HOME",&home).env("USERPROFILE",&home).env("APPDATA",home.join("roaming")).env("LOCALAPPDATA",home.join("local"))
                .env("CODEX_HOME",home.join(".codex")).env("PATH","").env_remove("ANTHROPIC_API_KEY")
                .env("KIMO_CLAUDE_BIN",&bin).env("KIMO_CODEX_BIN",&bin).env("KIMO_AGY_BIN",&bin).output().unwrap();
            if scenario=="invalid" {
                assert!(!output.status.success());assert!(format!("{}{}",String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr)).contains("unknown fixture scenario"));
            } else {
                assert!(output.status.success(),"{}{}",String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
                assert!(String::from_utf8_lossy(&output.stdout).contains("fixture scenario passed"));
            }
        }
        std::fs::write(dir.join("stdout"),json!({"type":"thread.started","thread_id":"stopping"}).to_string()+"\n").unwrap();
        std::fs::write(dir.join("stderr"),"").unwrap();std::fs::write(dir.join("wait"),"").unwrap();
        let result=run(&turn,&slot,&mut |e|{
            assert!(matches!(e,Ev::Session{..}));
            assert!(!stop_with(&slot,|_| Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied,"fixture kill refusal"))));
            assert!(slot.lock().unwrap().as_mut().unwrap().try_wait().unwrap().is_none(),"failed kill must preserve the running child");
            assert!(stop(&slot));
        });
        assert_eq!(result.error.as_deref(),Some("Stopped."));assert!(slot.lock().unwrap().is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn discovery_models_and_signin_child() {
        let Some(root)=std::env::var_os("KIMO_CLI_FIXTURE_DIR").map(PathBuf::from) else { return };
        let home=home_dir().unwrap();let bin=root.join("fixture").with_extension(std::env::consts::EXE_EXTENSION);
        let response=|text:&str,code:&str| {std::fs::write(root.join("stdout"),text).unwrap();std::fs::write(root.join("stderr"),"").unwrap();std::fs::write(root.join("exit"),code).unwrap();};
        let file=|name:&str,text:&str| {let path=home.join(name);std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(path,text).unwrap();};
        response("fixture CLI 1.0\n","0");
        for k in Kind::all() {assert_eq!(find(k),Some(bin.clone()));assert_eq!(find(k),Some(bin.clone()));}
        match std::env::var("KIMO_CLI_FIXTURE_SCENARIO").unwrap().as_str() {
            "models" => {
                let claude=models(Kind::ClaudeCode);assert!(claude.models.iter().any(|m|m.id=="sonnet"));assert_eq!(models(Kind::ClaudeCode).models.len(),claude.models.len());
                for (kind,catalog,id) in [(Kind::Codex,"{\"models\":[{\"slug\":\"fixture-codex\",\"display_name\":\"Fixture Codex\"}]}","fixture-codex"),(Kind::Antigravity,"fixture-agy\tFixture Agy\n","fixture-agy")] {
                    response("not a catalog","0");let empty=models(kind);assert!(empty.models.is_empty());assert!(empty.source.contains("did not list"));
                    response(catalog,"0");let found=models(kind);assert_eq!(found.models.len(),1);assert_eq!(found.models[0].id,id);assert!(found.source.starts_with("from your"));
                    response("invalid after cached success","0");assert_eq!(models(kind).models[0].id,id);
                }
            }
            "signin" => {
                let claude=status(Kind::ClaudeCode);assert!(claude.installed);assert_eq!(claude.version.as_deref(),Some("fixture CLI 1.0"));
                assert_eq!(status_for_platform(Kind::ClaudeCode,false).signed_in,Some(false));
                let keychain=status_for_platform(Kind::ClaudeCode,true);assert_eq!(keychain.signed_in,None);assert!(keychain.sign_in_note.contains("macOS Keychain"));
                std::env::set_var("ANTHROPIC_API_KEY","fixture-never-sent");assert!(status(Kind::ClaudeCode).sign_in_note.contains("ANTHROPIC_API_KEY"));std::env::remove_var("ANTHROPIC_API_KEY");
                file(".claude/.credentials.json","not a credential document");assert_eq!(status(Kind::ClaudeCode).signed_in,Some(true));
                response("","0");let codex=status(Kind::Codex);assert_eq!(codex.signed_in,Some(true));assert_eq!(codex.sign_in_note,"Signed in");assert_eq!(codex.version,None);
                response("","1");let codex=status(Kind::Codex);assert_eq!(codex.signed_in,Some(false));assert_eq!(codex.sign_in_note,"Not signed in");
                response("Sign in refused\nmore diagnostics","1");assert_eq!(status(Kind::Codex).sign_in_note,"Sign in refused");
                file(".codex/auth.json","not a credential document");
                let executable=std::fs::read(&bin).unwrap();std::fs::write(&bin,b"invalid executable fixture").unwrap();
                assert_eq!(status(Kind::Codex).signed_in,Some(true));std::fs::remove_file(home.join(".codex/auth.json")).unwrap();assert_eq!(status(Kind::Codex).signed_in,Some(false));std::fs::write(&bin,executable).unwrap();
                response("fixture","0");assert_eq!(status(Kind::Antigravity).signed_in,Some(false));
                file(".gemini/oauth_creds.json","not a credential document");assert_eq!(status(Kind::Antigravity).signed_in,Some(true));std::fs::remove_file(home.join(".gemini/oauth_creds.json")).unwrap();
                file(".gemini/google_accounts.json","not an account document");assert_eq!(status(Kind::Antigravity).signed_in,Some(true));
                for k in Kind::all(){assert!(!registered(k));}
                file(".claude.json","{\"mcpServers\":{\"kimo\":{\"command\":\"fixture\"}}}");assert!(registered(Kind::ClaudeCode));
                file(".codex/config.toml","[mcp_servers.kimo]\ncommand = \"fixture\"\n");assert!(registered(Kind::Codex));
                for name in [".gemini/config/mcp_config.json",".gemini/antigravity/mcp_config.json",".gemini/antigravity-cli/mcp_config.json",".gemini/settings.json"] {
                    file(name,"{\"mcpServers\":{\"kimo\":{\"command\":\"fixture\"}}}");assert!(registered(Kind::Antigravity));std::fs::remove_file(home.join(name)).unwrap();
                }
                std::env::remove_var("HOME");std::env::remove_var("USERPROFILE");for k in Kind::all(){assert!(!registered(k));}
            }
            scenario @ ("rediscover-path" | "rediscover-extra") => {
                let path=scenario=="rediscover-path";
                let directory=if path {home.join("shims")} else {home.join(".local/bin")};
                std::fs::create_dir_all(&directory).unwrap();
                let replacement=directory.join("codex").with_extension(std::env::consts::EXE_EXTENSION);
                std::fs::copy(&bin,&replacement).unwrap();
                for (name,contents) in [("stdout","replacement fixture"),("stderr",""),("exit","0")] {std::fs::write(directory.join(name),contents).unwrap();}
                if path {std::env::set_var("PATH",&directory);}
                std::fs::remove_file(&bin).unwrap();
                let found=find(Kind::Codex);
                std::fs::copy(&replacement,&bin).unwrap();
                assert_eq!(found,Some(replacement));
            }
            other => panic!("unknown fixture scenario: {other}"),
        }
        println!("fixture scenario passed");
    }

    #[test]
    fn platform_search_layouts_include_bundled_and_extra_candidates() {
        let root=std::env::temp_dir().join(format!("kimo-cli-layouts-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);
        for windows in [false,true] {
            let base=root.join(windows.to_string());let home=base.join("home");let roaming=base.join("roaming");let local=base.join("local");
            let variables=std::collections::HashMap::from([("APPDATA",roaming.clone()),("LOCALAPPDATA",local.clone())]);
            let env=|key:&str|variables.get(key).cloned();
            let write=|path:PathBuf| {std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(&path,b"fixture metadata").unwrap();path};
            let exe=|name:&str|if windows {format!("{name}.exe")} else {name.to_string()};
            for k in Kind::all() {
                assert_eq!(Kind::from_id(k.id()),Some(k));assert_eq!(Kind::from_id("Unknown"),None);
                let dirs=extra_dirs_for(k,windows,Some(&home),&env);
                if k==Kind::ClaudeCode {assert!(dirs.contains(&home.join(".claude/local")));}
                if k==Kind::Antigravity {assert!(dirs.contains(&home.join(".antigravity/bin")));assert!(dirs.contains(&home.join(".gemini/antigravity-cli/bin")));}
                assert!(dirs.contains(&home.join(".local/bin")));assert!(dirs.contains(&home.join(".npm-global/bin")));
                assert!(dirs.contains(&home.join(".bun/bin")));assert!(dirs.contains(&home.join(".volta/bin")));
                if windows {assert!(dirs.contains(&roaming.join("npm")));assert!(dirs.contains(&local.join("Programs/Antigravity/bin")));} else {assert!(dirs.contains(&PathBuf::from("/usr/bin")));}
                let search=base.join(format!("search-{}",k.id()));let file=write(search.join(exe(k.bin_name())));
                #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&file,std::fs::Permissions::from_mode(0o755)).unwrap();}
                assert_eq!(extra_candidate(k,&[search.clone()],windows),Some(file.clone()));
                assert!(extra_candidate(k,&[],windows).is_none());
                if windows==cfg!(windows) {assert_eq!(extra_candidate(k,&[base.join("absent"),search],windows),Some(file));}
            }
            let claude=exe("claude");
            let expected=[
                home.join(format!("Library/Application Support/Claude/claude-code/v1/{claude}")),
                home.join(format!(".config/Claude/claude-code/v1/build/{claude}")),
                home.join(format!(".config/Claude/claude-code/v2/{claude}")),
                home.join(format!(".local/share/claude/versions/v3/{claude}")),
                local.join(format!("Claude-Data/profile/claude-code/v1/build/{claude}")),
                local.join(format!("AnthropicClaude/claude-code/v1/{claude}")),
                roaming.join(format!("Claude/claude-code/v1/{claude}")),
                roaming.join("npm/node_modules/@anthropic-ai/claude-code/node_modules/@anthropic-ai/claude-code-win32-x64/claude.exe"),
                roaming.join("npm/node_modules/@anthropic-ai/claude-code/bin/claude.exe"),
            ];
            for path in &expected {write(path.clone());}
            write(home.join(".config/Claude/claude-code/not-a-version-file"));
            let found=bundled_for(Kind::ClaudeCode,windows,Some(&home),&env);for path in expected {assert!(found.contains(&path),"{}",path.display());}
            let codex=exe("codex");
            let expected=[home.join(format!(".codex/plugins/.plugin-appserver/{codex}")),home.join(format!(".codex/bin/{codex}")),local.join(format!("Programs/Codex/resources/{codex}")),local.join(format!("Programs/codex/resources/{codex}"))];
            for path in &expected {write(path.clone());}
            let found=bundled_for(Kind::Codex,windows,Some(&home),&env);for path in expected {assert!(found.contains(&path));}
            let agy=write(home.join(format!(".gemini/antigravity-cli/bin/{}",exe("agy"))));
            assert_eq!(bundled_for(Kind::Antigravity,windows,Some(&home),&env),vec![agy]);
            for k in Kind::all() {let dirs=extra_dirs_for(k,windows,None,&|_|None);assert_eq!(dirs.len(),if windows{0}else{3});}
            assert!(bundled_for(Kind::ClaudeCode,windows,None,&|_|None).is_empty());
            assert!(bundled_for(Kind::Antigravity,windows,None,&|_|None).is_empty());
            assert!(!bundled_for(Kind::Codex,windows,None,&|_|None).contains(&home.join(format!(".codex/bin/{}",exe("codex")))));
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unreadable_version_children_keep_the_direct_binary_candidate() {
        let root=std::env::temp_dir().join(format!("kimo-unreadable-cli-version-{}",std::process::id()));
        std::fs::create_dir_all(root.join("restricted/sub")).unwrap();
        std::fs::write(root.join("plain-file"),b"fixture").unwrap();
        fn listing(path:&Path)->std::io::Result<std::fs::ReadDir> {
            if path.file_name()==Some(std::ffi::OsStr::new("restricted")) {return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));}
            std::fs::read_dir(path)
        }
        assert_eq!(version_candidates(root.clone(),"cli",listing),vec![root.join("restricted/cli")]);
        assert!(version_candidates(root.join("missing"),"cli",listing).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

}
