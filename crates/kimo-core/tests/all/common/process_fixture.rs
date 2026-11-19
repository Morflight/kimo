//! Native child-process fixture; no shell, Unix utilities or inherited grandchildren.
use std::{path::PathBuf,process::Command,sync::OnceLock};
pub(crate) fn command(args: &[&str]) -> Command {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    let binary=BINARY.get_or_init(|| {
        let dir=std::env::temp_dir().join(format!("kimo-process-fixture-{}",std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source=dir.join("child.rs");let binary=dir.join("child").with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&source,r#"
use std::io::{Read,Write};
fn main() {
 let args:Vec<String>=std::env::args().skip(1).collect();
 for pair in args.chunks_exact(2) {
  match pair[0].as_str() {
   "stdout" => {std::io::stdout().write_all(pair[1].as_bytes()).unwrap();std::io::stdout().flush().unwrap();},
   "stderr" => {std::io::stderr().write_all(pair[1].as_bytes()).unwrap();},
   "stderr-zero" => {std::io::stderr().write_all(&vec![0;pair[1].parse().unwrap()]).unwrap();},
   "pid" => std::fs::write(&pair[1],std::process::id().to_string()).unwrap(),
   "sleep" => std::thread::sleep(std::time::Duration::from_millis(pair[1].parse().unwrap())),
   "exit" => std::process::exit(pair[1].parse().unwrap()),
   "copy" => {let mut data=Vec::new();std::io::stdin().read_to_end(&mut data).unwrap();std::fs::write(&pair[1],data).unwrap();},
   other => panic!("unknown fixture action: {other}"),
  }
 }
}
"#).unwrap();
        let result=Command::new("rustc").args(["--edition=2021","--crate-name","process_fixture"]).arg(source).arg("-o").arg(&binary).output().unwrap();
        assert!(result.status.success(),"fixture compilation: {}",String::from_utf8_lossy(&result.stderr));binary
    });
    let mut command=Command::new(binary);command.args(args);command
}

pub(crate) fn assert_dead(pid: u32) {
    if cfg!(windows) {
        let result=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",&format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ 'running' }} else {{ 'stopped' }}")]).output().unwrap();
        assert!(result.status.success(),"process status query failed: {}",String::from_utf8_lossy(&result.stderr));
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(),"stopped","fixture process {pid} is still running");
    } else { assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists(),"fixture process {pid} is still running"); }
}
