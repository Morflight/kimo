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
    listen_private_on(TcpListener::bind(("127.0.0.1", 0)), TcpListener::local_addr, sink)
}
fn listen_private_on(listener: std::io::Result<TcpListener>, address: fn(&TcpListener) -> std::io::Result<std::net::SocketAddr>, sink: Sink) -> std::io::Result<u16> {
    let l = listener?;
    let port = address(&l)?.port();
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
    listen_external_on(l, TcpListener::local_addr, sink, status)
}
fn listen_external_on(l: TcpListener, address: fn(&TcpListener) -> std::io::Result<std::net::SocketAddr>, sink: Sink, status: StatusSink) -> Result<External, String> {
    let port = address(&l).map_err(|e| e.to_string())?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let st2 = stop.clone();
    status(format!("listening 127.0.0.1:{port}"));
    std::thread::spawn(move || accept_external(&mut l.incoming(), &st2, port, sink, status));
    Ok(External { port, stop })
}

fn accept_external(incoming: &mut dyn Iterator<Item = std::io::Result<TcpStream>>, stop: &AtomicBool, port: u16, sink: Sink, status: StatusSink) {
    let n = AtomicU32::new(0);
    for s in incoming {
        if stop.load(Ordering::SeqCst) { break; }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::Mutex;
    use std::time::Duration;
    fn recorded_events() -> (Sink, std::sync::mpsc::Receiver<(Source, Value)>) {
        let (send, receive) = std::sync::mpsc::channel();
        (Arc::new(move |source, value| send.send((source, value)).unwrap()), receive)
    }
    fn recorded_status() -> (StatusSink, std::sync::mpsc::Receiver<String>) {
        let (send, receive) = std::sync::mpsc::channel();
        (Arc::new(move |value| send.send(value).unwrap()), receive)
    }
    #[test]
    fn bind_and_address_failures_release_the_port_without_announcing_a_listener() {
        fn unavailable(_: &TcpListener) -> std::io::Result<std::net::SocketAddr> { Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "fixture address unavailable")) }
        let (sink, events) = recorded_events(); let (status, statuses) = recorded_status();
        let error = listen_private_on(Err(std::io::Error::from(std::io::ErrorKind::AddrInUse)), TcpListener::local_addr, sink.clone()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
        let private = TcpListener::bind(("127.0.0.1", 0)).unwrap(); let private_address = private.local_addr().unwrap();
        assert_eq!(listen_private_on(Ok(private), unavailable, sink.clone()).unwrap_err().kind(), std::io::ErrorKind::NotConnected);
        assert!(TcpListener::bind(private_address).is_ok());
        let external = TcpListener::bind(("127.0.0.1", 0)).unwrap(); let external_address = external.local_addr().unwrap();
        assert_eq!(listen_external_on(external, unavailable, sink, status).err().as_deref(), Some("fixture address unavailable"));
        assert!(TcpListener::bind(external_address).is_ok());
        assert!(events.try_recv().is_err()); assert!(statuses.try_recv().is_err());
    }
    #[test]
    fn accept_and_greeting_failures_preserve_the_next_connection() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let failed = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (closed, _) = listener.accept().unwrap(); closed.shutdown(std::net::Shutdown::Write).unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (valid, _) = listener.accept().unwrap();
        client.write_all(b"{\"rev\":8}\n").unwrap(); client.shutdown(std::net::Shutdown::Write).unwrap();
        let (sink, events) = recorded_events(); let (status, statuses) = recorded_status();
        let mut incoming = vec![Err(std::io::Error::from(std::io::ErrorKind::Interrupted)), Ok(closed), Ok(valid)].into_iter();
        accept_external(&mut incoming, &AtomicBool::new(false), port, sink, status);
        let mut hello = [0; 17]; client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        client.read_exact(&mut hello).unwrap(); assert_eq!(&hello, HELLO);
        assert_eq!(events.recv_timeout(Duration::from_secs(2)).unwrap(), (Source::External { conn: 1 }, serde_json::json!({"rev":8})));
        assert_eq!(statuses.recv_timeout(Duration::from_secs(2)).unwrap(), "live");
        assert_eq!(statuses.recv_timeout(Duration::from_secs(2)).unwrap(), format!("listening 127.0.0.1:{port}"));
        assert!(events.try_recv().is_err()); drop(failed);
    }
    #[test]
    fn private_and_external_are_tagged() {
        let _ports = crate::testutil::PORTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _ports = crate::testutil::PORTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (sink, events) = recorded_events();
        let ext = listen_external(sink, Arc::new(|_| {})).unwrap();
        assert!(GUI_PORTS.contains(&ext.port));
        // nothing listens on the wildcard address for that port from this listener
        assert!(TcpListener::bind(("127.0.0.1", ext.port)).is_err());
        ext.stop();
        assert!(events.try_recv().is_err());
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
        let _ports = crate::testutil::PORTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut retry_fixture = Some(TcpListener::bind(("127.0.0.1", *GUI_PORTS.end())).unwrap());
        let held: Vec<_> = GUI_PORTS.clone().map(|port| loop {
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(listener) => break listener,
                Err(error) => {
                    drop(retry_fixture.take());
                    assert!(std::time::Instant::now() < deadline, "test requires free port {port}: {error}");
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }).collect();
        assert!(retry_fixture.is_none(), "the owned busy port must exercise the retry");
        let (sink, events) = recorded_events(); let (status, statuses) = recorded_status();
        let result = listen_external(sink, status);
        assert_eq!(result.err().as_deref(), Some("127.0.0.1:7421-7430 are all in use"));
        assert!(events.try_recv().is_err()); assert!(statuses.try_recv().is_err());
        drop(held);
    }

}
