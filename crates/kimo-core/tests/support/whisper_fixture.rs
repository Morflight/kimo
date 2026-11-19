//! Native recognizer fixture: identical JSON and argument recording on Linux and Windows.
use std::path::{Path, PathBuf};

pub fn build(path: &Path, response: &str, auto_language: &str, log: Option<&Path>) -> PathBuf {
    let binary=path.with_extension(std::env::consts::EXE_EXTENSION);
    let source=path.with_extension("rs");
    let log=log.map(|p|p.display().to_string()).unwrap_or_default();
    let code=format!(r#"
use std::io::Write;
fn main() {{
    let args:Vec<String>=std::env::args().skip(1).collect();
    let log={log:?};
    if !log.is_empty() {{
        let mut file=std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap();
        writeln!(file,"{{}}",args.join(" ")).unwrap();
    }}
    let value=|key:&str|args.windows(2).find(|pair|pair[0]==key).map(|pair|pair[1].as_str());
    let requested=value("-l").unwrap_or("auto");
    let language=if requested=="auto" {{{auto_language:?}}} else {{requested}};
    let output=value("-of").expect("recognizer output prefix");
    let response={response:?}.replace("__LANG__",language);
    std::fs::write(format!("{{output}}.json"),response).unwrap();
}}
"#);
    std::fs::write(&source,code).unwrap();
    let compiled=std::process::Command::new("rustc").args(["--edition=2021","--crate-name","whisper_fixture"]).arg(&source).arg("-o").arg(&binary).output().unwrap();
    assert!(compiled.status.success(),"fixture compilation: {}",String::from_utf8_lossy(&compiled.stderr));
    binary
}
