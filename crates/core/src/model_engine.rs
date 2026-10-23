//! Getting models and running them: the half of Atomic Chat's core that the
//! model server (model_gateway.rs) did not have.
//!
//! Two ways to get a model, whichever runtime the user has:
//! - **Ollama**: `POST /api/pull`, with its own progress lines. Ollama keeps
//!   the files and serves them; nothing else to start.
//! - **Hugging Face GGUF**: one file into `<data>/models/<org>__<repo>/`,
//!   resumed from a `.part` on the next try, then started as an engine.
//!
//! Downloaded files run in Brainpowr Engine (crates/engine): a pinned
//! llama.cpp build it installs itself, a model loaded on the first request
//! for it and unloaded when idle. [`runtimes`] lists every downloaded file
//! under the model server's URL, so the model server, Chat and the small jobs
//! see a model the moment it is on disk; the gateway calls
//! [`Engine::ensure`] before it forwards. Pinned models load at launch
//! (`prefs.pinned_models`); everything unloads when the app quits
//! ([`stop_all`]). `ollama serve` can be started when Ollama is stopped.
//!
//! Every long call here (a download, a pull) runs on its own thread and
//! reports through [`jobs`]; nothing blocks for longer than a request.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use brainpowr_engine::Engine;
use serde::Serialize;
use serde_json::Value;

use crate::harnesses::which;
use crate::model_server::{is_tiny, LocalKind, LocalModel, LocalRuntime};
use crate::prefs::Prefs;

// ---------------------------------------------------------------- suggestions

/// A small model worth having, by its Ollama tag and its GGUF repository. The
/// file in the repository is found when it is downloaded (the `quant` one),
/// so a renamed file is a lookup, not a broken link.
#[derive(Serialize, Clone, Debug)]
pub struct Suggested {
    pub name: &'static str,
    pub note: &'static str,
    pub ollama: &'static str,
    pub repo: &'static str,
    pub quant: &'static str,
    pub params_b: f32,
}

pub const SUGGESTED: &[Suggested] = &[
    Suggested {
        name: "Qwen3 1.7B",
        note: "Best all-round small model; reads, summarises, extracts.",
        ollama: "qwen3:1.7b",
        repo: "unsloth/Qwen3-1.7B-GGUF",
        quant: "Q4_K_M",
        params_b: 1.7,
    },
    Suggested {
        name: "Gemma 3 1B",
        note: "The fastest with real quality.",
        ollama: "gemma3:1b",
        repo: "ggml-org/gemma-3-1b-it-GGUF",
        quant: "Q4_K_M",
        params_b: 1.0,
    },
    Suggested {
        name: "Llama 3.2 3B",
        note: "A dependable generalist.",
        ollama: "llama3.2:3b",
        repo: "bartowski/Llama-3.2-3B-Instruct-GGUF",
        quant: "Q4_K_M",
        params_b: 3.0,
    },
    Suggested {
        name: "Qwen3 4B",
        note: "The largest still counted small.",
        ollama: "qwen3:4b",
        repo: "unsloth/Qwen3-4B-GGUF",
        quant: "Q4_K_M",
        params_b: 4.0,
    },
    Suggested {
        name: "Phi-4 mini",
        note: "Tool use.",
        ollama: "phi4-mini:3.8b",
        repo: "unsloth/Phi-4-mini-instruct-GGUF",
        quant: "Q4_K_M",
        params_b: 3.8,
    },
];

// ---------------------------------------------------------------- jobs

/// A download or a pull, as the UI shows it.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Job {
    pub id: String,
    pub label: String,
    /// "hf" or "ollama".
    pub kind: &'static str,
    /// "running", "done", "failed", "cancelled".
    pub status: &'static str,
    pub done: u64,
    pub total: Option<u64>,
    /// What the runtime says it is doing ("pulling manifest"...).
    pub note: Option<String>,
    pub error: Option<String>,
    /// The file, once a download is done.
    pub path: Option<String>,
}

struct Running {
    job: Job,
    cancel: Arc<AtomicBool>,
}

static JOBS: Mutex<BTreeMap<String, Running>> = Mutex::new(BTreeMap::new());

/// Every download and pull this session, newest last.
pub fn jobs() -> Vec<Job> {
    JOBS.lock()
        .unwrap()
        .values()
        .map(|r| r.job.clone())
        .collect()
}

pub fn cancel(id: &str) {
    if let Some(r) = JOBS.lock().unwrap().get(id) {
        r.cancel.store(true, Ordering::SeqCst);
    }
}

/// Forget the finished ones.
pub fn clear_jobs() {
    JOBS.lock()
        .unwrap()
        .retain(|_, r| r.job.status == "running");
}

/// Start a job unless the same one is already running; returns its flag.
fn begin(id: &str, label: &str, kind: &'static str) -> Result<Arc<AtomicBool>> {
    let mut jobs = JOBS.lock().unwrap();
    if jobs.get(id).is_some_and(|r| r.job.status == "running") {
        bail!("{label} is already downloading");
    }
    let cancel = Arc::new(AtomicBool::new(false));
    jobs.insert(
        id.to_string(),
        Running {
            job: Job {
                id: id.into(),
                label: label.into(),
                kind,
                status: "running",
                done: 0,
                total: None,
                note: None,
                error: None,
                path: None,
            },
            cancel: cancel.clone(),
        },
    );
    Ok(cancel)
}

fn update(id: &str, f: impl FnOnce(&mut Job)) {
    if let Some(r) = JOBS.lock().unwrap().get_mut(id) {
        f(&mut r.job);
    }
}

fn finish(id: &str, res: Result<Option<String>>, cancel: &AtomicBool) {
    update(id, |j| match res {
        Ok(path) => {
            j.status = "done";
            j.path = path;
            j.total = j.total.or(Some(j.done));
            j.done = j.total.unwrap_or(j.done);
        }
        Err(_) if cancel.load(Ordering::SeqCst) => j.status = "cancelled",
        Err(e) => {
            j.status = "failed";
            j.error = Some(format!("{e:#}"));
        }
    });
}

fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()?)
}

fn block_on<T>(f: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(f)
}

// ---------------------------------------------------------------- ollama

/// Pull a model into Ollama in the background. Returns the job id.
pub fn ollama_pull(prefs: &Prefs, name: &str) -> Result<String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.contains(char::is_whitespace) {
        bail!("give an Ollama model name, e.g. qwen3:1.7b");
    }
    let id = format!("ollama:{name}");
    let cancel = begin(&id, &name, "ollama")?;
    let base = prefs.ollama_url.trim_end_matches('/').to_string();
    let jid = id.clone();
    std::thread::spawn(move || {
        let res = block_on(pull(&base, &name, &jid, &cancel)).map(|_| None);
        finish(&jid, res, &cancel);
    });
    Ok(id)
}

async fn pull(base: &str, name: &str, id: &str, cancel: &AtomicBool) -> Result<()> {
    let mut resp = http()?
        .post(format!("{base}/api/pull"))
        .json(&serde_json::json!({"model": name, "stream": true}))
        .send()
        .await
        .map_err(|e| anyhow!("Ollama is not answering at {base}: {e}"))?;
    if !resp.status().is_success() {
        bail!("Ollama said {}", resp.status());
    }
    // One JSON object per line; a line can span chunks.
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if cancel.load(Ordering::SeqCst) {
            bail!("cancelled");
        }
        buf.extend_from_slice(&chunk);
        while let Some(i) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=i).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            if let Some(e) = v["error"].as_str() {
                bail!("{e}");
            }
            pull_line(id, &v);
        }
    }
    Ok(())
}

/// One progress line from `/api/pull`. Ollama pulls layer by layer, so the
/// bar follows the layer it is on.
fn pull_line(id: &str, v: &Value) {
    update(id, |j| {
        j.note = v["status"].as_str().map(str::to_string);
        if let (Some(done), Some(total)) = (v["completed"].as_u64(), v["total"].as_u64()) {
            j.done = done;
            j.total = Some(total);
        }
    });
}

/// `ollama serve`, when Ollama is installed but nothing answers.
pub fn start_ollama() -> Result<()> {
    let bin = which("ollama")
        .ok_or_else(|| anyhow!("Ollama is not installed: https://ollama.com/download"))?;
    Command::new(bin)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow!("starting ollama serve: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------- hugging face

/// Where downloaded models live.
pub fn models_dir() -> PathBuf {
    Prefs::data_dir().join("models")
}

/// A GGUF file a repository holds.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RepoFile {
    pub file: String,
    pub size: Option<u64>,
}

/// The GGUF files in a Hugging Face repository (`org/name`), without the
/// vision projectors and the second and later parts of a split file.
pub fn hf_files(repo: &str) -> Result<Vec<RepoFile>> {
    let repo = check_repo(repo)?;
    let v: Value = block_on(async {
        let r = http()?
            .get(format!(
                "https://huggingface.co/api/models/{repo}/tree/main"
            ))
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| anyhow!("can't reach Hugging Face: {e}"))?;
        if r.status() == reqwest::StatusCode::NOT_FOUND
            || r.status() == reqwest::StatusCode::UNAUTHORIZED
        {
            bail!("no public repository {repo} on Hugging Face");
        }
        Ok(r.error_for_status()?.json().await?)
    })?;
    Ok(gguf_files(&v))
}

fn gguf_files(tree: &Value) -> Vec<RepoFile> {
    tree.as_array()
        .map(|a| {
            a.iter()
                .filter(|e| e["type"] == "file")
                .filter_map(|e| {
                    let file = e["path"].as_str()?.to_string();
                    let lower = file.to_ascii_lowercase();
                    let later_part = lower.contains("-of-0") && !lower.contains("-00001-of-");
                    (lower.ends_with(".gguf") && !lower.contains("mmproj") && !later_part).then(
                        || RepoFile {
                            file,
                            size: e["size"].as_u64(),
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The file a quantization names, the smallest such when several do.
pub fn pick_file<'a>(files: &'a [RepoFile], quant: &str) -> Option<&'a RepoFile> {
    let q = quant.to_ascii_lowercase();
    files
        .iter()
        .filter(|f| f.file.to_ascii_lowercase().contains(&q))
        .min_by_key(|f| f.size.unwrap_or(u64::MAX))
}

fn check_repo(repo: &str) -> Result<String> {
    let repo = repo
        .trim()
        .trim_start_matches("https://huggingface.co/")
        .trim_matches('/')
        .to_string();
    let ok = repo.split('/').count() == 2
        && repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        && !repo.contains("..");
    if !ok {
        bail!("give a Hugging Face repository as org/name, e.g. unsloth/Qwen3-1.7B-GGUF");
    }
    Ok(repo)
}

/// Where a repository's file is kept: `<models>/<org>__<name>/<file>`.
pub fn local_path(repo: &str, file: &str) -> Result<PathBuf> {
    let repo = check_repo(repo)?;
    let name = Path::new(file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name.is_empty() || !name.to_ascii_lowercase().ends_with(".gguf") {
        bail!("{file} is not a GGUF file");
    }
    Ok(models_dir().join(repo.replace('/', "__")).join(name))
}

/// Download one GGUF file in the background (the `quant` one when `file` is
/// empty). Returns the job id. A `.part` left by an earlier try is resumed.
pub fn hf_download(repo: &str, file: &str, quant: &str) -> Result<String> {
    let repo = check_repo(repo)?;
    let label = if file.is_empty() {
        format!("{repo} {quant}")
    } else {
        file.to_string()
    };
    let id = format!("hf:{repo}:{}", if file.is_empty() { quant } else { file });
    let cancel = begin(&id, &label, "hf")?;
    let (file, quant, jid) = (file.to_string(), quant.to_string(), id.clone());
    std::thread::spawn(move || {
        let res = (|| {
            let file = if file.is_empty() {
                let files = hf_files(&repo)?;
                pick_file(&files, &quant)
                    .map(|f| f.file.clone())
                    .ok_or_else(|| anyhow!("{repo} has no {quant} GGUF file"))?
            } else {
                file
            };
            update(&jid, |j| j.label = file.clone());
            let dest = local_path(&repo, &file)?;
            let url = format!("https://huggingface.co/{repo}/resolve/main/{file}");
            block_on(fetch(&url, &dest, &jid, &cancel))?;
            Ok(Some(dest.display().to_string()))
        })();
        finish(&jid, res, &cancel);
    });
    Ok(id)
}

async fn fetch(url: &str, dest: &Path, id: &str, cancel: &AtomicBool) -> Result<()> {
    if dest.exists() {
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = dest.with_extension("gguf.part");
    let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let mut rb = http()?.get(url);
    if have > 0 {
        rb = rb.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut resp = rb
        .send()
        .await
        .map_err(|e| anyhow!("can't reach Hugging Face: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        bail!("Hugging Face said {status} for {url}");
    }
    // 206: the server resumes where the part ends; 200: it starts over.
    let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT;
    let start = if resumed { have } else { 0 };
    let total = resp.content_length().map(|n| n + start);
    update(id, |j| {
        j.done = start;
        j.total = total;
    });
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&part)?;
    let mut done = start;
    while let Some(chunk) = resp.chunk().await? {
        if cancel.load(Ordering::SeqCst) {
            bail!("cancelled");
        }
        out.write_all(&chunk)?;
        done += chunk.len() as u64;
        update(id, |j| j.done = done);
    }
    out.flush()?;
    drop(out);
    if total.is_some_and(|t| t != done) {
        bail!(
            "the download stopped at {done} of {} bytes; try again to resume",
            total.unwrap_or(0)
        );
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// A GGUF file on this machine, as the engine sees it.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Downloaded {
    pub path: String,
    /// The id it is served as: the file name without `.gguf`.
    pub name: String,
    pub size: u64,
    pub meta: brainpowr_engine::gguf::Meta,
    /// Its session, while it is in memory.
    pub loaded: Option<brainpowr_engine::SessionInfo>,
    /// Kept in memory, and loaded when the app opens.
    pub pinned: bool,
}

/// Every downloaded GGUF, with what is loaded.
pub fn downloaded(prefs: &Prefs) -> Vec<Downloaded> {
    let e = engine();
    let sessions = e.sessions();
    e.models()
        .into_iter()
        .map(|m| Downloaded {
            loaded: sessions.iter().find(|s| s.model == m.id).cloned(),
            pinned: prefs.pinned_models.contains(&m.id),
            name: m.id,
            path: m.path,
            size: m.size,
            meta: m.meta,
        })
        .collect()
}

/// Delete a downloaded file, unloading it first. Only files under
/// [`models_dir`] can go.
pub fn delete(path: &str) -> Result<()> {
    let p = inside_models(path)?;
    engine().unload(&alias(&p));
    std::fs::remove_file(&p)?;
    if let Some(dir) = p.parent() {
        let _ = std::fs::remove_dir(dir);
    }
    Ok(())
}

fn inside_models(path: &str) -> Result<PathBuf> {
    let root = dunce::canonicalize(models_dir()).map_err(|_| anyhow!("no models downloaded"))?;
    let p = dunce::canonicalize(path).map_err(|_| anyhow!("{path} is not there"))?;
    if !p.starts_with(&root) {
        bail!("{path} is not a downloaded model");
    }
    Ok(p)
}

/// The id a file is served as: `Qwen3-1.7B-Q4_K_M` for `Qwen3-1.7B-Q4_K_M.gguf`.
fn alias(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model")
        .to_string()
}

// ---------------------------------------------------------------- the engine

/// The runtime id downloaded models are served under.
pub const RUNTIME_ID: &str = "brainpowr";

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();

/// Brainpowr Engine (crates/engine) for this app: builds and logs under
/// `<data>/brainpowr`, models from [`models_dir`], settings from prefs.
pub fn engine() -> &'static Arc<Engine> {
    ENGINE.get_or_init(|| {
        let prefs = Prefs::load();
        let e = Engine::new(Prefs::data_dir().join("brainpowr"), models_dir(), prefs.engine.clone());
        e.prefer(prefs.engine_variant.clone());
        e.set_pinned(prefs.pinned_models.clone());
        e
    })
}

/// Settings changed: the engine follows at once.
pub fn apply(prefs: &Prefs) {
    let e = engine();
    e.set_settings(prefs.engine.clone());
    e.prefer(prefs.engine_variant.clone());
    e.set_pinned(prefs.pinned_models.clone());
}

/// Install a llama.cpp build in the background, as a job like a download.
pub fn install_backend(variant: &str) -> Result<String> {
    let id = format!("engine:{variant}");
    let cancel = begin(&id, &format!("llama.cpp {} ({variant})", brainpowr_engine::backend::PINNED_TAG), "engine")?;
    let (jid, variant) = (id.clone(), variant.to_string());
    std::thread::spawn(move || {
        let home = engine().home().to_path_buf();
        let progress = |done: u64, total: u64| {
            update(&jid, |j| {
                j.done = done;
                j.total = Some(total);
            })
        };
        let res = brainpowr_engine::backend::install(&home, &variant, &progress, &cancel)
            .map(|i| Some(home.join("backends").join(&i.tag).join(&i.variant).join(&i.server).display().to_string()));
        finish(&jid, res, &cancel);
    });
    Ok(id)
}

/// The pinned models, loaded at launch. Off the calling thread: a model takes
/// seconds to load. Returns nothing; a failure shows on the next status.
pub fn restore(prefs: &Prefs) {
    let ids = prefs.pinned_models.clone();
    if ids.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        for id in ids {
            let _ = engine().ensure(&id);
        }
    });
}

/// Every model in memory, unloaded; called as the app quits, so a closed app
/// does not leave a model holding the GPU.
pub fn stop_all() {
    if let Some(e) = ENGINE.get() {
        e.unload_all();
    }
}

/// The downloaded models as one runtime for [`crate::model_server::detect`].
/// Its URL is the model server's: that is where a request makes the engine
/// load the model. Running whenever a llama.cpp build is there to load with.
pub fn runtimes(prefs: &Prefs) -> Vec<LocalRuntime> {
    let e = engine();
    let models = e.models();
    if models.is_empty() {
        return vec![];
    }
    let bin = e.binary();
    vec![LocalRuntime {
        id: RUNTIME_ID.into(),
        name: "Brainpowr Engine".into(),
        kind: LocalKind::LlamaCpp,
        base_url: prefs.model_serve.local_url(),
        installed: bin.is_some(),
        running: bin.is_some() && prefs.model_serve.enabled,
        models: models
            .into_iter()
            .map(|m| {
                let params = m.meta.size_label.as_deref().and_then(params_from_label);
                LocalModel {
                    tiny: is_tiny(&m.id, params, Some(m.size)),
                    params_b: params,
                    size_bytes: Some(m.size),
                    context: m.meta.context_length,
                    family: m.meta.architecture.clone(),
                    runtime: RUNTIME_ID.into(),
                    id: m.id,
                    ..Default::default()
                }
            })
            .collect(),
        version: Some(brainpowr_engine::backend::PINNED_TAG.into()),
        message: if bin.is_none() {
            Some("No llama.cpp build yet: install one under Get models.".into())
        } else if !prefs.model_serve.enabled {
            Some("Turn the model server on to use these.".into())
        } else {
            None
        },
        install: "",
        host: None,
        key: String::new(),
    }]
}

/// `1.7B` → 1.7; `8x7B` and other shapes say nothing simple.
fn params_from_label(label: &str) -> Option<f32> {
    let l = label.trim().to_ascii_uppercase();
    let (n, mul) = if let Some(n) = l.strip_suffix('B') {
        (n, 1.0)
    } else if let Some(n) = l.strip_suffix('M') {
        (n, 0.001)
    } else {
        return None;
    };
    n.parse::<f32>().ok().filter(|x| x.is_finite() && *x > 0.0).map(|x| x * mul)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gguf_files_skips_projectors_and_later_parts() {
        let tree = json!([
            {"type": "file", "path": "README.md", "size": 10},
            {"type": "file", "path": "Qwen3-1.7B-Q4_K_M.gguf", "size": 1_100_000_000u64},
            {"type": "file", "path": "Qwen3-1.7B-Q8_0.gguf", "size": 1_800_000_000u64},
            {"type": "file", "path": "mmproj-F16.gguf", "size": 5},
            {"type": "file", "path": "big-00001-of-00002.gguf", "size": 7},
            {"type": "file", "path": "big-00002-of-00002.gguf", "size": 7},
            {"type": "directory", "path": "old.gguf"}
        ]);
        let names: Vec<String> = gguf_files(&tree).into_iter().map(|f| f.file).collect();
        assert_eq!(
            names,
            [
                "Qwen3-1.7B-Q4_K_M.gguf",
                "Qwen3-1.7B-Q8_0.gguf",
                "big-00001-of-00002.gguf"
            ]
        );
        assert!(gguf_files(&json!({"error": "x"})).is_empty());
    }

    #[test]
    fn pick_file_matches_the_quant_and_takes_the_smallest() {
        let files = vec![
            RepoFile {
                file: "m-UD-Q4_K_M.gguf".into(),
                size: Some(30),
            },
            RepoFile {
                file: "m-Q4_K_M.gguf".into(),
                size: Some(20),
            },
            RepoFile {
                file: "m-Q8_0.gguf".into(),
                size: Some(10),
            },
        ];
        assert_eq!(pick_file(&files, "q4_k_m").unwrap().file, "m-Q4_K_M.gguf");
        assert!(pick_file(&files, "Q2_K").is_none());
    }

    #[test]
    fn repos_and_paths_stay_inside_the_models_folder() {
        assert_eq!(
            check_repo("https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/").unwrap(),
            "unsloth/Qwen3-1.7B-GGUF"
        );
        assert!(check_repo("unsloth").is_err());
        assert!(check_repo("a/b/c").is_err());
        assert!(check_repo("../etc").is_err());
        assert!(check_repo("a/b c").is_err());
        let p = local_path("unsloth/Qwen3-1.7B-GGUF", "sub/../Qwen3-1.7B-Q4_K_M.gguf").unwrap();
        assert!(p.ends_with("unsloth__Qwen3-1.7B-GGUF/Qwen3-1.7B-Q4_K_M.gguf"));
        assert!(local_path("a/b", "notes.txt").is_err());
        assert_eq!(alias(&p), "Qwen3-1.7B-Q4_K_M");
    }

    #[test]
    fn size_labels_read_as_parameters() {
        assert_eq!(params_from_label("1.7B"), Some(1.7));
        assert_eq!(params_from_label("360M"), Some(0.36));
        assert_eq!(params_from_label("8x7B"), None);
    }

    #[test]
    fn a_pull_line_moves_the_bar() {
        let id = "ollama:test-pull-line";
        begin(id, "t", "ollama").unwrap();
        assert!(begin(id, "t", "ollama").is_err());
        pull_line(id, &json!({"status": "pulling manifest"}));
        pull_line(
            id,
            &json!({"status": "pulling abc", "total": 100, "completed": 40}),
        );
        let j = jobs().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!(
            (j.done, j.total, j.note.as_deref()),
            (40, Some(100), Some("pulling abc"))
        );
        let flag = AtomicBool::new(false);
        finish(id, Ok(None), &flag);
        let j = jobs().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!((j.status, j.done), ("done", 100));
    }
}
