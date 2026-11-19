//! Run the packaged Linux window on an owned X server and session bus.
#![cfg(target_os = "linux")]
use std::{ffi::{CStr, CString, c_char, c_int, c_long, c_uint, c_ulong, c_void}, io::BufRead,
    path::PathBuf, process::{Child, Command, Stdio}, time::{Duration, Instant}};

struct Process(Child);
impl Drop for Process { fn drop(&mut self) { if self.0.try_wait().ok().flatten().is_none() { let _ = self.0.kill(); } let _ = self.0.wait(); } }
struct OwnedDir(PathBuf);
impl Drop for OwnedDir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
#[repr(C)]
#[derive(Clone, Copy)]
struct ClientMessage { kind: c_int, serial: c_ulong, send_event: c_int, display: *mut c_void,
    window: c_ulong, message_type: c_ulong, format: c_int, data: [c_long; 5] }
#[repr(C)]
union Event { client: ClientMessage, pad: [c_long; 24] }
#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut c_void;
    fn XCloseDisplay(display: *mut c_void) -> c_int;
    fn XDefaultRootWindow(display: *mut c_void) -> c_ulong;
    fn XQueryTree(display: *mut c_void, window: c_ulong, root: *mut c_ulong, parent: *mut c_ulong, children: *mut *mut c_ulong, count: *mut c_uint) -> c_int;
    fn XFetchName(display: *mut c_void, window: c_ulong, name: *mut *mut c_char) -> c_int;
    fn XFree(data: *mut c_void) -> c_int;
    fn XInternAtom(display: *mut c_void, name: *const c_char, only_if_exists: c_int) -> c_ulong;
    fn XSendEvent(display: *mut c_void, window: c_ulong, propagate: c_int, mask: c_long, event: *mut Event) -> c_int;
    fn XFlush(display: *mut c_void) -> c_int;
}
struct Display(*mut c_void);
impl Drop for Display { fn drop(&mut self) { unsafe { XCloseDisplay(self.0); } } }
impl Display {
    fn main_window(&self) -> Option<c_ulong> {
        let (mut root, mut parent, mut count) = (0, 0, 0);
        let mut children = std::ptr::null_mut(); let mut found = None;
        // Xlib owns both returned allocations; release each after copying its data.
        unsafe {
            if XQueryTree(self.0, XDefaultRootWindow(self.0), &mut root, &mut parent, &mut children, &mut count) != 0 {
                for i in 0..count as usize {
                    let window = *children.add(i); let mut name = std::ptr::null_mut();
                    if XFetchName(self.0, window, &mut name) != 0 && !name.is_null() {
                        if CStr::from_ptr(name).to_bytes() == b"kimo" { found = Some(window); }
                        XFree(name.cast());
                    }
                }
                if !children.is_null() { XFree(children.cast()); }
            }
        }
        found
    }
    fn close_window(&self, window: c_ulong) {
        let protocol = CString::new("WM_PROTOCOLS").unwrap();
        let close = CString::new("WM_DELETE_WINDOW").unwrap();
        unsafe {
            let mut event = Event { client: ClientMessage { kind: 33, serial: 0, send_event: 1,
                display: self.0, window, message_type: XInternAtom(self.0, protocol.as_ptr(), 0), format: 32,
                data: [XInternAtom(self.0, close.as_ptr(), 0) as c_long, 0, 0, 0, 0] } };
            assert_ne!(XSendEvent(self.0, window, 0, 0, &mut event), 0);
            XFlush(self.0);
        }
    }
}
fn first_line(child: &mut Process) -> String {
    let mut line = String::new();
    std::io::BufReader::new(child.0.stdout.take().unwrap()).read_line(&mut line).unwrap();
    assert!(!line.trim().is_empty(), "owned display/session service failed to start");
    line.trim().to_owned()
}
fn wait_success(child: &mut Process, log: &PathBuf) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "{status}: {}", std::fs::read_to_string(log).unwrap()); return;
        }
        assert!(Instant::now() < deadline, "owned application failed to exit: {}", std::fs::read_to_string(log).unwrap());
        std::thread::sleep(Duration::from_millis(50));
    }
}
#[test]
fn native_window_accepts_second_instance_and_closes_its_owned_engine_cleanly() {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = OwnedDir(std::env::temp_dir().join(format!("kimo-window-{}-{nonce}", std::process::id())));
    std::fs::create_dir(&dir.0).unwrap();
    let diagnostics = std::fs::File::create(dir.0.join("session.log")).unwrap();
    let mut screen = Process(Command::new("Xvfb").args(["-displayfd", "1", "-screen", "0", "1280x800x24", "-nolisten", "tcp"])
        .stdout(Stdio::piped()).stderr(diagnostics.try_clone().unwrap()).spawn().expect("Xvfb is required for the native Linux window proof"));
    let screen_name = format!(":{}", first_line(&mut screen));
    let mut bus = Process(Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"])
        .stdout(Stdio::piped()).stderr(diagnostics).spawn().expect("dbus-daemon is required for the owned session"));
    let address = first_line(&mut bus); assert!(address.starts_with("unix:"));
    let log_path = dir.0.join("app.log"); let log = std::fs::File::create(&log_path).unwrap();
    let command = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kimo-gui"));
        command.env("DISPLAY", &screen_name).env("DBUS_SESSION_BUS_ADDRESS", &address)
            .env("KIMO_DATA_DIR", dir.0.join("data")).env("XDG_CONFIG_HOME", dir.0.join("config"))
            .env("XDG_CACHE_HOME", dir.0.join("cache")).env("XDG_DATA_HOME", dir.0.join("local"))
            .env("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1") // isolated root-owned CI container only
            .env("WEBKIT_DISABLE_COMPOSITING_MODE", "1").env("LIBGL_ALWAYS_SOFTWARE", "1")
            .stdout(log.try_clone().unwrap()).stderr(log.try_clone().unwrap());
        command
    };
    // A tiny generated clip makes startup load a real project and render its first
    // frame through the native viewer cache; no user media is involved.
    let engine = kimo_gui::engine::Supervisor::new(Some(PathBuf::from(std::env::var_os("KIMO_MCP_BIN").unwrap())),
        vec![("KIMO_SPECTATOR".into(), "off".into()), ("KIMO_INLINE_IMAGES".into(), "0".into())], None);
    assert!(matches!(engine.start(), kimo_gui::engine::Status::Running { .. }));
    let call = |tool: &str, action: &str, args: serde_json::Value| {
        let reply = engine.call(tool, &serde_json::json!({"action":action,"args":args})).unwrap();
        assert!(!kimo_gui::engine::is_error(&reply), "{reply}");
        serde_json::from_str::<serde_json::Value>(&kimo_gui::engine::result_text(&reply)).unwrap_or_else(|error| panic!("{action}: {error}: {reply}"))
    };
    call("project", "project_set", serde_json::json!({"width":64,"height":64,"fps":24}));
    let track = call("timeline", "add_track", serde_json::json!({}))["track_id"].clone();
    call("timeline", "add_clip", serde_json::json!({"track_id":track,"source_type":"solid","color":[0.2,0.3,0.5],"start":0,"duration":1}));
    let project = dir.0.join("owned.kimo");
    call("project", "save_project", serde_json::json!({"path":project}));
    engine.stop();
    let mut app = Process(command().arg(&project).spawn().unwrap());
    let display_name = CString::new(screen_name.as_str()).unwrap();
    let display = Display(unsafe { XOpenDisplay(display_name.as_ptr()) }); assert!(!display.0.is_null());
    let deadline = Instant::now() + Duration::from_secs(40);
    let window = loop {
        assert!(app.0.try_wait().unwrap().is_none(), "{}", std::fs::read_to_string(&log_path).unwrap());
        if let Some(window) = display.main_window() { if dir.0.join("data/bridge.json").is_file() { break window; } }
        assert!(Instant::now() < deadline, "window did not initialize: {}", std::fs::read_to_string(&log_path).unwrap());
        std::thread::sleep(Duration::from_millis(100));
    };
    let deadline = Instant::now() + Duration::from_secs(40);
    let first_frame = loop {
        let frame = std::fs::read_dir(dir.0.join("data/viewer-cache")).ok().into_iter().flatten()
            .filter_map(Result::ok).map(|entry| entry.path()).find(|path| path.extension().is_some_and(|extension| extension == "png"));
        if let Some(frame) = frame { if std::fs::read(&frame).is_ok_and(|bytes| bytes.starts_with(b"\x89PNG\r\n\x1a\n")) { break frame; } }
        assert!(app.0.try_wait().unwrap().is_none(), "{}", std::fs::read_to_string(&log_path).unwrap());
        assert!(Instant::now() < deadline, "initial project frame was not rendered: {}", std::fs::read_to_string(&log_path).unwrap());
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(first_frame.starts_with(dir.0.join("data/viewer-cache")));
    let mut second = Process(command().args(["--owned-probe", dir.0.join("missing.kimo").to_str().unwrap()]).spawn().unwrap());
    wait_success(&mut second, &log_path);
    assert!(app.0.try_wait().unwrap().is_none(), "the second instance must join the first");
    display.close_window(window);
    wait_success(&mut app, &log_path);
    assert!(!dir.0.join("data/bridge.json").exists());
    let recovery = kimo_gui::recovery::load(&dir.0.join("data"));
    assert!(recovery.clean);
    assert_eq!(recovery.project_path.as_deref(), project.to_str());
}
