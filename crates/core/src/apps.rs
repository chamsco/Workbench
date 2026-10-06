//! Apps: downloadable views with their own agent, described by one JSON
//! file, `backspace-app.json` (format: docs/apps.md).
//!
//! An installed app is a folder under `<data>/apps/<id>/` holding the
//! manifest and the files its view needs. The desktop app serves those files
//! to a sandboxed frame and lets the page talk back through a small SDK:
//! ask the app's agent (any CLI, local model, router or Cloud), keep a few
//! values, show a toast. Apps may also list tools that coding CLIs get
//! through `backspace mcp`.
//!
//! Install from a folder (development) or from a manifest URL; the URL form
//! downloads the `files` the manifest lists, relative to the manifest.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MANIFEST: &str = "backspace-app.json";
/// The manifest versions this build reads.
pub const SCHEMA: u32 = 1;
const MAX_FILE: usize = 8 << 20;
const MAX_FILES: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    #[serde(default = "one")]
    pub schema: u32,
    /// Lowercase letters, digits and dashes; the app's folder name.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub icon: Icon,
    #[serde(default)]
    pub view: View,
    #[serde(default)]
    pub agent: Option<AgentSpec>,
    /// What the page may ask the host for: "agent", "storage", "notify",
    /// "open_url", "routes". Anything else is refused.
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub tools: Vec<Tool>,
    /// Files to download next to the manifest on a URL install.
    #[serde(default)]
    pub files: Vec<String>,
}

fn one() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Icon {
    /// One or two characters drawn on `color`.
    #[serde(default)]
    pub glyph: String,
    #[serde(default)]
    pub color: String,
    /// An image in the app folder (png, svg).
    #[serde(default)]
    pub file: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct View {
    /// A page in the app folder ("index.html").
    #[serde(default)]
    pub entry: String,
    /// Or a page served elsewhere (a dev server, a hosted app).
    #[serde(default)]
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct AgentSpec {
    #[serde(default)]
    pub name: String,
    /// The agent's brief, given to every reply as its system prompt.
    #[serde(default)]
    pub instructions: String,
    /// Who answers unless the user picks otherwise.
    #[serde(default)]
    pub route: Option<crate::chat::Route>,
    #[serde(default)]
    pub suggestions: Vec<String>,
}

/// A command coding CLIs can call through `backspace mcp`. The input is
/// written to its stdin as JSON; whatever it prints is the result.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "empty_schema")]
    pub input_schema: Value,
    /// Program and arguments, run in the app folder.
    pub command: Vec<String>,
}

fn empty_schema() -> Value {
    serde_json::json!({ "type": "object", "properties": {} })
}

/// An installed app as the UI lists it.
#[derive(Serialize, Clone, Debug)]
pub struct Installed {
    #[serde(flatten)]
    pub manifest: Manifest,
    pub dir: String,
    /// Where it came from: a folder or a URL.
    pub source: String,
    pub installed_at: u64,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct Meta {
    source: String,
    installed_at: u64,
    #[serde(default = "yes")]
    enabled: bool,
}

fn yes() -> bool {
    true
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest> {
        let m: Manifest = serde_json::from_str(text).context("reading backspace-app.json")?;
        m.check()?;
        Ok(m)
    }

    pub fn check(&self) -> Result<()> {
        if self.schema == 0 || self.schema > SCHEMA {
            bail!("this app needs a newer Backspace (manifest schema {})", self.schema);
        }
        if self.id.is_empty()
            || self.id.len() > 48
            || !self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            || self.id.starts_with('-')
        {
            bail!("an app id is 1-48 lowercase letters, digits and dashes (got {:?})", self.id);
        }
        if self.name.trim().is_empty() {
            bail!("the app needs a name");
        }
        if self.view.entry.is_empty() && self.view.url.is_empty() {
            bail!("the app needs a view: an `entry` page in its folder or a `url`");
        }
        if !self.view.url.is_empty() && !self.view.url.starts_with("http://") && !self.view.url.starts_with("https://") {
            bail!("view.url must be http(s)");
        }
        for f in self.files.iter().chain([&self.view.entry, &self.icon.file]) {
            if !f.is_empty() {
                safe_rel(f)?;
            }
        }
        if self.files.len() > MAX_FILES {
            bail!("too many files (most is {MAX_FILES})");
        }
        for t in &self.tools {
            if t.name.is_empty() || !t.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                bail!("tool names are letters, digits and underscores ({:?})", t.name);
            }
            if t.command.is_empty() {
                bail!("tool {} has no command", t.name);
            }
        }
        Ok(())
    }

    pub fn allows(&self, perm: &str) -> bool {
        self.permissions.iter().any(|p| p == perm)
    }
}

/// A relative path that stays inside the app folder.
pub fn safe_rel(p: &str) -> Result<PathBuf> {
    let path = Path::new(p);
    if path.is_absolute() || p.contains('\\') {
        bail!("{p}: paths in an app are relative, with forward slashes");
    }
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => bail!("{p}: paths may not leave the app folder"),
        }
    }
    if out.as_os_str().is_empty() {
        bail!("empty path");
    }
    Ok(out)
}

pub struct Apps {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Apps {
    pub fn open(dir: PathBuf) -> Self {
        Self {
            dir,
            lock: Mutex::new(()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn app_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    pub fn list(&self) -> Vec<Installed> {
        let mut out = vec![];
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        for e in rd.flatten() {
            if let Some(a) = e.file_name().to_str().and_then(|id| self.get(id)) {
                out.push(a);
            }
        }
        out.sort_by(|a, b| a.manifest.name.to_lowercase().cmp(&b.manifest.name.to_lowercase()));
        out
    }

    pub fn get(&self, id: &str) -> Option<Installed> {
        let d = self.app_dir(id);
        let m = Manifest::parse(&std::fs::read_to_string(d.join(MANIFEST)).ok()?).ok()?;
        if m.id != id {
            return None;
        }
        let meta: Meta = std::fs::read(d.join(".meta.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Some(Installed {
            manifest: m,
            dir: d.display().to_string(),
            source: meta.source,
            installed_at: meta.installed_at,
            enabled: meta.enabled,
        })
    }

    /// Install (or update) from a folder holding backspace-app.json, or the
    /// manifest file itself.
    pub fn install_dir(&self, path: &Path) -> Result<Installed> {
        let src = if path.is_file() {
            path.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            path.to_path_buf()
        };
        let text = std::fs::read_to_string(src.join(MANIFEST))
            .with_context(|| format!("no {MANIFEST} in {}", src.display()))?;
        let m = Manifest::parse(&text)?;
        let _g = self.lock.lock().unwrap();
        let stage = self.dir.join(format!(".{}.new", m.id));
        let _ = std::fs::remove_dir_all(&stage);
        copy_tree(&src, &stage, 0)?;
        self.commit(&m.id, &stage, &src.display().to_string())
    }

    /// Install (or update) from the URL of a backspace-app.json: the
    /// manifest, then each listed file and the entry page, relative to it.
    pub async fn install_url(&self, http: &reqwest::Client, url: &str) -> Result<Installed> {
        let url = url.trim();
        if !url.starts_with("https://") && !url.starts_with("http://") {
            bail!("an app URL starts with https://");
        }
        let get = |u: String| async move {
            let r = http.get(&u).send().await.with_context(|| format!("fetching {u}"))?;
            if !r.status().is_success() {
                bail!("{u}: {}", r.status());
            }
            let b = r.bytes().await?;
            if b.len() > MAX_FILE {
                bail!("{u} is larger than {} MB", MAX_FILE >> 20);
            }
            Ok::<_, anyhow::Error>(b.to_vec())
        };
        let text = String::from_utf8(get(url.to_string()).await?).context("the manifest is not text")?;
        let m = Manifest::parse(&text)?;
        let base = url.rsplit_once('/').map(|(b, _)| b).unwrap_or(url);
        let stage = self.dir.join(format!(".{}.new", m.id));
        let _ = std::fs::remove_dir_all(&stage);
        std::fs::create_dir_all(&stage)?;
        std::fs::write(stage.join(MANIFEST), &text)?;
        let mut files: Vec<&String> = m.files.iter().collect();
        for extra in [&m.view.entry, &m.icon.file] {
            if !extra.is_empty() && !files.contains(&extra) {
                files.push(extra);
            }
        }
        for f in files {
            let rel = safe_rel(f)?;
            let body = get(format!("{base}/{f}")).await?;
            let to = stage.join(&rel);
            if let Some(p) = to.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::write(to, body)?;
        }
        let _g = self.lock.lock().unwrap();
        self.commit(&m.id, &stage, url)
    }

    /// Swap a staged folder in, keeping the app's stored values.
    fn commit(&self, id: &str, stage: &Path, source: &str) -> Result<Installed> {
        let dest = self.app_dir(id);
        let keep = dest.join(".data");
        if keep.is_dir() {
            let _ = std::fs::rename(&keep, stage.join(".data"));
        }
        let meta = Meta {
            source: source.to_string(),
            installed_at: crate::chat::now_ms(),
            enabled: true,
        };
        std::fs::write(stage.join(".meta.json"), serde_json::to_vec(&meta)?)?;
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&self.dir)?;
        std::fs::rename(stage, &dest)?;
        self.get(id).ok_or_else(|| anyhow!("installed, but the manifest did not read back"))
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        Manifest {
            id: id.into(),
            ..placeholder()
        }
        .check()?;
        let d = self.app_dir(id);
        if d.is_dir() {
            std::fs::remove_dir_all(d)?;
        }
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, on: bool) -> Result<()> {
        let p = self.app_dir(id).join(".meta.json");
        let mut meta: Meta = std::fs::read(&p)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        meta.enabled = on;
        std::fs::write(p, serde_json::to_vec(&meta)?)?;
        Ok(())
    }

    /// A file of an app's view, for the frame's custom protocol.
    pub fn file(&self, id: &str, rel: &str) -> Result<(Vec<u8>, &'static str)> {
        let a = self.get(id).ok_or_else(|| anyhow!("no app {id}"))?;
        let rel = if rel.is_empty() { a.manifest.view.entry.as_str() } else { rel };
        let p = self.app_dir(id).join(safe_rel(rel)?);
        if p.components().any(|c| c.as_os_str() == ".data" || c.as_os_str() == ".meta.json") {
            bail!("not served");
        }
        let body = std::fs::read(&p).with_context(|| format!("{id}/{rel}"))?;
        Ok((body, mime_of(&p)))
    }

    // ------------------------------------------------------------ storage

    fn store_path(&self, id: &str) -> PathBuf {
        self.app_dir(id).join(".data").join("storage.json")
    }

    fn store(&self, id: &str) -> BTreeMap<String, Value> {
        std::fs::read(self.store_path(id))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn storage_get(&self, id: &str, key: &str) -> Option<Value> {
        self.store(id).get(key).cloned()
    }

    /// Set (or with Null, remove) one value. An app keeps at most 1 MB.
    pub fn storage_set(&self, id: &str, key: &str, value: Value) -> Result<()> {
        if self.get(id).is_none() {
            bail!("no app {id}");
        }
        let _g = self.lock.lock().unwrap();
        let mut s = self.store(id);
        if value.is_null() {
            s.remove(key);
        } else {
            s.insert(key.to_string(), value);
        }
        let bytes = serde_json::to_vec(&s)?;
        if bytes.len() > 1 << 20 {
            bail!("app storage is full (1 MB)");
        }
        let p = self.store_path(id);
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, bytes)?;
        Ok(())
    }

    /// Run one of an app's tools with `input` on stdin.
    pub fn run_tool(&self, id: &str, tool: &str, input: &Value) -> Result<String> {
        use std::io::Write;
        let a = self.get(id).filter(|a| a.enabled).ok_or_else(|| anyhow!("no app {id}"))?;
        let t = a
            .manifest
            .tools
            .iter()
            .find(|t| t.name == tool)
            .ok_or_else(|| anyhow!("{id} has no tool {tool}"))?;
        let mut child = std::process::Command::new(&t.command[0])
            .args(&t.command[1..])
            .current_dir(&a.dir)
            .env("PATH", crate::harnesses::path_env())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {}", t.command[0]))?;
        child.stdin.take().unwrap().write_all(input.to_string().as_bytes())?;
        let out = child.wait_with_output()?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        if !out.status.success() {
            bail!("{tool} failed: {}{}", text, String::from_utf8_lossy(&out.stderr));
        }
        Ok(text)
    }
}

fn placeholder() -> Manifest {
    Manifest {
        schema: 1,
        id: String::new(),
        name: "x".into(),
        version: String::new(),
        description: String::new(),
        author: String::new(),
        homepage: String::new(),
        icon: Icon::default(),
        view: View {
            entry: "index.html".into(),
            url: String::new(),
        },
        agent: None,
        permissions: vec![],
        tools: vec![],
        files: vec![],
    }
}

fn copy_tree(from: &Path, to: &Path, depth: usize) -> Result<()> {
    if depth > 8 {
        bail!("{} is nested too deep", from.display());
    }
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let name = e.file_name();
        let n = name.to_string_lossy();
        if n.starts_with('.') || n == "node_modules" || n == "target" {
            continue;
        }
        let ft = e.file_type()?;
        if ft.is_dir() {
            copy_tree(&e.path(), &to.join(&name), depth + 1)?;
        } else if ft.is_file() {
            std::fs::copy(e.path(), to.join(&name))?;
        }
    }
    Ok(())
}

pub fn mime_of(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "wasm" => "application/wasm",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bs-apps-{name}-{}", crate::chat::now_ms()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn manifest_rules() {
        let ok = r#"{"id":"prompt-lab","name":"Prompt Lab","view":{"entry":"index.html"}}"#;
        assert!(Manifest::parse(ok).is_ok());
        for bad in [
            r#"{"id":"Bad Id","name":"x","view":{"entry":"a.html"}}"#,
            r#"{"id":"x","name":"x","view":{}}"#,
            r#"{"id":"x","name":"x","view":{"entry":"../a.html"}}"#,
            r#"{"id":"x","name":"x","view":{"entry":"/etc/passwd"}}"#,
            r#"{"id":"x","name":"x","view":{"url":"file:///a"}}"#,
            r#"{"schema":9,"id":"x","name":"x","view":{"entry":"a.html"}}"#,
        ] {
            assert!(Manifest::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn install_serve_store_remove() {
        let src = tmp("src");
        std::fs::write(
            src.join(MANIFEST),
            r#"{"id":"demo","name":"Demo","view":{"entry":"index.html"},"permissions":["storage"],
                "tools":[{"name":"echo","command":["cat"]}]}"#,
        )
        .unwrap();
        std::fs::write(src.join("index.html"), "<h1>hi</h1>").unwrap();
        let apps = Apps::open(tmp("dest"));
        let a = apps.install_dir(&src).unwrap();
        assert_eq!(a.manifest.name, "Demo");
        let (body, mime) = apps.file("demo", "").unwrap();
        assert_eq!(body, b"<h1>hi</h1>");
        assert!(mime.starts_with("text/html"));
        assert!(apps.file("demo", "../../etc/passwd").is_err());
        apps.storage_set("demo", "k", serde_json::json!({"n": 1})).unwrap();
        assert!(apps.file("demo", ".data/storage.json").is_err());
        // Reinstalling keeps stored values.
        apps.install_dir(&src).unwrap();
        assert_eq!(apps.storage_get("demo", "k").unwrap()["n"], 1);
        #[cfg(unix)]
        assert_eq!(apps.run_tool("demo", "echo", &serde_json::json!({"a": 2})).unwrap(), r#"{"a":2}"#);
        assert_eq!(apps.list().len(), 1);
        apps.remove("demo").unwrap();
        assert!(apps.list().is_empty());
        assert!(apps.remove("../x").is_err());
    }
}
