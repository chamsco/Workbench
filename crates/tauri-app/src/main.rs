//! Tauri shell for Backspace. The UI in `ui/` draws; this file exposes the
//! core to it: chats, the project open on this machine and the machines
//! followed (`Fleet`), the CLI/model scan, Cloud plans. Commands for reads
//! and actions, and a "state" event whenever anything changes.

use std::path::PathBuf;
use std::sync::Arc;

use backspace_core::agents::{Agent, ComputerStatus};
use backspace_core::apps::Installed;
use backspace_core::chat::{Attachment, Route, Scope, Thread, ThreadInfo};
use backspace_core::memory::Note;
use backspace_core::cloud::{Account, Overage, Plan, PlanInfo};
use backspace_core::diagram::{self, Diagram};
use backspace_core::files::FileEntry;
use backspace_core::fleet::{Fleet, MachineInfo};
use backspace_core::harnesses::HarnessInfo;
use backspace_core::prefs::RouterCfg;
use backspace_core::prefs::{Prefs, TabSpec};
use backspace_core::update::UpdateInfo;
use backspace_core::{Harness, ProjectState};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

type Res<T> = Result<T, String>;
type F<'a> = State<'a, Arc<Fleet>>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn snapshot(f: F) -> ProjectState {
    f.snapshot()
}

#[tauri::command]
fn send(f: F, text: String) {
    f.backend().send(text);
}

#[tauri::command]
fn approve(f: F, id: usize) {
    f.backend().approve(id);
}

#[tauri::command]
fn reject(f: F, id: usize, feedback: String) {
    f.backend().reject(id, feedback);
}

#[tauri::command]
fn file_ticket(f: F, title: String, body: String) -> Res<String> {
    f.backend().file_ticket(&title, &body).map_err(err)
}

#[tauri::command]
async fn post_message(f: F<'_>, to: String, text: String) -> Res<()> {
    let b = f.backend();
    tauri::async_runtime::spawn_blocking(move || b.post_message(&to, &text))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
fn diagram(f: F, id: usize) -> Res<Diagram> {
    let s = f.snapshot();
    let ap = s.approvals.get(id).ok_or("no such approval")?;
    Ok(diagram::for_approval(&s, ap))
}

#[tauri::command]
async fn list_files(f: F<'_>, agent: usize) -> Res<Vec<FileEntry>> {
    let b = f.backend();
    tauri::async_runtime::spawn_blocking(move || b.list_files(agent))
        .await
        .map_err(err)
}

#[tauri::command]
async fn read_file(f: F<'_>, path: String) -> Res<String> {
    let b = f.backend();
    tauri::async_runtime::spawn_blocking(move || b.read_file(&path))
        .await
        .map_err(err)?
        .map_err(err)
}

// ---------------------------------------------------------------- machines

#[tauri::command]
fn machines(f: F) -> Vec<MachineInfo> {
    f.machines()
}

#[tauri::command]
fn select_machine(f: F, index: usize) {
    f.select(index);
}

#[tauri::command]
async fn add_machine(f: F<'_>, name: String, url: String, token: String) -> Res<usize> {
    let f = f.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f.add_machine(&name, &url, &token))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
fn remove_machine(f: F, index: usize) {
    f.remove_machine(index);
}

#[derive(serde::Serialize)]
struct ShareStatus {
    enabled: bool,
    addr: String,
    token: String,
    sharing: bool,
    error: Option<String>,
}

#[tauri::command]
fn share_status(f: F) -> ShareStatus {
    let p = f.prefs().share;
    ShareStatus {
        enabled: p.enabled,
        addr: p.addr,
        token: p.token,
        sharing: f.sharing(),
        error: f.share_error(),
    }
}

#[tauri::command]
fn set_share(f: F, enabled: bool, addr: String, new_token: bool) -> Res<()> {
    f.set_share(enabled, Some(&addr), new_token).map_err(err)
}

// ---------------------------------------------------------------- prefs, updates

#[tauri::command]
fn prefs(f: F) -> Prefs {
    f.prefs()
}

#[tauri::command]
fn set_tabs(f: F, tabs: Vec<TabSpec>, active: usize) {
    // Bench runs keep their scripted tab out of the user's prefs.
    if std::env::var_os("BACKSPACE_LAYOUT").is_some() {
        return;
    }
    f.update_prefs(|p| {
        if !tabs.is_empty() {
            p.active_tab = active.min(tabs.len() - 1);
            p.tabs = tabs;
        }
    });
}

#[tauri::command]
fn set_theme(f: F, theme: String) {
    f.update_prefs(|p| p.theme = theme);
}

#[tauri::command]
fn set_check_updates(f: F, on: bool) {
    f.update_prefs(|p| p.check_updates = on);
}

#[tauri::command]
async fn check_update(f: F<'_>) -> Res<UpdateInfo> {
    let f = f.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f.check_update())
        .await
        .map_err(err)?
        .map_err(err)
}

/// Opens http(s) links in the system browser; nothing else.
#[tauri::command]
fn open_url(url: String) -> Res<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("only web links open from here".into());
    }
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(&url).spawn().map(|_| ()).map_err(err)
}

/// Run a blocking core call off the UI thread.
async fn blocking<T: Send + 'static>(
    f: F<'_>,
    job: impl FnOnce(&Fleet) -> anyhow::Result<T> + Send + 'static,
) -> Res<T> {
    let f = f.inner().clone();
    tauri::async_runtime::spawn_blocking(move || job(&f))
        .await
        .map_err(err)?
        .map_err(err)
}

// ---------------------------------------------------------------- projects

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Open a project folder")
            .blocking_pick_folder()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.display().to_string())
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
async fn open_project(f: F<'_>, path: String) -> Res<()> {
    blocking(f, move |f| f.open_project(&path)).await
}

#[tauri::command]
fn close_project(f: F) {
    f.close_project();
}

#[tauri::command]
fn forget_project(f: F, path: String) {
    f.forget_project(&path);
}

#[tauri::command]
fn has_project(f: F) -> bool {
    f.local().is_some()
}

// ---------------------------------------------------------------- CLIs, models, routers

#[tauri::command]
fn harnesses(f: F) -> Vec<HarnessInfo> {
    f.harnesses()
}

#[tauri::command]
async fn rescan(f: F<'_>) -> Res<Vec<HarnessInfo>> {
    blocking(f, |f| Ok(f.rescan())).await
}

/// Bring an agent that lives elsewhere: read its A2A card and keep it.
#[tauri::command]
async fn connect_agent(f: F<'_>, url: String, token: String) -> Res<backspace_core::prefs::RemoteAgent> {
    blocking(f, move |f| f.connect_agent(&url, &token)).await
}

#[tauri::command]
fn remove_agent_connection(f: F, id: String) {
    f.remove_agent_connection(&id);
}

#[tauri::command]
fn set_harness(f: F, id: String, on: bool) -> Vec<HarnessInfo> {
    f.set_harness(&id, on)
}

#[tauri::command]
async fn add_router(f: F<'_>, name: String, base_url: String, api_key: String) -> Res<HarnessInfo> {
    blocking(f, move |f| {
        let name = name.trim().to_string();
        let mut base = base_url.trim().trim_end_matches('/').to_string();
        if name.is_empty() || base.is_empty() {
            anyhow::bail!("a router needs a name and a base URL");
        }
        if !base.contains("://") {
            base = format!("https://{base}");
        }
        let id: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let r = RouterCfg {
            id: format!("{id}-{}", backspace_core::chat::now_ms() % 10000),
            name,
            base_url: base,
            api_key: api_key.trim().to_string(),
            models: vec![],
        };
        let info = backspace_core::harnesses::probe_router(&r);
        if info.auth != backspace_core::harnesses::Auth::Authenticated {
            anyhow::bail!(info
                .message
                .unwrap_or_else(|| "the router did not answer".into()));
        }
        f.update_prefs(|p| p.routers.push(r));
        f.rescan();
        Ok(info)
    })
    .await
}

#[tauri::command]
async fn remove_router(f: F<'_>, id: String) -> Res<()> {
    blocking(f, move |f| {
        let rid = id.trim_start_matches("router:").to_string();
        f.update_prefs(|p| {
            p.routers.retain(|r| r.id != rid);
            p.harnesses.remove(&format!("router:{rid}"));
        });
        f.rescan();
        Ok(())
    })
    .await
}

#[tauri::command]
async fn set_ollama_url(f: F<'_>, url: String) -> Res<()> {
    blocking(f, move |f| {
        f.update_prefs(|p| p.ollama_url = url.trim().trim_end_matches('/').to_string());
        f.rescan();
        Ok(())
    })
    .await
}

#[tauri::command]
fn set_onboarded(f: F, done: bool) {
    f.update_prefs(|p| p.onboarded = done);
}

#[tauri::command]
fn set_uses(f: F, uses: Vec<String>) {
    let uses: Vec<String> = uses
        .into_iter()
        .filter(|u| u == "chat" || u == "code")
        .collect();
    if !uses.is_empty() {
        f.update_prefs(|p| p.uses = uses);
    }
}

#[tauri::command]
fn set_worker(f: F, worker: Option<String>) {
    f.update_prefs(|p| p.worker = worker.filter(|w| !w.is_empty()));
}

/// Which API keys are set (never their values).
#[tauri::command]
fn api_keys(f: F) -> Vec<(String, bool, bool)> {
    let saved = f.prefs().api_keys;
    [
        "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY",
        "TYPESAFE_API_KEY",
    ]
    .iter()
    .map(|k| {
        let in_app = saved.get(*k).is_some_and(|v| !v.is_empty());
        let env = std::env::var(k).is_ok_and(|v| !v.is_empty());
        (k.to_string(), in_app, env)
    })
    .collect()
}

#[tauri::command]
fn set_api_key(f: F, name: String, value: String) -> Res<()> {
    if ![
        "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY",
        "TYPESAFE_API_KEY",
    ]
    .contains(&name.as_str())
    {
        return Err("unknown key".into());
    }
    let v = value.trim().to_string();
    f.update_prefs(|p| {
        if v.is_empty() {
            p.api_keys.remove(&name);
        } else {
            p.api_keys.insert(name.clone(), v.clone());
        }
    });
    if v.is_empty() {
        std::env::remove_var(&name);
    } else {
        std::env::set_var(&name, &v);
    }
    Ok(())
}

#[tauri::command]
fn set_mode(f: F, mode: String) {
    f.update_prefs(|p| p.mode = mode);
}

#[tauri::command]
fn set_code_view(f: F, view: String) {
    f.update_prefs(|p| p.code_view = view);
}

#[tauri::command]
fn set_split(f: F, split: Option<backspace_core::prefs::SplitCfg>) {
    f.update_prefs(|p| p.split = split);
}

#[tauri::command]
fn set_pinned_apps(f: F, ids: Vec<String>) {
    f.update_prefs(|p| p.pinned_apps = ids);
}

#[tauri::command]
fn set_view_prefs(f: F, chat_view: Option<String>, motion: Option<String>) {
    f.update_prefs(|p| {
        if let Some(v) = chat_view {
            p.chat_view = v;
        }
        if let Some(m) = motion {
            p.motion = m;
        }
    });
}

#[tauri::command]
fn set_memory_on(f: F, on: bool) {
    f.update_prefs(|p| p.memory_on = on);
}

#[tauri::command]
fn set_default_route(f: F, route: Option<Route>) {
    f.update_prefs(|p| p.default_route = route);
}

// ---------------------------------------------------------------- chat

#[derive(serde::Deserialize)]
struct NewFile {
    name: String,
    mime: String,
    /// data: URL or bare base64.
    data: String,
}

#[tauri::command]
fn chat_list(f: F) -> Vec<ThreadInfo> {
    f.chats().list()
}

#[tauri::command]
fn chat_thread(f: F, id: String) -> Option<Thread> {
    f.chats().thread(&id)
}

#[tauri::command]
fn chat_new(f: F, route: Route, scope: Option<Scope>) -> Thread {
    f.chats().create_in(route, scope.unwrap_or_default())
}

// ---------------------------------------------------------------- memory

#[tauri::command]
fn memory_list(f: F) -> Vec<Note> {
    f.memory().list()
}

#[tauri::command]
fn memory_add(f: F, text: String, project: Option<String>, source: Option<String>) -> Res<Note> {
    f.memory()
        .add(&text, project, source.as_deref().unwrap_or(""))
        .map_err(err)
}

#[tauri::command]
fn memory_update(
    f: F,
    id: String,
    text: Option<String>,
    // "" moves the note to Everywhere; missing leaves it where it is.
    project: Option<String>,
    on: Option<bool>,
) -> Res<Note> {
    let project = project.map(|p| Some(p).filter(|p| !p.is_empty()));
    f.memory().update(&id, text, project, on).map_err(err)
}

#[tauri::command]
fn memory_delete(f: F, id: String) -> Res<()> {
    f.memory().delete(&id).map_err(err)
}

/// Dreams so far (newest first), whether one is running, and where the
/// memory repo is.
#[tauri::command]
fn memory_dreams(f: F) -> serde_json::Value {
    serde_json::json!({
        "dreams": f.dreams(),
        "dreaming": f.is_dreaming(),
        "repo": f.memory().root().display().to_string(),
    })
}

#[tauri::command]
async fn memory_dream(f: F<'_>) -> Res<backspace_core::memory_dream::Dream> {
    blocking(f, |f| f.dream()).await
}

#[tauri::command]
fn memory_undo_dream(f: F, at: u64) -> Res<()> {
    f.undo_dream(at).map_err(err)
}

#[tauri::command]
fn set_memory_dream(f: F, on: bool) {
    f.update_prefs(|p| p.memory_dream = on);
}

/// Show the memory repo in the file manager.
#[tauri::command]
fn memory_reveal(f: F) -> Res<()> {
    let dir = f.memory().root().to_path_buf();
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        std::process::Command::new("explorer")
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(&dir).spawn().map(|_| ()).map_err(err)
}

// ---------------------------------------------------------------- tracing

#[tauri::command]
fn trace_get(f: F, id: String) -> Option<backspace_core::trace::Trace> {
    f.chats().trace(&id)
}

#[tauri::command]
fn set_tracing(f: F, cfg: backspace_core::trace::Export) {
    f.update_prefs(|p| p.tracing = cfg);
}

/// Send one small trace to the collector in `cfg`, to check the address and sign-in.
#[tauri::command]
async fn trace_test(cfg: backspace_core::trace::Export) -> Res<()> {
    backspace_core::trace::send_test(&cfg).await.map_err(err)
}

// ---------------------------------------------------------------- companion

#[tauri::command]
fn companion_status(f: F) -> serde_json::Value {
    f.companion_status()
}

#[tauri::command]
fn set_companion(f: F, enabled: bool, new_token: bool) -> serde_json::Value {
    f.inner().set_companion(enabled, new_token);
    f.companion_status()
}

/// A QR code as SVG, for the pairing link.
#[tauri::command]
fn qr_svg(text: String) -> Res<String> {
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(err)?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(200, 200)
        .quiet_zone(true)
        .build())
}

// ---------------------------------------------------------------- apps

#[tauri::command]
fn apps_list(f: F) -> Vec<Installed> {
    f.apps().list()
}

#[tauri::command]
async fn app_install_url(f: F<'_>, url: String) -> Res<Installed> {
    blocking(f, move |f| f.install_app_url(&url)).await
}

#[tauri::command]
async fn app_install_dir(f: F<'_>, path: String) -> Res<Installed> {
    blocking(f, move |f| f.apps().install_dir(std::path::Path::new(&path))).await
}

#[tauri::command]
async fn pick_app(app: tauri::AppHandle) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Choose an app folder (with backspace-app.json)")
            .blocking_pick_folder()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.display().to_string())
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
fn app_remove(f: F, id: String) -> Res<()> {
    f.apps().remove(&id).map_err(err)?;
    f.update_prefs(|p| p.pinned_apps.retain(|x| x != &id));
    Ok(())
}

#[tauri::command]
fn app_storage_get(f: F, id: String, key: String) -> Option<serde_json::Value> {
    f.apps().storage_get(&id, &key)
}

#[tauri::command]
fn app_storage_set(f: F, id: String, key: String, value: serde_json::Value) -> Res<()> {
    f.apps().storage_set(&id, &key, value).map_err(err)
}

/// The example apps that ship in the binary, installable without a network.
#[tauri::command]
fn app_examples() -> Vec<serde_json::Value> {
    EXAMPLES
        .iter()
        .filter_map(|(_, files)| {
            files
                .iter()
                .find(|(n, _)| *n == backspace_core::apps::MANIFEST)
                .and_then(|(_, b)| serde_json::from_slice(b).ok())
        })
        .collect()
}

#[tauri::command]
async fn app_install_example(f: F<'_>, id: String) -> Res<Installed> {
    blocking(f, move |f| {
        let (_, files) = EXAMPLES
            .iter()
            .find(|(i, _)| *i == id)
            .ok_or_else(|| anyhow::anyhow!("no example {id}"))?;
        let dir = std::env::temp_dir().join(format!("backspace-example-{id}"));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, body) in files.iter() {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap())?;
            std::fs::write(p, body)?;
        }
        let r = f.apps().install_dir(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        r
    })
    .await
}

const SDK: &str = include_str!("../../../apps/sdk.js");

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const EXAMPLES: &[(&str, &[(&str, &[u8])])] = &[(
    "prompt-lab",
    &[
        ("backspace-app.json", include_bytes!("../../../apps/prompt-lab/backspace-app.json")),
        ("index.html", include_bytes!("../../../apps/prompt-lab/index.html")),
    ],
)];

#[tauri::command]
async fn chat_send(
    f: F<'_>,
    id: String,
    text: String,
    files: Vec<NewFile>,
    reply_to: Option<String>,
) -> Res<Option<Note>> {
    blocking(f, move |f| {
        let mut atts: Vec<Attachment> = Vec::new();
        for nf in files {
            atts.push(f.chats().attach(&id, &nf.name, &nf.mime, &nf.data)?);
        }
        f.chats().send(&id, &text, atts, reply_to, f.chat_ctx())?;
        // Anything worth keeping in what you said? Decided on this machine;
        // the app shows "… will remember that" with a way to say no.
        let prefs = f.prefs();
        if !(prefs.memory_on && prefs.memory_auto) {
            return Ok(None);
        }
        let t = f.chats().thread(&id);
        if t.as_ref().is_some_and(|t| t.app.is_some()) {
            return Ok(None);
        }
        let scope = t.and_then(|t| t.agent.map(|a| format!("agent:{a}")).or(t.project));
        Ok(f.memory().capture(&text, scope, Some(id.clone())))
    })
    .await
}

#[tauri::command]
fn memory_forget(f: F, id: String) -> Res<()> {
    f.memory().forget(&id).map_err(err)
}

#[tauri::command]
fn memory_confirm(f: F, id: String) -> Res<()> {
    f.memory().confirm(&id).map_err(err)
}

#[tauri::command]
fn set_memory_auto(f: F, on: bool) {
    f.update_prefs(|p| p.memory_auto = on);
}

// ---------------------------------------------------------------- agents

#[tauri::command]
fn agents_list(f: F) -> Vec<Agent> {
    f.agents().list()
}

#[tauri::command]
fn agent_save(f: F, agent: Agent) -> Res<Agent> {
    f.agents().save(agent).map_err(err)
}

#[tauri::command]
async fn agent_delete(f: F<'_>, id: String) -> Res<()> {
    blocking(f, move |f| f.agents().delete(&id)).await
}

#[tauri::command]
async fn agent_computer(f: F<'_>, id: String, action: String) -> Res<ComputerStatus> {
    blocking(f, move |f| match action.as_str() {
        "start" => f.agents().start(&id),
        "stop" => f.agents().stop(&id),
        _ => f.agents().status(&id),
    })
    .await
}

#[tauri::command]
fn chat_stop(f: F, id: String) {
    f.chats().stop(&id);
}

#[tauri::command]
fn chat_retry(f: F, id: String) -> Res<()> {
    f.chats().retry(&id, f.chat_ctx()).map_err(err)
}

/// Carry on with a stopped or cut-off reply.
#[tauri::command]
fn chat_resume(f: F, id: String) -> Res<()> {
    f.chats().resume(&id, f.chat_ctx()).map_err(err)
}

#[tauri::command]
fn chat_edit(f: F, id: String, msg: String, text: String) -> Res<()> {
    f.chats().edit(&id, &msg, &text, f.chat_ctx()).map_err(err)
}

#[tauri::command]
fn chat_react(f: F, id: String, msg: String, emoji: String) -> Res<()> {
    f.chats().react(&id, &msg, &emoji).map_err(err)
}

#[tauri::command]
fn chat_rename(f: F, id: String, title: String) -> Res<()> {
    f.chats().rename(&id, &title).map_err(err)
}

#[tauri::command]
fn chat_pin(f: F, id: String, pinned: bool) -> Res<()> {
    f.chats().pin(&id, pinned).map_err(err)
}

#[tauri::command]
fn chat_delete(f: F, id: String) -> Res<()> {
    f.chats().delete(&id).map_err(err)
}

#[tauri::command]
fn chat_branch(f: F, id: String, msg: String) -> Res<Thread> {
    f.chats().branch(&id, &msg).map_err(err)
}

#[tauri::command]
fn chat_set_route(f: F, id: String, route: Route) -> Res<()> {
    f.chats().set_route(&id, route).map_err(err)
}

/// An attachment as a data: URL, for showing it in the thread.
#[tauri::command]
async fn chat_file(f: F<'_>, id: String, file: String) -> Res<String> {
    blocking(f, move |f| {
        let p = f.chats().attachment_path(&id, &file)?;
        let bytes = std::fs::read(&p)?;
        let mime = f
            .chats()
            .thread(&id)
            .and_then(|t| {
                t.messages
                    .iter()
                    .flat_map(|m| m.attachments.iter())
                    .find(|a| a.id == file)
                    .map(|a| a.mime.clone())
            })
            .unwrap_or_else(|| "application/octet-stream".into());
        Ok(format!(
            "data:{mime};base64,{}",
            backspace_core::chat::b64_encode(&bytes)
        ))
    })
    .await
}

#[tauri::command]
async fn link_preview(f: F<'_>, url: String) -> Res<backspace_core::chat::LinkPreview> {
    blocking(f, move |f| f.link_preview(&url)).await
}

// ---------------------------------------------------------------- cloud

#[tauri::command]
fn plans() -> Vec<PlanInfo> {
    backspace_core::cloud::PLANS.to_vec()
}

#[tauri::command]
fn cloud_cached(f: F) -> Option<Account> {
    f.chats().account()
}

#[tauri::command]
async fn cloud_account(f: F<'_>) -> Res<Option<Account>> {
    blocking(f, |f| f.cloud_account()).await
}

#[tauri::command]
async fn cloud_signup(f: F<'_>) -> Res<Account> {
    blocking(f, |f| f.cloud_signup()).await
}

#[tauri::command]
async fn cloud_set_plan(f: F<'_>, plan: Plan, overage: Overage) -> Res<Account> {
    blocking(f, move |f| f.cloud_set_plan(plan, overage)).await
}

#[tauri::command]
fn cloud_sign_out(f: F) {
    f.cloud_sign_out();
}

// ---------------------------------------------------------------- boot

/// Platform class for the stylesheet, plus the bench's starting layout.
#[tauri::command]
fn boot() -> serde_json::Value {
    let platform = if cfg!(target_os = "macos") {
        "mac"
    } else if cfg!(windows) {
        "win"
    } else {
        "linux"
    };
    let layout = std::env::var("BACKSPACE_LAYOUT")
        .ok()
        .and_then(|v| v.parse::<u8>().ok());
    let bench = std::env::var_os("BACKSPACE_READY_FILE").is_some();
    // First name for the chat greeting: the login name, capitalised.
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|u| !u.is_empty() && u != "root")
        .map(|u| {
            let mut c = u.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or(u)
        })
        .unwrap_or_else(|| "there".into());
    serde_json::json!({ "platform": platform, "layout": layout, "bench": bench, "user": user })
}

/// The UI's first frame with data is on screen; bench/ times launch to this.
#[tauri::command]
fn ready() {
    if let Ok(path) = std::env::var("BACKSPACE_READY_FILE") {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let _ = std::fs::write(path, ms.to_string());
    }
}

fn main() -> anyhow::Result<()> {
    // Agents in coding CLIs reach the message board through any Backspace
    // binary: `msg ...` and `mcp`.
    if let Some(code) =
        backspace_core::board::client_main(&std::env::args().skip(1).collect::<Vec<_>>())
    {
        std::process::exit(code);
    }
    // A folder on the command line opens it; otherwise the app starts with
    // no project (never the launch folder, which is `/` from the Dock).
    let ws = std::env::args().nth(1).map(PathBuf::from);
    // Keys saved in the app, before any harness reads its config.
    Prefs::load().apply_keys();

    // WebKitGTK's DMA-BUF renderer draws nothing on X servers without DRI3
    // (Xvfb, some VMs); the fallback costs nothing elsewhere.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    let harness = ws.map(Harness::open).transpose()?;
    // Scripted runs (bench/) hand the goal over without typing it.
    if let (Some(h), Ok(goal)) = (&harness, std::env::var("BACKSPACE_GOAL")) {
        h.send(goal);
    }
    let mut loaded = Prefs::load();
    if let Some(h) = &harness {
        loaded.touch_project(&h.root().display().to_string());
        loaded.save();
    }
    let fleet = Fleet::new(harness, loaded)?;
    // The first scan runs in the background; Setup and Settings show it.
    {
        let f = fleet.clone();
        std::thread::spawn(move || f.rescan());
    }
    let changes = fleet.changes();

    // App views: bsapp://localhost/<id>/<path> (http://bsapp.localhost/...
    // on Windows), served from the installed app's folder into a sandboxed
    // frame. The frame has no IPC of its own; it talks to the page through
    // postMessage (apps/sdk.js), and the page decides what it may do.
    let apps = fleet.apps().clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("bsapp", move |_ctx, req| {
            use tauri::http::Response;
            let path = req.uri().path().trim_start_matches('/').to_string();
            let path = percent_decode(&path);
            let (id, rest) = path.split_once('/').unwrap_or((path.as_str(), ""));
            let res = if rest == "__backspace/sdk.js" {
                Ok((SDK.as_bytes().to_vec(), "text/javascript; charset=utf-8"))
            } else {
                apps.file(id, rest)
            };
            match res {
                Ok((body, mime)) => Response::builder()
                    .status(200)
                    .header("Content-Type", mime)
                    .header("Cache-Control", "no-cache")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(body)
                    .unwrap(),
                Err(e) => Response::builder()
                    .status(404)
                    .header("Content-Type", "text/plain")
                    .body(e.to_string().into_bytes())
                    .unwrap(),
            }
        })
        .manage(fleet)
        .invoke_handler(tauri::generate_handler![
            snapshot,
            send,
            approve,
            reject,
            file_ticket,
            post_message,
            diagram,
            list_files,
            read_file,
            machines,
            select_machine,
            add_machine,
            remove_machine,
            share_status,
            set_share,
            prefs,
            set_tabs,
            set_theme,
            set_check_updates,
            check_update,
            open_url,
            pick_folder,
            open_project,
            close_project,
            forget_project,
            has_project,
            harnesses,
            rescan,
            set_harness,
            add_router,
            remove_router,
            set_ollama_url,
            set_onboarded,
            set_mode,
            set_code_view,
            set_split,
            set_pinned_apps,
            set_memory_on,
            set_view_prefs,
            set_uses,
            set_worker,
            api_keys,
            set_api_key,
            set_default_route,
            chat_list,
            chat_thread,
            chat_new,
            memory_list,
            memory_forget,
            memory_dreams,
            trace_get,
            set_tracing,
            trace_test,
            memory_dream,
            memory_undo_dream,
            set_memory_dream,
            memory_reveal,
            memory_confirm,
            set_memory_auto,
            agents_list,
            agent_save,
            agent_delete,
            agent_computer,
            companion_status,
            set_companion,
            qr_svg,
            memory_add,
            memory_update,
            memory_delete,
            apps_list,
            app_install_url,
            app_install_dir,
            pick_app,
            app_remove,
            app_storage_get,
            app_storage_set,
            app_examples,
            app_install_example,
            chat_send,
            chat_stop,
            chat_retry,
            chat_resume,
            connect_agent,
            remove_agent_connection,
            chat_edit,
            chat_react,
            chat_rename,
            chat_pin,
            chat_delete,
            chat_branch,
            chat_set_route,
            chat_file,
            link_preview,
            plans,
            cloud_cached,
            cloud_account,
            cloud_signup,
            cloud_set_plan,
            cloud_sign_out,
            boot,
            ready
        ])
        .setup(move |app| {
            let win = app.get_webview_window("main").expect("main window");
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                let _ = apply_vibrancy(&win, NSVisualEffectMaterial::Sidebar, None, None);
            }
            // The fleet coalesces changes; the UI coalesces again per frame.
            std::thread::spawn(move || {
                while changes.recv_blocking().is_ok() {
                    let _ = win.emit("state", ());
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}
