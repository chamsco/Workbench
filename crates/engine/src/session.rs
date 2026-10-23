//! Models in memory: one `llama-server` per loaded model, started on the first
//! request for it and stopped when it has sat idle, or when another model
//! needs the room.
//!
//! The mechanics follow Atomic Chat's (atomic-chat-core PLAN.md §8.2):
//! readiness is `GET /health` answering 2xx, an early exit is classified from
//! the log with the same substring cascade, and a session is the process plus
//! its port. Two requests for the same cold model start it once.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Ports from here up; Backspace's own sit at 742x.
pub const FIRST_PORT: u16 = 8090;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Context per model, capped at what the model was trained on.
    pub ctx: u64,
    /// Unload a model nobody used for this long. 0: never.
    pub idle_minutes: u64,
    /// Models in memory at once; the least recently used goes first.
    pub max_loaded: usize,
    /// Layers on the GPU; 99 means all of them.
    pub gpu_layers: u32,
    /// Seconds a model may take to load before it is given up on.
    pub load_timeout: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ctx: 8192,
            idle_minutes: 15,
            max_loaded: 1,
            gpu_layers: 99,
            load_timeout: 180,
        }
    }
}

/// What a session needs to know about its model.
#[derive(Clone, Debug)]
pub struct Spec {
    pub id: String,
    pub path: PathBuf,
    /// The trained context, when the file says.
    pub ctx_train: Option<u64>,
    pub embedding: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SessionInfo {
    pub model: String,
    pub path: String,
    pub port: u16,
    pub base_url: String,
    pub pid: u32,
    /// Seconds since the last request.
    pub idle_secs: u64,
    /// Kept in memory whatever the idle timer says.
    pub pinned: bool,
    pub log: String,
}

/// Why a model did not come up, in Atomic's codes.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LoadError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for LoadError {}

struct Session {
    info: SessionInfo,
    child: Child,
    last: Instant,
}

pub struct Supervisor {
    logs: PathBuf,
    settings: Mutex<Settings>,
    sessions: Mutex<HashMap<String, Session>>,
    /// One lock per model, so a cold model is started once however many ask.
    starting: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    pinned: Mutex<Vec<String>>,
}

impl Supervisor {
    pub fn new(logs: PathBuf, settings: Settings) -> Self {
        Self {
            logs,
            settings: Mutex::new(settings),
            sessions: Mutex::new(HashMap::new()),
            starting: Mutex::new(HashMap::new()),
            pinned: Mutex::new(vec![]),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn set_settings(&self, s: Settings) {
        *self.settings.lock().unwrap() = s;
    }

    pub fn set_pinned(&self, ids: Vec<String>) {
        *self.pinned.lock().unwrap() = ids;
    }

    /// The running sessions; one that died is dropped here.
    pub fn sessions(&self) -> Vec<SessionInfo> {
        let pinned = self.pinned.lock().unwrap().clone();
        let mut s = self.sessions.lock().unwrap();
        s.retain(|_, x| matches!(x.child.try_wait(), Ok(None)));
        let mut v: Vec<SessionInfo> = s
            .values()
            .map(|x| SessionInfo {
                idle_secs: x.last.elapsed().as_secs(),
                pinned: pinned.contains(&x.info.model),
                ..x.info.clone()
            })
            .collect();
        v.sort_by(|a, b| a.model.cmp(&b.model));
        v
    }

    /// The model's session, started with `bin` if it is not running. Blocks
    /// while the model loads (seconds for a small one).
    pub fn ensure(&self, bin: &Path, spec: &Spec) -> Result<SessionInfo> {
        let gate = self
            .starting
            .lock()
            .unwrap()
            .entry(spec.id.clone())
            .or_default()
            .clone();
        let _g = gate.lock().unwrap();
        if let Some(info) = self.touch(&spec.id) {
            return Ok(info);
        }
        self.make_room(&spec.id);
        let settings = self.settings();
        let port = self.free_port()?;
        let log = self.logs.join(format!("{}.log", safe(&spec.id)));
        std::fs::create_dir_all(&self.logs)?;
        let out = std::fs::File::create(&log)?;
        let mut cmd = Command::new(bin);
        cmd.args(args(spec, &settings, port))
            .stdin(Stdio::null())
            .stdout(out.try_clone()?)
            .stderr(out);
        if let Some(dir) = bin.parent() {
            // The build's own libraries (ggml-*.so, *.dll, *.dylib) sit beside it.
            cmd.current_dir(dir);
            let var = if cfg!(windows) {
                "PATH"
            } else if cfg!(target_os = "macos") {
                "DYLD_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            };
            let mut paths = vec![dir.to_path_buf()];
            if let Some(old) = std::env::var_os(var) {
                paths.extend(std::env::split_paths(&old));
            }
            if let Ok(joined) = std::env::join_paths(paths) {
                cmd.env(var, joined);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow!("starting {}: {e}", bin.display()))?;
        let deadline = Instant::now() + Duration::from_secs(settings.load_timeout);
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                return Err(classify(&tail(&log), &status.to_string()).into());
            }
            if health(port) {
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(LoadError {
                    code: "MODEL_LOAD_TIMED_OUT",
                    message: format!("{} did not load within {}s", spec.id, settings.load_timeout),
                }
                .into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let info = SessionInfo {
            model: spec.id.clone(),
            path: spec.path.display().to_string(),
            port,
            base_url: format!("http://127.0.0.1:{port}/v1"),
            pid: child.id(),
            idle_secs: 0,
            pinned: false,
            log: log.display().to_string(),
        };
        self.sessions.lock().unwrap().insert(
            spec.id.clone(),
            Session {
                info: info.clone(),
                child,
                last: Instant::now(),
            },
        );
        Ok(info)
    }

    /// A running session, marked used now; None if it is not running.
    pub fn touch(&self, id: &str) -> Option<SessionInfo> {
        let mut s = self.sessions.lock().unwrap();
        let alive = s
            .get_mut(id)
            .is_some_and(|x| matches!(x.child.try_wait(), Ok(None)));
        if !alive {
            s.remove(id);
            return None;
        }
        let x = s.get_mut(id)?;
        x.last = Instant::now();
        Some(x.info.clone())
    }

    /// Stop the least recently used unpinned sessions until one more fits.
    fn make_room(&self, for_id: &str) {
        let max = self.settings().max_loaded.max(1);
        let pinned = self.pinned.lock().unwrap().clone();
        loop {
            let victim = {
                let s = self.sessions.lock().unwrap();
                if s.len() < max {
                    return;
                }
                s.iter()
                    .filter(|(k, _)| *k != for_id && !pinned.contains(k))
                    .min_by_key(|(_, x)| x.last)
                    .map(|(k, _)| k.clone())
            };
            match victim {
                Some(v) => self.unload(&v),
                // Everything loaded is pinned: go over the limit rather than refuse.
                None => return,
            }
        }
    }

    pub fn unload(&self, id: &str) {
        if let Some(mut x) = self.sessions.lock().unwrap().remove(id) {
            let _ = x.child.kill();
            let _ = x.child.wait();
        }
    }

    pub fn unload_all(&self) {
        for (_, mut x) in self.sessions.lock().unwrap().drain() {
            let _ = x.child.kill();
            let _ = x.child.wait();
        }
    }

    /// Unload what has been idle past the setting; returns what went.
    pub fn reap(&self) -> Vec<String> {
        let idle = self.settings().idle_minutes;
        if idle == 0 {
            return vec![];
        }
        let pinned = self.pinned.lock().unwrap().clone();
        let stale: Vec<String> = self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|(k, x)| {
                !pinned.contains(k) && x.last.elapsed() >= Duration::from_secs(idle * 60)
            })
            .map(|(k, _)| k.clone())
            .collect();
        for k in &stale {
            self.unload(k);
        }
        stale
    }

    fn free_port(&self) -> Result<u16> {
        let taken: Vec<u16> = self
            .sessions
            .lock()
            .unwrap()
            .values()
            .map(|x| x.info.port)
            .collect();
        (FIRST_PORT..FIRST_PORT + 200)
            .filter(|p| !taken.contains(p))
            .find(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok())
            .ok_or_else(|| anyhow!("no free port from {FIRST_PORT}"))
    }
}

/// The command line. Kept to flags every recent llama-server has, because a
/// `llama-server` from PATH may be older than the pinned build.
pub fn args(spec: &Spec, s: &Settings, port: u16) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-m".into(),
        spec.path.display().to_string(),
        "-a".into(),
        spec.id.clone(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "-ngl".into(),
        s.gpu_layers.to_string(),
    ];
    if spec.embedding {
        a.extend(["--embedding".into(), "--pooling".into(), "mean".into()]);
    } else {
        let ctx = spec
            .ctx_train
            .filter(|c| *c > 0)
            .map_or(s.ctx, |t| t.min(s.ctx));
        // Chat templates from the file, so tool calls work.
        a.extend(["--jinja".into(), "--ctx-size".into(), ctx.to_string()]);
    }
    a
}

/// `GET /health` answers 2xx: the model is loaded.
fn health(port: u16) -> bool {
    let Ok(mut s) =
        TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(500))
    else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
    if s.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 16];
    let n = s.read(&mut buf).unwrap_or(0);
    let head = String::from_utf8_lossy(&buf[..n]);
    head.starts_with("HTTP/1.1 2") || head.starts_with("HTTP/1.0 2")
}

/// The last 16 KiB of a log.
fn tail(log: &Path) -> String {
    let b = std::fs::read(log).unwrap_or_default();
    String::from_utf8_lossy(&b[b.len().saturating_sub(16 << 10)..]).into_owned()
}

/// Atomic's cascade, in order (PLAN.md §8.2 "Errors").
pub fn classify(log: &str, exit: &str) -> LoadError {
    let l = log.to_ascii_lowercase();
    let any = |xs: &[&str]| xs.iter().any(|x| l.contains(x));
    let code = if l.contains("dyld") && l.contains("symbol not found") {
        "OS_VERSION_UNSUPPORTED"
    } else if any(&[
        "out of memory",
        "failed to allocate",
        "insufficient memory",
        "erroroutofdevicememory",
        "kiogpucommandbuffercallbackerroroutofmemory",
        "cuda_error_out_of_memory",
    ]) {
        "OUT_OF_MEMORY"
    } else if any(&[
        "error loading model architecture",
        "unknown model architecture",
        "error loading model hyperparameters",
        "key not found in model",
    ]) {
        "MODEL_ARCH_NOT_SUPPORTED"
    } else if any(&["unknown projector type"]) {
        "MULTIMODAL_PROJECTOR_LOAD_FAILED"
    } else if any(&[
        "corrupted or incomplete",
        "invalid magic",
        "wrong number of tensors",
        "unexpectedly reached end of file",
        "failed to read tensor",
    ]) {
        "MODEL_FILE_CORRUPT"
    } else {
        "LLAMA_CPP_PROCESS_ERROR"
    };
    let last = log
        .lines()
        .rev()
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or("no output");
    let message = match code {
        "OUT_OF_MEMORY" => "not enough memory for this model; try a smaller quant, a smaller context or fewer GPU layers".to_string(),
        "MODEL_ARCH_NOT_SUPPORTED" => "this llama.cpp build does not know the model's architecture".to_string(),
        "MODEL_FILE_CORRUPT" => "the file is incomplete or corrupt; download it again".to_string(),
        "OS_VERSION_UNSUPPORTED" => "this build needs a newer OS".to_string(),
        _ => format!("llama-server exited ({exit}): {last}"),
    };
    LoadError { code, message }
}

fn safe(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(ctx: Option<u64>, embedding: bool) -> Spec {
        Spec {
            id: "m".into(),
            path: "/m.gguf".into(),
            ctx_train: ctx,
            embedding,
        }
    }

    #[test]
    fn args_cap_the_context_and_switch_for_embeddings() {
        let s = Settings::default();
        let a = args(&spec(Some(4096), false), &s, 8090).join(" ");
        assert_eq!(
            a,
            "-m /m.gguf -a m --host 127.0.0.1 --port 8090 -ngl 99 --jinja --ctx-size 4096"
        );
        assert!(args(&spec(Some(131072), false), &s, 1)
            .join(" ")
            .ends_with("--ctx-size 8192"));
        assert!(args(&spec(None, false), &s, 1)
            .join(" ")
            .ends_with("--ctx-size 8192"));
        assert!(args(&spec(None, true), &s, 1)
            .join(" ")
            .ends_with("--embedding --pooling mean"));
    }

    #[test]
    fn classify_follows_the_cascade_in_order() {
        assert_eq!(
            classify("dyld: Symbol not found: _foo", "1").code,
            "OS_VERSION_UNSUPPORTED"
        );
        assert_eq!(
            classify("ggml_vulkan: ErrorOutOfDeviceMemory\n", "1").code,
            "OUT_OF_MEMORY"
        );
        assert_eq!(
            classify(
                "llama_model_load: error loading model architecture: unknown",
                "1"
            )
            .code,
            "MODEL_ARCH_NOT_SUPPORTED"
        );
        assert_eq!(
            classify("gguf_init: invalid magic characters", "1").code,
            "MODEL_FILE_CORRUPT"
        );
        let e = classify("something\nbad flag --frob\n\n", "exit status: 1");
        assert_eq!(e.code, "LLAMA_CPP_PROCESS_ERROR");
        assert!(e.message.ends_with("bad flag --frob"), "{}", e.message);
    }

    /// A stand-in `llama-server`: Python serving `/health` on the port it is
    /// given, or failing the way a model too big for memory does.
    #[cfg(unix)]
    fn fake_server(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let py = r#"
import sys, http.server
a = sys.argv[1:]
if "oom" in a[a.index("-m") + 1]:
    print("ggml_vulkan: ErrorOutOfDeviceMemory", flush=True); sys.exit(1)
port = int(a[a.index("--port") + 1])
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200); self.send_header("content-length", "2"); self.end_headers(); self.wfile.write(b"ok")
    def log_message(self, *x): pass
http.server.HTTPServer(("127.0.0.1", port), H).serve_forever()
"#;
        std::fs::write(dir.join("fake.py"), py).unwrap();
        let bin = dir.join("llama-server");
        std::fs::write(
            &bin,
            format!(
                "#!/bin/sh\nexec python3 {} \"$@\"\n",
                dir.join("fake.py").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    #[cfg(unix)]
    #[test]
    fn loads_on_demand_makes_room_and_reports_why_a_load_failed() {
        let dir = std::env::temp_dir().join(format!("brainpowr-sup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = fake_server(&dir);
        let sup = Supervisor::new(
            dir.join("logs"),
            Settings {
                max_loaded: 1,
                load_timeout: 20,
                ..Default::default()
            },
        );
        let sp = |id: &str| Spec {
            id: id.into(),
            path: dir.join(format!("{id}.gguf")),
            ctx_train: None,
            embedding: false,
        };
        let a = sup.ensure(&bin, &sp("a")).unwrap();
        assert!(health(a.port));
        // Asking again reuses the session.
        assert_eq!(sup.ensure(&bin, &sp("a")).unwrap().pid, a.pid);
        // One model at a time: b pushes a out.
        let b = sup.ensure(&bin, &sp("b")).unwrap();
        let ids: Vec<String> = sup.sessions().into_iter().map(|s| s.model).collect();
        assert_eq!(ids, ["b"]);
        // a's process is gone (b may well have taken its port).
        assert_ne!(b.pid, a.pid);
        assert!(
            !std::path::Path::new(&format!("/proc/{}", a.pid)).exists()
                || cfg!(not(target_os = "linux"))
        );
        // A pinned model stays, so the limit is exceeded rather than it unloaded.
        sup.set_pinned(vec!["b".into()]);
        sup.ensure(&bin, &sp("c")).unwrap();
        assert_eq!(sup.sessions().len(), 2);
        let err = sup.ensure(&bin, &sp("oom")).unwrap_err();
        assert_eq!(
            err.downcast_ref::<LoadError>().unwrap().code,
            "OUT_OF_MEMORY"
        );
        sup.unload_all();
        assert!(sup.sessions().is_empty() && !health(b.port));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_names_a_log_file() {
        assert_eq!(safe("org/Qwen3:1.7b"), "org_Qwen3_1.7b");
    }
}
