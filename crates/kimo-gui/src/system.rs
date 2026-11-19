//! Tools this computer already has. kimo never downloads a part the user already installed: FFmpeg / FFprobe from
//! PATH or the usual install places (winget, Chocolatey, Scoop, C:\ffmpeg, Program Files, Homebrew, /usr/bin),
//! whisper.cpp's `whisper-cli`, and a Whisper model already on disk (KIMO_WHISPER_MODEL, or ~/.cache/kimo_models,
//! where the engine's own fetch script puts it). Found tools are handed to the engine by exact path, so they work even
//! when they are not on PATH.
use std::path::{Path, PathBuf};

/// The model files the engine accepts, best first (same list as kimo-core's auto_captions).
pub const MODELS: [&str; 4] = ["ggml-large-v3-turbo-q5_0.bin", "ggml-small-q5_1.bin", "ggml-small.bin", "ggml-base-q5_1.bin"];

fn exe(name: &str) -> String { exe_for(name, cfg!(windows)) }
fn exe_for(name: &str, windows: bool) -> String { if windows { format!("{name}.exe") } else { name.to_string() } }

fn env_dir(key: &str) -> Option<PathBuf> { std::env::var_os(key).map(PathBuf::from) }

fn home() -> Option<PathBuf> { crate::agents::home_dir() }

/// Folders where FFmpeg usually lands when it is installed but not on PATH.
fn ffmpeg_dirs() -> Vec<PathBuf> {
    ffmpeg_dirs_for(cfg!(windows), &env_dir, home())
}

fn ffmpeg_dirs_for(windows: bool, env: &dyn Fn(&str) -> Option<PathBuf>, home: Option<PathBuf>) -> Vec<PathBuf> {
    let mut v = vec![];
    if windows {
        if let Some(l) = env("LOCALAPPDATA") {
            let l = l.join("Microsoft").join("WinGet");
            v.push(l.join("Links"));
            // winget's portable FFmpeg: Packages\<id>\ffmpeg-<build>\bin
            if let Ok(rd) = std::fs::read_dir(l.join("Packages")) {
                for pkg in rd.flatten().filter(|e| e.file_name().to_string_lossy().to_ascii_lowercase().contains("ffmpeg")) {
                    v.push(pkg.path().join("bin"));
                    if let Ok(inner) = std::fs::read_dir(pkg.path()) { for d in inner.flatten() { v.push(d.path().join("bin")); } }
                }
            }
        }
        if let Some(p) = env("ProgramData") { v.push(p.join("chocolatey").join("bin")); }
        if let Some(h) = home { v.push(h.join("scoop").join("shims")); v.push(h.join("scoop").join("apps").join("ffmpeg").join("current").join("bin")); }
        for pf in ["ProgramFiles", "ProgramFiles(x86)"] { if let Some(p) = env(pf) { v.push(p.join("ffmpeg").join("bin")); v.push(p.join("FFmpeg").join("bin")); } }
        v.push(PathBuf::from(r"C:\ffmpeg\bin"));
        v.push(PathBuf::from(r"C:\Program Files\ffmpeg\bin"));
    } else {
        v.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/snap/bin", "/opt/local/bin"].map(PathBuf::from));
        if let Some(h) = home { v.push(h.join(".local").join("bin")); }
    }
    v
}

fn first_file(dirs: &[PathBuf], file: &str) -> Option<PathBuf> { dirs.iter().map(|d| d.join(file)).find(|p| p.is_file()) }

/// The engine's own models folder: $KIMO_MODELS, else ~/.cache/kimo_models.
pub fn models_dir() -> Option<PathBuf> {
    env_dir("KIMO_MODELS").or_else(|| home().map(|h| h.join(".cache").join("kimo_models")))
}

/// Pick the best model file in a folder.
pub fn model_in(dir: &Path) -> Option<PathBuf> { MODELS.iter().map(|m| dir.join(m)).find(|p| p.is_file()) }

fn whisper_fallback(windows: bool, dirs: impl FnOnce() -> Vec<PathBuf>) -> Option<PathBuf> {
    if windows { None } else { first_file(&dirs(), "whisper-cli") }
}

/// A part already on this computer (outside kimo's own download folder), by part id.
pub fn find(id: &str) -> Option<PathBuf> {
    match id {
        "ffmpeg" | "ffprobe" => crate::agents::which(id).or_else(|| first_file(&ffmpeg_dirs(), &exe(id))),
        "whisper" => std::env::var_os("KIMO_WHISPER_BIN").map(PathBuf::from).filter(|p| p.is_file())
            .or_else(|| crate::agents::which("whisper-cli"))
            .or_else(|| crate::agents::which("whisper-cpp"))
            .or_else(|| models_dir().map(|d| d.join("bin").join(exe("whisper-cli"))).filter(|p| p.is_file()))
            .or_else(|| whisper_fallback(cfg!(windows), ffmpeg_dirs)),
        "model" => std::env::var_os("KIMO_WHISPER_MODEL").map(PathBuf::from).filter(|p| p.is_file())
            .or_else(|| models_dir().and_then(|d| model_in(&d))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picks_the_best_model_and_the_first_existing_tool() {
        let d = std::env::temp_dir().join(format!("kimo-sys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::create_dir_all(d.join("b")).unwrap();
        assert!(model_in(&d).is_none());
        std::fs::write(d.join("ggml-base-q5_1.bin"), b"x").unwrap();
        std::fs::write(d.join("ggml-small-q5_1.bin"), b"x").unwrap();
        assert_eq!(model_in(&d).unwrap().file_name().unwrap(), "ggml-small-q5_1.bin", "the better model wins");
        std::fs::write(d.join("b").join(exe("ffmpeg")), b"x").unwrap();
        assert_eq!(first_file(&[d.join("a"), d.join("b")], &exe("ffmpeg")).unwrap(), d.join("b").join(exe("ffmpeg")));
        assert!(find("nothing").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn platform_search_paths_include_portable_packages_without_cross_platform_assumptions() {
        assert_eq!(whisper_fallback(true, ffmpeg_dirs), None);
        assert_eq!(whisper_fallback(false, ffmpeg_dirs), first_file(&ffmpeg_dirs(), "whisper-cli"));
        assert_eq!(exe_for("ffmpeg", true), "ffmpeg.exe");
        assert_eq!(exe_for("ffmpeg", false), "ffmpeg");
        let root = std::env::temp_dir().join(format!("kimo-system-paths-{}", std::process::id()));
        let package = root.join("Microsoft/WinGet/Packages/vendor.FFmpeg/build");
        std::fs::create_dir_all(&package).unwrap();
        let whisper = package.join("whisper-cli");
        std::fs::write(&whisper, b"native discovery fixture").unwrap();
        assert_eq!(whisper_fallback(false, || vec![root.join("absent"), package.clone()]), Some(whisper.clone()));
        std::fs::remove_file(&whisper).unwrap();
        assert_eq!(whisper_fallback(false, || vec![package.clone()]), None);

        std::fs::create_dir_all(root.join("Microsoft/WinGet/Packages/unrelated")).unwrap();
        std::fs::write(root.join("Microsoft/WinGet/Packages/ffmpeg-file"), b"not a directory").unwrap();
        let dirs = ffmpeg_dirs_for(true, &|_| Some(root.clone()), Some(root.clone()));
        assert!(dirs.contains(&package.join("bin")));
        assert!(dirs.contains(&root.join("Microsoft/WinGet/Links")));
        assert!(dirs.contains(&root.join("chocolatey/bin")));
        assert!(dirs.contains(&root.join("scoop/shims")));
        assert!(!dirs.iter().any(|p| p.to_string_lossy().contains("unrelated")));
        let absent = ffmpeg_dirs_for(true, &|_| None, None);
        assert_eq!(absent, vec![PathBuf::from(r"C:\ffmpeg\bin"), PathBuf::from(r"C:\Program Files\ffmpeg\bin")]);
        assert_eq!(ffmpeg_dirs_for(false, &|_| None, None).len(), 5);
        assert_eq!(ffmpeg_dirs_for(false, &|_| None, Some(root.clone())).last(), Some(&root.join(".local/bin")));
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(ffmpeg_dirs_for(true, &|k| if k == "LOCALAPPDATA" { Some(root.clone()) } else { None }, None).len(), 3);
    }

    #[test]
    fn discovery_child() {
        let Ok(mode) = std::env::var("KIMO_SYSTEM_TEST_MODE") else { return; };
        let root = PathBuf::from(std::env::var_os("KIMO_SYSTEM_TEST_ROOT").unwrap());
        if mode == "fallback" {
            assert_eq!(models_dir(), home().map(|h| h.join(".cache/kimo_models")));
            let expected = first_file(&ffmpeg_dirs(), &exe("ffprobe"));
            assert_eq!(find("ffprobe"), expected, "outside PATH, only documented install folders are searched");
            assert_eq!(find("model"), Some(root.join("models").join(MODELS[0])));
            return;
        }
        assert_eq!(models_dir(), Some(root.join("models")));
        let found = find("whisper");
        let expected = match mode.as_str() {
            "explicit" => root.join("explicit"),
            "path-cli" => root.join("path").join(exe("whisper-cli")),
            "path-cpp" => root.join("path").join(exe("whisper-cpp")),
            "models" => root.join("models/bin").join(exe("whisper-cli")),
            _ => { assert_eq!(mode, "missing"); assert!(found.is_none()); return; }
        };
        assert_eq!(found, Some(expected));
        assert_eq!(find("model"), Some(root.join("models").join(MODELS[0])));
        assert_eq!(find("ffmpeg"), Some(root.join("path").join(exe("ffmpeg"))));
    }

    #[test]
    fn discovery_respects_explicit_path_and_model_precedence_in_isolated_processes() {
        let root = std::env::temp_dir().join(format!("kimo-system-find-{}", std::process::id()));
        std::fs::create_dir_all(root.join("path")).unwrap();
        std::fs::create_dir_all(root.join("models/bin")).unwrap();
        std::fs::write(root.join("models").join(MODELS[0]), b"model fixture").unwrap();
        let executable = |p: &Path| {
            std::fs::write(p, b"fixture").unwrap();
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700)).unwrap(); }
        };
        executable(&root.join("path").join(exe("ffmpeg")));
        for mode in ["explicit", "path-cli", "path-cpp", "models", "missing"] {
            let file = match mode {
                "explicit" => root.join("explicit"),
                "path-cli" => root.join("path").join(exe("whisper-cli")),
                "path-cpp" => root.join("path").join(exe("whisper-cpp")),
                _ => root.join("models/bin").join(exe("whisper-cli")),
            };
            if mode != "missing" { executable(&file); }
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "system::tests::discovery_child", "--nocapture"])
                .env("KIMO_SYSTEM_TEST_MODE", mode).env("KIMO_SYSTEM_TEST_ROOT", &root)
                .env("KIMO_MODELS", root.join("models")).env("KIMO_WHISPER_BIN", root.join("explicit"))
                .env("KIMO_WHISPER_MODEL", root.join("absent-model"))
                .env("PATH", root.join("path")).output().unwrap();
            assert!(result.status.success(), "{mode}: {} {}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
            if mode != "missing" { std::fs::remove_file(file).unwrap(); }
        }
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "system::tests::discovery_child", "--nocapture"])
            .env("KIMO_SYSTEM_TEST_MODE", "fallback").env("KIMO_SYSTEM_TEST_ROOT", &root)
            .env_remove("KIMO_MODELS").env("KIMO_WHISPER_MODEL", root.join("models").join(MODELS[0]))
            .env("PATH", root.join("path")).output().unwrap();
        assert!(result.status.success(), "{} {}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        std::fs::remove_dir_all(root).unwrap();
    }

}
