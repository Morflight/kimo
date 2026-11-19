//! Exercise the shipped executable's early entry points without opening a desktop.
use std::{path::PathBuf, process::Command};

struct OwnedDir(PathBuf);
impl OwnedDir {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("kimo-entry-{label}-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kimo-gui"));
        command.env("KIMO_DATA_DIR", &self.0);
        command
    }
}
impl Drop for OwnedDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

#[test]
fn version_exits_without_initializing_application_state() {
    let dir = OwnedDir::new("version");
    let output = dir.command().arg("--version").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), format!("kimo-gui {}", env!("CARGO_PKG_VERSION")));
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[test]
fn deliberate_crash_writes_one_owned_report_before_desktop_startup() {
    let dir = OwnedDir::new("crash");
    let output = dir.command().arg("--crash-test").output().unwrap();
    assert!(!output.status.success());
    let reports = kimo_gui::crash::pending(&dir.0.join("crashes"));
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].message, "kimo --crash-test: a deliberate crash to check crash reports");
    assert!(reports[0].text.contains(&format!("version: {}", env!("CARGO_PKG_VERSION"))));
    assert!(reports[0].text.contains("thread: main"));
    assert!(reports[0].text.contains("backtrace:"));
    assert!(String::from_utf8_lossy(&output.stderr).contains(&reports[0].message));
    assert!(!dir.0.join("bridge.json").exists());
    assert!(!dir.0.join("engine.log").exists());
}
