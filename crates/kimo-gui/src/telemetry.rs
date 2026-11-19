//! Live agent activity. kimo-mcp streams every tool call as JSON lines over loopback TCP (crates/kimo-mcp/src/spectator.rs).
//! Two listeners tell the sources apart without any engine change:
//!   * the GUI's own engine is started with KIMO_SPECTATOR=tcp://127.0.0.1:<private port> (exact port, no hello), so
//!     everything on that port is the GUI agent's;
//!   * external MCP clients (Claude Code, Codex, ...) launch their own kimo-mcp, which by default scans
//!     127.0.0.1:7421-7430 and streams to whoever greets it with `KIMO-SPECTATOR/1`; each such connection is one
//!     external engine. This listener runs only while the MCP switch is ON.
//! Loopback only, never 0.0.0.0. Many connections at once (one thread each).
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

pub const GUI_PORTS: std::ops::RangeInclusive<u16> = 7421..=7430;
pub const HELLO: &[u8] = b"KIMO-SPECTATOR/1\n";

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Source { Gui, External { conn: u32 } }

pub type Sink = Arc<dyn Fn(Source, Value) + Send + Sync>;
pub type StatusSink = Arc<dyn Fn(String) + Send + Sync>;

fn pump(s: TcpStream, src: Source, sink: Sink, status: Option<StatusSink>) {
    for line in BufReader::new(s).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() { continue; }
        match serde_json::from_str::<Value>(&line) {
            Ok(v) => sink(src.clone(), v),
            Err(e) => if let Some(st) = &status { st(format!("bad json: {e}")) },
        }
    }
}

/// The private port for the GUI's own engine. Returns the port; the accept loop runs on its own thread.
pub fn listen_private(sink: Sink) -> std::io::Result<u16> {
    let l = TcpListener::bind(("127.0.0.1", 0))?;
    let port = l.local_addr()?.port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let sink = sink.clone();
            std::thread::spawn(move || pump(s, Source::Gui, sink, None));
        }
    });
    Ok(port)
}

/// The public spectator listener for external engines. `stop` ends it (checked between accepts).
pub struct External { pub port: u16, stop: Arc<AtomicBool> }
impl External {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port)); // wake the accept
    }
}

pub fn listen_external(sink: Sink, status: StatusSink) -> Result<External, String> {
    let l = GUI_PORTS.clone().find_map(|p| TcpListener::bind(("127.0.0.1", p)).ok())
        .ok_or_else(|| "127.0.0.1:7421-7430 are all in use".to_string())?;
    let port = l.local_addr().map_err(|e| e.to_string())?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let st2 = stop.clone();
    let n = Arc::new(AtomicU32::new(0));
    status(format!("listening 127.0.0.1:{port}"));
    std::thread::spawn(move || {
        for s in l.incoming() {
            if st2.load(Ordering::SeqCst) { break; }
            let Ok(mut s) = s else { continue };
            if s.write_all(HELLO).is_err() { continue; }
            let conn = n.fetch_add(1, Ordering::SeqCst) + 1;
            let (sink, status) = (sink.clone(), status.clone());
            status("live".into());
            std::thread::spawn(move || {
                pump(s, Source::External { conn }, sink, Some(status.clone()));
                status(format!("listening 127.0.0.1:{port}"));
            });
        }
    });
    Ok(External { port, stop })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::Mutex;
    use std::time::Duration;
    #[test]
    fn private_and_external_are_tagged() {
        let _ports = crate::testutil::PORTS.lock().unwrap_or_else(|e| e.into_inner());
        let got: Arc<Mutex<Vec<(Source, Value)>>> = Arc::new(Mutex::new(vec![]));
        let g2 = got.clone();
        let sink: Sink = Arc::new(move |s, v| g2.lock().unwrap().push((s, v)));
        let p = listen_private(sink.clone()).unwrap();
        let ext = listen_external(sink, Arc::new(|_| {})).unwrap();
        let mut a = TcpStream::connect(("127.0.0.1", p)).unwrap();
        a.write_all(b"{\"tool_name\":\"split_clip\",\"phase\":\"start\"}\n").unwrap();
        // two external engines at once, each greeted with the hello the engine waits for
        let mut b = TcpStream::connect(("127.0.0.1", ext.port)).unwrap();
        let mut c = TcpStream::connect(("127.0.0.1", ext.port)).unwrap();
        for s in [&mut b, &mut c] {
            let mut hello = [0u8; 17];
            s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            s.read_exact(&mut hello).unwrap();
            assert_eq!(&hello, HELLO);
        }
        b.write_all(b"{\"tool_name\":\"color_grade\",\"phase\":\"complete\"}\nnot json\n").unwrap();
        c.write_all(b"{\"type\":\"project_changed\",\"rev\":3}\n").unwrap();
        drop((a, b, c));
        std::thread::sleep(Duration::from_millis(300));
        let g = got.lock().unwrap();
        assert_eq!(g.len(), 3, "{g:?}");
        assert!(g.iter().any(|(s, v)| *s == Source::Gui && v["tool_name"] == "split_clip"));
        let ext_conns: std::collections::HashSet<u32> = g.iter().filter_map(|(s, _)| match s { Source::External { conn } => Some(*conn), _ => None }).collect();
        assert_eq!(ext_conns.len(), 2, "two external engines, two connection ids");
        ext.stop();
    }
    #[test]
    fn binds_loopback_only() {
        let _ports = crate::testutil::PORTS.lock().unwrap_or_else(|e| e.into_inner());
        let ext = listen_external(Arc::new(|_, _| {}), Arc::new(|_| {})).unwrap();
        assert!(GUI_PORTS.contains(&ext.port));
        // nothing listens on the wildcard address for that port from this listener
        assert!(TcpListener::bind(("127.0.0.1", ext.port)).is_err());
        ext.stop();
    }
    #[test]
    fn malformed_and_blank_lines_do_not_replace_valid_events() {
        for report_errors in [false, true] {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server, _) = listener.accept().unwrap();
            let events = Arc::new(Mutex::new(Vec::new()));
            let errors = Arc::new(Mutex::new(Vec::new()));
            let received = events.clone();
            let reported = errors.clone();
            let sink: Sink = Arc::new(move |source, value| received.lock().unwrap().push((source, value)));
            let status: StatusSink = Arc::new(move |message| reported.lock().unwrap().push(message));
            let worker = std::thread::spawn(move || pump(server, Source::External { conn: 7 }, sink, report_errors.then_some(status)));
            client.write_all(b"  \nnot json\n{\"rev\":4}\n\xff\n{\"rev\":5}\n").unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            worker.join().unwrap();
            assert_eq!(*events.lock().unwrap(), vec![(Source::External { conn: 7 }, serde_json::json!({"rev": 4}))]);
            let errors = errors.lock().unwrap();
            assert_eq!(errors.len(), usize::from(report_errors));
            if report_errors { assert!(errors[0].starts_with("bad json:")); }
        }
    }

    #[test]
    fn all_public_ports_occupied_is_an_error_without_starting_a_listener() {
        let _ports = crate::testutil::PORTS.lock().unwrap_or_else(|e| e.into_inner());
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let held: Vec<_> = GUI_PORTS.clone().map(|port| loop {
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(listener) => break listener,
                Err(error) => {
                    assert!(std::time::Instant::now() < deadline, "test requires free port {port}: {error}");
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }).collect();
        let result = listen_external(Arc::new(|_, _| panic!("no listener should deliver events")), Arc::new(|_| panic!("no listener should report live status")));
        assert_eq!(result.err().as_deref(), Some("127.0.0.1:7421-7430 are all in use"));
        drop(held);
    }

}
