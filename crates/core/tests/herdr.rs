//! M6 acceptance (PLAN §9.2, §12): herdr behind a swappable adapter, version
//! pinned, never a gate.
//!
//! herdr is a terminal agent multiplexer (herdrdev/herdr). Backspace talks to
//! it through one `Supervisor` interface that also has a native
//! implementation, so herdr can be swapped out or switched off. Only the
//! pinned release is used: any other version, a missing binary or a dead
//! socket falls back to native with a notice. What herdr reports about an
//! agent (idle, working, blocked, done) is a signal for the human to look at,
//! never a gate.
//!
//! The real herdr is not installed here (its download host is not reachable
//! from this environment), so these tests run against a stand-in that speaks
//! the documented protocol: `herdr --version`, and newline-delimited JSON
//! over the Unix socket with `ping` and `agent.list`.

use std::path::{Path, PathBuf};

use backspace_core::herdr::{self, HerdrConfig, Signal, Supervisor};

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bs-herdr-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[cfg(unix)]
fn fake_bin(dir: &Path, version_line: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("herdr");
    std::fs::write(&p, format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ] || [ \"$1\" = \"-V\" ]; then echo '{version_line}'; exit 0; fi\nexit 2\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn versions_are_read_from_herdrs_own_output() {
    assert_eq!(herdr::parse_version("herdr 0.9.5\n").as_deref(), Some("0.9.5"));
    assert_eq!(herdr::parse_version("herdr v0.9.5 (abc123)").as_deref(), Some("0.9.5"));
    assert_eq!(herdr::parse_version("nothing here"), None);
}

#[test]
fn herdr_is_off_by_default_and_pinned_to_one_release() {
    let c = HerdrConfig::default();
    assert!(!c.enabled);
    assert_eq!(c.version, herdr::PINNED);
    assert_eq!(herdr::PINNED, "0.9.5");
    let cfg = backspace_core::Config::parse(backspace_core::config::DEFAULT_CONFIG).unwrap();
    assert_eq!(cfg.herdr, HerdrConfig::default());
}

#[test]
fn switched_off_the_native_supervisor_runs() {
    let (s, notice) = herdr::select(&HerdrConfig::default());
    assert_eq!(s.name(), "native");
    assert!(notice.is_none());
}

#[test]
fn a_missing_herdr_falls_back_to_native_and_says_so() {
    let c = HerdrConfig { enabled: true, bin: "/nonexistent/herdr".into(), ..HerdrConfig::default() };
    let (s, notice) = herdr::select(&c);
    assert_eq!(s.name(), "native");
    assert!(notice.unwrap().contains("not found"));
}

#[cfg(unix)]
#[test]
fn any_release_but_the_pinned_one_is_refused() {
    let d = scratch("ver");
    let c = HerdrConfig { enabled: true, bin: fake_bin(&d, "herdr 0.9.6").display().to_string(), ..HerdrConfig::default() };
    let (s, notice) = herdr::select(&c);
    assert_eq!(s.name(), "native");
    let n = notice.unwrap();
    assert!(n.contains("0.9.6") && n.contains("0.9.5"), "{n}");
}

#[cfg(unix)]
fn fake_server(sock: &Path, agents: serde_json::Value) -> std::thread::JoinHandle<Vec<String>> {
    use std::io::{BufRead, BufReader, Write};
    let l = std::os::unix::net::UnixListener::bind(sock).unwrap();
    std::thread::spawn(move || {
        let mut methods = Vec::new();
        for _ in 0..2 {
            let (s, _) = l.accept().unwrap();
            let mut w = s.try_clone().unwrap();
            let mut line = String::new();
            BufReader::new(s).read_line(&mut line).unwrap();
            let req: serde_json::Value = serde_json::from_str(&line).unwrap();
            let method = req["method"].as_str().unwrap().to_string();
            let result = match method.as_str() {
                "ping" => serde_json::json!({"type": "pong"}),
                "agent.list" => agents.clone(),
                _ => serde_json::json!(null),
            };
            writeln!(w, "{}", serde_json::json!({"id": req["id"], "result": result})).unwrap();
            methods.push(method);
        }
        methods
    })
}

#[cfg(unix)]
#[test]
fn the_pinned_release_is_used_and_its_states_are_signals() {
    let d = scratch("ok");
    let sock = d.join("herdr.sock");
    let server = fake_server(&sock, serde_json::json!({"agents": [
        {"id": "w1:p1", "name": "t1", "status": "working"},
        {"id": "w1:p2", "name": "t2", "status": "blocked"},
        {"id": "w1:p3", "name": "t3", "status": "strange"}
    ]}));
    let c = HerdrConfig { enabled: true, bin: fake_bin(&d, "herdr 0.9.5").display().to_string(), socket: Some(sock.display().to_string()), ..HerdrConfig::default() };
    let (s, notice) = herdr::select(&c);
    assert_eq!(s.name(), "herdr 0.9.5", "{notice:?}");
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let signals = rt.block_on(s.signals()).unwrap();
    assert_eq!(signals, vec![
        ("t1".to_string(), Signal::Working),
        ("t2".to_string(), Signal::Blocked),
        ("t3".to_string(), Signal::Unknown),
    ]);
    assert_eq!(server.join().unwrap(), vec!["ping", "agent.list"]);
}

#[cfg(unix)]
#[test]
fn a_dead_socket_falls_back_to_native() {
    let d = scratch("dead");
    let c = HerdrConfig { enabled: true, bin: fake_bin(&d, "herdr 0.9.5").display().to_string(), socket: Some(d.join("none.sock").display().to_string()), ..HerdrConfig::default() };
    let (s, notice) = herdr::select(&c);
    assert_eq!(s.name(), "native");
    assert!(notice.unwrap().contains("socket"));
}

#[test]
fn the_native_supervisor_reports_no_signals() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let s: Box<dyn Supervisor> = Box::new(herdr::Native);
    assert!(rt.block_on(s.signals()).unwrap().is_empty());
}
