//! Real CLI subprocesses: stdout/stderr, exit codes and isolated synthetic project files.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn cli(args: &[&str], input: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_kimo"))
        .args(args).env("KIMO_SPECTATOR", "off").env("KIMO_OBSERVE", "off")
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    if let Some(input) = input { child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap(); }
    child.wait_with_output().unwrap()
}
fn success(args: &[&str]) -> Value {
    let output = cli(args, None);
    assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}
struct Temp(PathBuf);
impl Temp {
    fn new(name: &str) -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("kimo-cli-contract-{}-{name}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn call(project: &Path, tool: &str, args: Value) -> Value {
    success(&["--json", "--quiet", "call", tool, "--project", project.to_str().unwrap(), "--args", &args.to_string()])
}

#[test]
fn identity_catalog_and_argument_aliases() {
    let info = success(&["--json", "info"]);
    for version in ["--version", "-V"] {
        assert_eq!(success(&["--json", version]), json!({"version":info["version"],"commit":info["commit"]}));
    }
    let names = success(&["tools"]);
    assert!(names["tools"].as_array().unwrap().iter().any(|name| name == "add_track"));
    let found = success(&["tools", "search", "blur", "--limit=2"]);
    assert!(!found["matches"].as_array().unwrap().is_empty());
    assert!(found["matches"].as_array().unwrap().len() <= 2);
    assert!(success(&["tools", "describe", "add_shape"])["call"]["tool"].is_string());
    for args in [vec!["-q", "call", "kimo_info", "--json", "{}"], vec!["--quiet", "--json", "call", "kimo_info", "--args={}"]] {
        let output = cli(&args, None);assert!(output.status.success());assert!(output.stderr.is_empty());
        assert_eq!(serde_json::from_slice::<Value>(&output.stdout).unwrap()["version"], info["version"]);
    }
    for flag in ["--args", "--json"] {
        let output = cli(&["--json", "-q", "call", "kimo_info", flag, "-"], Some("{}"));
        assert!(output.status.success());assert_eq!(serde_json::from_slice::<Value>(&output.stdout).unwrap()["version"], info["version"]);
    }
}

#[test]
fn usage_errors_are_not_tool_successes() {
    let cases: &[&[&str]] = &[
        &[], &["help"], &["-h"], &["unknown-command"], &["call"], &["call", "kimo_info", "extra"],
        &["call", "kimo_info", "--args", "{"], &["call", "kimo_info", "--json", "[]"], &["--project"],
        &["export"], &["export", "a"], &["export", "a", "b", "extra"], &["export", "a", "b", "--range", "invalid"],
        &["export", "a", "b", "--range", "bad:1"], &["export", "a", "b", "--range", "0:bad"],
        &["export", "a", "b", "--start", "bad"], &["export", "a", "b", "--end", "bad"], &["export", "a", "b", "--crf", "bad"],
        &["export", "a", "b", "--preset", "x", "--crf", "20"], &["render-job"], &["tools", "search"],
        &["tools", "search", "blur", "--limit", "bad"], &["tools", "describe"], &["tools", "unexpected"],
    ];
    for args in cases {
        let output = cli(args, None);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
        assert!(output.stdout.is_empty(), "usage errors do not print a successful result");
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
    }
    let output = cli(&["--json", "call", "kimo_info", "--args", "not-json"], None);
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "usage");
}

#[test]
fn tool_errors_and_unavailable_commands_keep_their_exit_code() {
    for args in [vec!["--json", "-q", "call", "no_such_tool"], vec!["--json", "-q", "call", "add_shape", "--args", "{}"], vec!["--json", "tools", "describe", "no_such_tool"], vec!["--json", "render-job", "no-such-subcommand"]] {
        let output = cli(&args, None);assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(output.stdout.is_empty());let value: Value=serde_json::from_slice(&output.stderr).unwrap();assert!(value["error"]["code"].is_string());assert!(value["error"]["message"].is_string());
    }
    let temp=Temp::new("missing");let project=temp.0.join("missing.kimo");
    for range in [":", "0:", ":1", "0:1"] {
        let output=cli(&["--json","export",project.to_str().unwrap(),"out.mp4","--range",range],None);
        assert_eq!(output.status.code(),Some(1));let error:Value=serde_json::from_slice(&output.stderr).unwrap();assert_eq!(error["error"]["code"],"io_error");
    }
}

#[test]
fn mutating_calls_save_atomically_and_no_save_preserves_bytes() {
    let temp=Temp::new("save");let project=temp.0.join("project.kimo");
    let output=cli(&["call","add_track","--project",project.to_str().unwrap()],None);
    assert!(output.status.success());assert!(String::from_utf8_lossy(&output.stderr).contains("kimo: created"));
    let track:Value=serde_json::from_slice(&output.stdout).unwrap();let before=std::fs::read(&project).unwrap();
    success(&["-q","call","add_track","--project",project.to_str().unwrap(),"--no-save"]);
    assert_eq!(std::fs::read(&project).unwrap(),before);
    call(&project,"kimo_info",json!({}));assert_eq!(std::fs::read(&project).unwrap(),before);
    let group=success(&["tools","describe","add_shape"])["call"]["tool"].as_str().unwrap().to_string();
    call(&project,&group,json!({"action":"add_shape","args":{"track_id":track["track_id"],"shape":"rect","width":20,"height":20,"start":0,"duration":1}}));
    assert_ne!(std::fs::read(&project).unwrap(),before);
    assert_eq!(std::fs::read_dir(&temp.0).unwrap().count(),1,"atomic save leaves no temporary file");
    let invalid=temp.0.join("invalid.kimo");std::fs::write(&invalid,"not json").unwrap();
    assert_eq!(cli(&["call","kimo_info","--project",invalid.to_str().unwrap()],None).status.code(),Some(1));
}

#[test]
fn export_uses_the_requested_range_and_produces_a_real_file() {
    let temp=Temp::new("export");let project=temp.0.join("project.kimo");let output=temp.0.join("out.mp4");
    call(&project,"project_set",json!({"width":320,"height":180,"fps":25}));
    let track=call(&project,"add_track",json!({}));
    call(&project,"add_clip",json!({"track_id":track["track_id"],"source_type":"solid","color":[1,0,0],"start":0,"duration":1}));
    let result=success(&["--json","-q","export",project.to_str().unwrap(),output.to_str().unwrap(),"--start","0","--end","0.2","--crf","25"]);
    assert_eq!(result["path"],output.to_str().unwrap());assert!(std::fs::metadata(&output).unwrap().len()>0);
    let probe=Command::new("ffprobe").args(["-v","error","-count_frames","-select_streams","v:0","-show_entries","stream=nb_read_frames,width,height","-of","json"]).arg(&output).output().unwrap();
    assert!(probe.status.success());let probe:Value=serde_json::from_slice(&probe.stdout).unwrap();
    assert_eq!(probe["streams"][0]["nb_read_frames"],"5");assert_eq!(probe["streams"][0]["width"],320);assert_eq!(probe["streams"][0]["height"],180);
}
