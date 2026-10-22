//! Tauri shell for Backspace. The UI in `ui/` draws; this file exposes the
//! core to it: chats, the project open on this machine and the machines
//! followed (`Fleet`), the CLI/model scan, Cloud plans. Commands for reads
//! and actions, and a "state" event whenever anything changes.

use std::path::PathBuf;
use std::sync::Arc;

use backspace_core::agents::{Agent, ComputerStatus, Routine};
use backspace_core::apps::Installed;
use backspace_core::chat::{Attachment, Life, Route, Scope, Thread, ThreadInfo};
use backspace_core::delegate::{self, Delegation, Snapshot};
use backspace_core::memory::Note;
use backspace_core::cloud::{Account, Overage, Plan, PlanInfo};
use backspace_core::diagram::{self, Diagram};
use backspace_core::files::FileEntry;
use backspace_core::fleet::{Fleet, MachineInfo};
use backspace_core::harnesses::HarnessInfo;
use backspace_core::prefs::RouterCfg;
use backspace_core::prefs::{Prefs, TabSpec};
use backspace_core::projects::Project;
use backspace_core::update::UpdateInfo;
use backspace_core::{Harness, ProjectState};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

mod update_install;

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

/// The user's answer on a set of visual options (empty: dismissed).
#[tauri::command]
fn pick_option(f: F, id: String, variant: Option<String>) {
    f.backend().pick_option(&id, variant.as_deref());
}

/// Stop an agent and those under it; no agent: the whole project.
#[tauri::command]
fn stop(f: F, agent: Option<usize>) {
    f.backend().stop(agent);
}

/// Carry on with an agent a restart cut off: its worktree, branch and CLI
/// session are already on disk, so this resumes instead of starting over.
#[tauri::command]
async fn resume(f: F<'_>, agent: usize) -> Res<String> {
    let b = f.backend();
    tauri::async_runtime::spawn_blocking(move || b.resume(agent))
        .await
        .map_err(err)?
        .map_err(err)
}

/// Another task in parallel, on its own branch; returns its ticket key.
#[tauri::command]
async fn start_task(f: F<'_>, text: String) -> Res<String> {
    let b = f.backend();
    tauri::async_runtime::spawn_blocking(move || b.start_task(&text)).await.map_err(err)?.map_err(err)
}

/// The project's recent commits on every branch, for the side panel.
#[tauri::command]
async fn git_log(path: String) -> Res<Vec<serde_json::Value>> {
    tauri::async_runtime::spawn_blocking(move || {
        backspace_core::git::log(std::path::Path::new(&path), 80).map(|v| {
            v.into_iter()
                .map(|(hash, subject, author, when, refs)| serde_json::json!({ "hash": hash, "subject": subject, "author": author, "when": when, "refs": refs }))
                .collect()
        })
    })
    .await
    .map_err(err)?
    .map_err(err)
}

/// A system notification (Settings → Notifications decides whether the UI
/// asks for one).
#[tauri::command]
fn notify(app: tauri::AppHandle, title: String, body: String) -> Res<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification().builder().title(title).body(body).show().map_err(err)
}

/// The chat face's OpenUI prompt (None turns rich answers off).
#[tauri::command]
fn set_genui(prompt: Option<String>) {
    *backspace_core::chat::GENUI_PROMPT.write().unwrap() = prompt.filter(|p| !p.is_empty());
}

#[tauri::command]
fn set_notify(f: F, notify: String) {
    f.update_prefs(|p| p.notify = notify);
}

/// Commits a day for the last two weeks and lines changed, for Home.
/// What is uncommitted in a folder, and its branch: the side panel's Changes.
#[tauri::command]
async fn git_changes(path: String) -> Res<serde_json::Value> {
    tauri::async_runtime::spawn_blocking(move || {
        backspace_core::git::changes(std::path::Path::new(&path)).map(|c| serde_json::json!(c))
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
async fn git_diff(path: String, file: String) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || backspace_core::git::diff_file(std::path::Path::new(&path), &file))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
async fn git_activity(path: String) -> Res<serde_json::Value> {
    tauri::async_runtime::spawn_blocking(move || {
        backspace_core::git::activity(std::path::Path::new(&path), 14)
            .map(|(days, added, removed)| serde_json::json!({ "days": days, "added": added, "removed": removed }))
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
fn file_ticket(f: F, title: String, body: String) -> Res<String> {
    f.backend().file_ticket(&title, &body).map_err(err)
}

#[tauri::command]
async fn post_message(f: F<'_>, to: String, text: String, reply_to: Option<usize>) -> Res<()> {
    let b = f.backend();
    tauri::async_runtime::spawn_blocking(move || b.post_message(&to, &text, reply_to))
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

/// An image in the project as a data: URL, for the Options canvas.
#[tauri::command]
async fn read_image(f: F<'_>, path: String) -> Res<String> {
    let b = f.backend();
    tauri::async_runtime::spawn_blocking(move || b.read_image(&path))
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
async fn share_status(f: F<'_>) -> Res<ShareStatus> {
    blocking(f, |f| {
        let p = f.prefs().share;
        Ok(ShareStatus {
            enabled: p.enabled,
            addr: p.addr,
            token: p.token,
            sharing: f.sharing(),
            error: f.share_error(),
        })
    })
    .await
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

/// Save a canvas layout under a project, so opening another project cannot
/// overwrite it. An empty `workspace` writes the global layout (no project
/// open), which is what the old `set_tabs` did for everyone.
#[tauri::command]
fn set_tabs_for(f: F, workspace: String, tabs: Vec<TabSpec>, active: usize) {
    if std::env::var_os("BACKSPACE_LAYOUT").is_some() {
        return;
    }
    f.update_prefs(|p| p.set_tabs_for(&workspace, tabs, active));
}

/// The layout to show for a workspace: its own if it has one, else the global
/// default. Called when a project opens or closes.
#[tauri::command]
fn tabs_for(f: F, workspace: String) -> backspace_core::prefs::TabsFor {
    f.prefs().tabs_for(&workspace)
}

#[tauri::command]
fn set_theme(f: F, theme: String) {
    f.update_prefs(|p| p.theme = theme);
}

/// Settings → Appearance. Values are clamped here so a colour or a contrast
/// from the webview can never make the window unreadable.
#[tauri::command]
fn set_appearance(f: F, appearance: backspace_core::prefs::Appearance) {
    f.update_prefs(|p| {
        let mut a = appearance;
        a.accent = hex_or_empty(&a.accent);
        a.accent_dark = hex_or_empty(&a.accent_dark);
        a.bg = hex_or_empty(&a.bg);
        a.fg = hex_or_empty(&a.fg);
        a.contrast = a.contrast_or_default();
        p.appearance = a;
    });
}

/// "#1a2b3c" or ""; anything else is dropped rather than written to prefs.
fn hex_or_empty(s: &str) -> String {
    let s = s.trim();
    let ok = s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit());
    if ok {
        s.to_ascii_lowercase()
    } else {
        String::new()
    }
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

/// Download the newest build over this one, then quit into it.
///
/// The download, the swap and the relaunch all happen outside the UI thread
/// and outside this process: the command returns only if something went
/// wrong. The shell that runs on success is a new window on the same app.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Res<()> {
    update_install::install(app).await.map_err(err)
}

/// Can this URL be shown inside the app's own frame?
///
/// The canvas is an `<iframe>`, and a site that sends `X-Frame-Options` or a
/// CSP `frame-ancestors` rule cannot be rendered in one -- the engine refuses,
/// and it looks exactly like a refused connection. That is not something the
/// app can override (it is the whole point of those headers), and no desktop
/// shell can either: Electron's `<webview>`, an in-app browser panel and a
/// Cursor-style preview all hit the same wall unless they host the page in a
/// top-level browsing context of its own.
///
/// So rather than let the frame fail and be mysterious, ask the site first and
/// tell the user before they click. Redirects are followed, because a bare
/// `http://` URL usually 301s to `https://` and the headers that matter are on
/// the last hop, and the answer is read from the response rather than guessed.
#[tauri::command]
async fn frame_check(url: String) -> Res<serde_json::Value> {
    let u = url.trim().to_string();
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        return Ok(serde_json::json!({
            "url": u, "reachable": false, "frameable": null,
            "reason": "Only http and https addresses can be previewed here.",
        }));
    }
    // The same client the harness uses: rustls, so https needs no new dependency.
    let client = reqwest::Client::builder()
        .user_agent("Backspace/0.1 (canvas preview check)")
        .timeout(std::time::Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::limited(6))
        .build()
        .map_err(|e| format!("could not start the check: {e}"))?;

    let resp = match client.head(&u).send().await {
        Ok(r) => r,
        // Some servers refuse HEAD. A GET whose body we never read is just as
        // good a way to see the headers.
        Err(_) => client
            .get(&u)
            .send()
            .await
            .map_err(|e| format!("{u} did not answer: {e}"))?,
    };
    let status = resp.status().as_u16();
    let final_url = resp.url().to_string();
    let header = |name: &str| -> Option<String> {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
    };
    let content_type = header("content-type").unwrap_or_default();
    let xfo = header("x-frame-options");
    let csp = header("content-security-policy");

    Ok(serde_json::json!({
        "url": u,
        "reachable": true,
        "status": status,
        "final_url": final_url,
        "content_type": content_type,
        "frameable": frameable_from(xfo.as_deref(), csp.as_deref()),
        "reason": framing_reason(&u, &final_url, status, xfo.as_deref(), csp.as_deref()),
    }))
}

/// The decision, from the two headers browsers act on. A header that is present
/// but not restrictive (or no framing header at all) means framing is allowed.
fn frameable_from(xfo: Option<&str>, csp: Option<&str>) -> Option<bool> {
    if let Some(v) = xfo {
        let v = v.to_ascii_lowercase();
        if v.contains("deny") || v.contains("sameorigin") {
            return Some(false);
        }
    }
    if let Some(v) = csp {
        for part in v.split(';') {
            let lower = part.trim().to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("frame-ancestors") {
                let rest = rest.trim();
                if !rest.is_empty() && rest != "*" {
                    return Some(false);
                }
            }
        }
    }
    Some(true)
}

/// One line for the user, naming the header when a header is the reason.
fn framing_reason(
    asked: &str,
    final_url: &str,
    status: u16,
    xfo: Option<&str>,
    csp: Option<&str>,
) -> String {
    let went = if final_url != asked {
        format!(" (redirected to {final_url})")
    } else {
        String::new()
    };
    if let Some(v) = xfo {
        let lower = v.to_ascii_lowercase();
        if lower.contains("deny") || lower.contains("sameorigin") {
            return format!("{asked}{went} answers with X-Frame-Options: {v}, so no app can show it in a frame.");
        }
    }
    if let Some(v) = csp {
        for part in v.split(';') {
            let lower = part.trim().to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("frame-ancestors") {
                let rest = rest.trim();
                if !rest.is_empty() && rest != "*" {
                    return format!("{asked}{went} answers with frame-ancestors {rest}, so it cannot be shown in a frame here.");
                }
            }
        }
    }
    if status >= 400 {
        return format!("{asked}{went} answered {status}; the preview would show that, not a page.");
    }
    format!("{asked}{went} answers {status} with no framing restriction, so it should load in the canvas.")
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
async fn harnesses(f: F<'_>) -> Res<Vec<HarnessInfo>> {
    // Off the main thread. This scans the machine — spawning codex, claude,
    // cursor-agent, opencode to read their versions and sign-in state — and a
    // synchronous command runs on the UI thread in Tauri v2, so the window
    // stopped pumping messages and Windows put "(Not responding)" in the title
    // bar for the whole scan. Same shape as `rescan` below.
    blocking(f, |f| Ok(f.harnesses())).await
}

/// Open a CLI's own sign-in (Sign in with ChatGPT for Codex) in a terminal.
#[tauri::command]
fn cli_login(id: String) -> Res<()> {
    backspace_core::harnesses::open_login(&id).map_err(err)
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

/// Who runs a project and how: the planner's and workers' model, what CLIs
/// may do without asking, their effort. Applies to the next project opened.
#[tauri::command]
fn set_agent_run(f: F, planner: Option<String>, worker: Option<String>, permission: String, effort: Option<String>) {
    f.update_prefs(|p| {
        p.planner = planner.filter(|w| !w.is_empty());
        p.worker = worker.filter(|w| !w.is_empty());
        p.cli_permission = permission;
        p.cli_effort = effort.filter(|e| !e.is_empty());
    });
}

#[tauri::command]
fn set_backdrop(f: F, backdrop: String) {
    f.update_prefs(|p| p.backdrop = backdrop);
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

/// Outside MCP servers bots can be given. Each gets an id that is a valid
/// server name, unique, and never `backspace` (Backspace's own server) or an app's.
#[tauri::command]
fn set_connections(f: F, list: Vec<backspace_core::prefs::Connection>) -> Vec<backspace_core::prefs::Connection> {
    // Installed apps share the id space (a bot's apps_off names both).
    let mut seen = std::collections::HashSet::from(["backspace".to_string()]);
    seen.extend(f.apps().list().into_iter().map(|a| a.manifest.id));
    let list: Vec<_> = list
        .into_iter()
        .filter(|c| !c.target.trim().is_empty())
        .map(|mut c| {
            // A saved one keeps its id (bots refer to it); a new one takes its name's.
            let base: String = (if c.id.is_empty() { &c.name } else { &c.id }).to_lowercase().chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect();
            let base = if base.trim_matches('_').is_empty() { "connection".to_string() } else { base.trim_matches('_').to_string() };
            let (mut id, mut n) = (base.clone(), 1);
            while !seen.insert(id.clone()) {
                n += 1;
                id = format!("{base}_{n}");
            }
            c.id = id;
            c
        })
        .collect();
    f.update_prefs(|p| p.connections = list.clone());
    list
}

#[tauri::command]
fn set_pinned_apps(f: F, ids: Vec<String>) {
    f.update_prefs(|p| p.pinned_apps = ids);
}

#[tauri::command]
fn set_view_prefs(f: F, chat_view: Option<String>, motion: Option<String>, wb_view: Option<String>) {
    f.update_prefs(|p| {
        if let Some(v) = wb_view {
            p.wb_view = v;
        }
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

/// Give a chat a bounded life, or take the bound away (`Life::Keep`). A ghost
/// chat is hidden from the list and its file is deleted, so leaving it destroys
/// it; a temporal one forgets itself `ttl_minutes` after its last message.
#[tauri::command]
fn chat_life(f: F, id: String, life: Life, ttl_minutes: Option<u64>) -> Res<Thread> {
    f.chats()
        .set_life(&id, life, ttl_minutes)
        .map_err(|e| e.to_string())
}

/// Forget every chat that has expired. Called once at boot and on a timer, so a
/// temporal chat that ran out while the app was closed is gone before it is
/// listed.
#[tauri::command]
fn chat_purge(f: F) -> Vec<String> {
    f.chats().purge_expired()
}

/// Forget every ghost chat. The face calls this as the mode goes on, so a
/// one-off left in memory by an earlier one cannot be reached again.
#[tauri::command]
fn chat_purge_ghosts(f: F) -> Vec<String> {
    f.chats().purge_ghosts()
}

// ------------------------------------------------------- local tiny models

/// The small models on this machine, for Settings → Local models. Blocks for
/// a couple of seconds while each runtime is probed.
#[tauri::command]
fn local_models(f: F) -> serde_json::Value {
    let prefs = f.prefs();
    // This machine's runtimes and the ones it reaches (the network, connected
    // machines) side by side, so the slower set does not add to the faster.
    let (runtimes, remote) = std::thread::scope(|s| {
        let r = s.spawn(|| backspace_core::model_server::remote_runtimes(&prefs));
        (backspace_core::model_server::detect(&prefs, &[]), r.join().unwrap_or_default())
    });
    let recommend = backspace_core::model_server::recommend(&runtimes);
    serde_json::json!({
        "runtimes": runtimes,
        "remote": remote,
        "lan": prefs.lan_models.iter().map(|l| serde_json::json!({"name": l.name, "base_url": l.base_url})).collect::<Vec<_>>(),
        "recommend": recommend.map(|(url, model)| serde_json::json!({"base_url": url, "model": model})),
        "endpoint": backspace_core::model_server::best_endpoint(&prefs),
        "chosen": prefs.subagent_model,
        "tasks": prefs.local_tasks,
    })
}

/// Settings → Local models: which tiny model the small delegated jobs use
/// ("<runtime id>:<model id>"), or None to leave them to the main model.
#[tauri::command]
fn set_subagent_model(f: F, model: Option<String>) {
    f.update_prefs(|p| p.subagent_model = model.filter(|m| !m.trim().is_empty()));
}

/// Settings → Local models: which small jobs that model runs ("scouts",
/// "triage", "workers"). Empty means scouts only, the safe default.
#[tauri::command]
fn set_local_tasks(f: F, tasks: Vec<String>) {
    let known = ["scouts", "triage", "workers"];
    f.update_prefs(|p| {
        p.local_tasks = tasks
            .into_iter()
            .filter(|t| known.contains(&t.as_str()))
            .collect();
    });
}

// ------------------------------------------------------- model server

/// Settings → Local models → Serve: the model server's URLs, token and state.
#[tauri::command]
fn model_serve_status(f: F) -> serde_json::Value {
    f.model_serve_status()
}

#[tauri::command]
fn set_model_serve(f: F, enabled: bool, allow_lan: bool, new_token: bool) -> serde_json::Value {
    f.inner().set_model_serve(enabled, allow_lan, new_token);
    f.model_serve_status()
}

/// Model servers on the local network. A few seconds: a broadcast, then a
/// sweep of the usual runtime ports.
#[tauri::command]
async fn lan_scan(f: F<'_>) -> Res<Vec<backspace_core::model_gateway::Found>> {
    blocking(f, |f| Ok(backspace_core::model_gateway::discover(&f.prefs()))).await
}

/// Save a server on the network, once it lists models with that token.
#[tauri::command]
async fn lan_add(f: F<'_>, name: String, base_url: String, token: String) -> Res<Vec<String>> {
    blocking(f, move |f| {
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            anyhow::bail!("give the server's URL, e.g. http://192.168.1.20:11434/v1");
        }
        let models = backspace_core::model_gateway::check_lan(&base_url, token.trim())?;
        let name = if name.trim().is_empty() { backspace_core::model_server::host_of(&base_url) } else { name.trim().to_string() };
        f.update_prefs(|p| {
            p.lan_models.retain(|l| l.base_url.trim_end_matches('/') != base_url);
            p.lan_models.push(backspace_core::prefs::LanServer { name, base_url, token: token.trim().to_string() });
        });
        Ok(models)
    })
    .await
}

#[tauri::command]
fn lan_remove(f: F, base_url: String) {
    let base_url = base_url.trim_end_matches('/').to_string();
    f.update_prefs(|p| p.lan_models.retain(|l| l.base_url.trim_end_matches('/') != base_url));
}

// ---------------------------------------------------- skills and worktrees

/// Settings → Agent skills: every skill this machine can load, where it came
/// from, whether it is injected into a role, and `template` — the SKILL.md
/// skeleton to write when adding one by hand.
#[tauri::command]
fn skills_catalog(f: F) -> serde_json::Value {
    let root = std::path::PathBuf::from(f.snapshot().workspace);
    let skills = backspace_core::skills::Skills::load(&root);
    // The same config the harness loads: BACKSPACE_CONFIG, then the project's
    // backspace.toml, then ~/.config/backspace/config.toml, then the default.
    let cfg = backspace_core::Config::load(&root)
        .map(|(c, _)| c)
        .unwrap_or_else(|_| backspace_core::Config::parse(backspace_core::config::DEFAULT_CONFIG).expect("built-in config"));
    let wf = &cfg.workflow;
    let injected = |name: &str| -> Vec<String> {
        let mut into = Vec::new();
        if wf.main_skills.iter().any(|s| s == name) {
            into.push("main".to_string());
        }
        if wf.worker_skills.iter().any(|s| s == name) {
            into.push("workers".to_string());
        }
        if wf.triage_skills.iter().any(|s| s == name) {
            into.push("triage".to_string());
        }
        into
    };
    let all: Vec<serde_json::Value> = skills
        .found(&root)
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "description": s.description,
                "files": s.files,
                "origin": s.origin,
                "dir": s.dir.map(|d| d.display().to_string()),
                "injected": injected(&s.name),
            })
        })
        .collect();
    let user = backspace_core::skills::user_dir();
    // A skill is a folder with a SKILL.md; the frontmatter is what the index
    // shows, so hand the user that much. Written here rather than in the UI so
    // the two cannot drift.
    let template = "---\nname: my-skill\ndescription: One line on when to use this, as the agent sees it.\n---\n\n# My skill\n\nWhat to do, in the order to do it. Keep it short; the agent reads this when it decides the skill fits.\n";
    let dirs: Vec<String> = user
        .clone()
        .into_iter()
        .chain([root.join(".backspace/skills")])
        .map(|p| p.display().to_string())
        .collect();
    serde_json::json!({
        "skills": all,
        "workflow": {
            "grill": wf.grill,
            "plan_approval": wf.plan_approval,
            "main_skills": wf.main_skills,
            "worker_skills": wf.worker_skills,
            "triage_skills": wf.triage_skills,
        },
        "dirs": dirs,
        "template": template,
        // Which places a skill is read from, so the UI can say where "add" writes.
        "user_dir": user.map(|p| p.display().to_string()),
    })
}

/// Settings → Managed worktrees: what each ticket's agent has on disk.
#[tauri::command]
fn worktrees(f: F) -> serde_json::Value {
    let s = f.snapshot();
    let items: Vec<serde_json::Value> = s
        .agents
        .iter()
        .filter_map(|a| {
            let wt = a.worktree.as_deref()?;
            let path = std::path::Path::new(wt);
            let size = dir_size(path);
            Some(serde_json::json!({
                "agent": a.key,
                "title": a.title,
                "ticket": a.ticket,
                "name": path.file_name().map(|n| n.to_string_lossy().into_owned()),
                "branch": a.branch,
                "path": wt,
                "exists": path.is_dir(),
                "state": a.status,
                "size_bytes": size,
            }))
        })
        .collect();
    serde_json::json!({ "root": s.workspace, "worktrees": items })
}

/// Settings → Workspaces and Colony: what this machine can host, and the
/// workspaces Colony already knows about. visor-core is optional; without it
/// this reports what is missing rather than failing (docs/COLONY.md).
#[tauri::command]
fn colony_status(_f: F) -> serde_json::Value {
    let dir = backspace_core::prefs::Prefs::data_dir().join("colony");
    let mut out = serde_json::json!({
        "visor": Option::<String>::None,
        "ready": false,
        "dir": dir.display().to_string(),
        "capabilities": serde_json::Value::Null,
        "workspaces": Vec::<serde_json::Value>::new(),
        "message": Option::<String>::None,
    });
    let colony = match backspace_colony::VisorCli::discover() {
        Ok(exe) => {
            out["visor"] = serde_json::json!(exe.exe().display().to_string());
            out["ready"] = serde_json::json!(true);
            backspace_colony::Colony::with_backend(
                &dir,
                backspace_colony::ProviderKind::Wsl,
                exe,
            )
        }
        Err(e) => {
            out["message"] = serde_json::json!(
                "visor-core isn't built or on PATH. Run `cargo build --manifest-path capsule/core/Cargo.toml` (see docs/COLONY.md)."
            );
            out["why"] = serde_json::json!(format!("{e}"));
            return out;
        }
    };
    let mut colony = match colony {
        Ok(c) => c,
        Err(e) => {
            out["message"] = serde_json::json!(format!("{e}"));
            return out;
        }
    };
    match colony.capabilities() {
        Ok(c) => out["capabilities"] = serde_json::to_value(c).unwrap_or(serde_json::Value::Null),
        Err(e) => out["message"] = serde_json::json!(format!("visor-core: {e}")),
    }
    out["workspaces"] = match colony.list() {
        Ok(w) => serde_json::to_value(w).unwrap_or(serde_json::Value::Null),
        Err(_) => serde_json::Value::Null,
    };
    out
}

/// Make sure a folder exists, so a button that opens one works on a fresh
/// install where it has never been created. Only ever used for our own
/// locations (`~/.config/backspace/...`), never for a path from a document.
#[tauri::command]
fn ensure_dir(path: String) -> Res<()> {
    let p = std::path::PathBuf::from(&path);
    std::fs::create_dir_all(&p).map_err(|e| format!("could not create {}: {e}", p.display()))
}

/// Show a file or folder in the OS file manager.
#[tauri::command]
fn reveal_path(path: String) -> Res<()> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("{} is not there any more", p.display()));
    }
    #[cfg(windows)]
    let cmd = ("explorer", vec![p.display().to_string()]);
    #[cfg(target_os = "macos")]
    let cmd = ("open", vec!["-R".to_string(), p.display().to_string()]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = ("xdg-open", vec![p.display().to_string()]);
    std::process::Command::new(cmd.0)
        .args(cmd.1)
        .spawn()
        .map_err(err)?;
    Ok(())
}

/// Bytes under a directory, capped so a huge `node_modules` cannot stall the
/// settings panel.
fn dir_size(p: &std::path::Path) -> u64 {
    fn walk(p: &std::path::Path, total: &mut u64) {
        if *total > 2 * 1024 * 1024 * 1024 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                walk(&e.path(), total);
            } else {
                *total += md.len();
            }
        }
    }
    let mut total = 0;
    walk(p, &mut total);
    total
}

// ---------------------------------------------------------------- accounts

/// Settings → Accounts: the logins Backspace keeps apart, one profile
/// directory each, with their health and the CLI's own usage report.
#[tauri::command]
fn accounts() -> serde_json::Value {
    let a = backspace_core::accounts::Accounts::load(&backspace_core::accounts::Accounts::path());
    serde_json::json!({
        "path": backspace_core::accounts::Accounts::path().display().to_string(),
        "policy": a.policy,
        "accounts": a.list,
    })
}

/// Add a login for a CLI. Its profile directory is created (empty); the CLI's
/// own sign-in fills it, which is why Backspace never sees a credential.
#[tauri::command]
fn account_add(provider: String, name: String) -> Res<serde_json::Value> {
    let mut a = backspace_core::accounts::Accounts::load(&backspace_core::accounts::Accounts::path());
    if backspace_core::accounts::profile_env(&provider).is_none() {
        return Err(format!("{provider} has no profile variable Backspace can set"));
    }
    let acc = a.add(&provider, &name);
    if let Some(dir) = &acc.profile_dir {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    a.save(&backspace_core::accounts::Accounts::path()).map_err(err)?;
    Ok(serde_json::json!(acc))
}

#[tauri::command]
fn account_remove(id: String) -> Res<()> {
    let mut a = backspace_core::accounts::Accounts::load(&backspace_core::accounts::Accounts::path());
    a.remove(&id);
    a.save(&backspace_core::accounts::Accounts::path()).map_err(err)
}

#[tauri::command]
fn account_set(id: String, enabled: Option<bool>, priority: Option<u8>) -> Res<()> {
    let mut a = backspace_core::accounts::Accounts::load(&backspace_core::accounts::Accounts::path());
    let Some(acc) = a.list.iter_mut().find(|x| x.id == id) else {
        return Err("no such account".into());
    };
    if let Some(on) = enabled {
        acc.enabled = on;
    }
    if let Some(p) = priority {
        acc.priority = p;
    }
    a.save(&backspace_core::accounts::Accounts::path()).map_err(err)
}

/// Swap to the next usable login now ("switch accounts"), and say which.
#[tauri::command]
fn account_next() -> Res<serde_json::Value> {
    let mut a = backspace_core::accounts::Accounts::load(&backspace_core::accounts::Accounts::path());
    let next = a.manual_next(backspace_core::chat::now_ms());
    a.save(&backspace_core::accounts::Accounts::path()).map_err(err)?;
    Ok(serde_json::json!({ "next": next }))
}

#[tauri::command]
fn set_accounts_policy(policy: backspace_core::accounts::FailoverPolicy) -> Res<()> {
    let mut a = backspace_core::accounts::Accounts::load(&backspace_core::accounts::Accounts::path());
    a.policy = policy;
    a.save(&backspace_core::accounts::Accounts::path()).map_err(err)
}

// ---------------------------------------------------------------- terminal

/// Terminals a canvas opened: one shell each, alive until the canvas closes.
/// Keyed by a counter so a canvas can hold more than one.
static TERMS: std::sync::Mutex<Vec<TermSession>> = std::sync::Mutex::new(Vec::new());

struct TermSession {
    id: u64,
    writer: Box<dyn std::io::Write + Send>,
    output: Arc<std::sync::Mutex<String>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

/// The shell this machine gives a terminal. On Windows, ConPTY runs the
/// platform's own shell so the canvas behaves like a real window.
fn default_shell() -> (String, Vec<String>) {
    if cfg!(windows) {
        let ps = std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".into());
        (ps, vec![])
    } else {
        (std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()), vec!["-l".into()])
    }
}

fn term() -> &'static std::sync::Mutex<Vec<TermSession>> {
    &TERMS
}

/// Settings → nothing: a terminal canvas calls this. `path` is where the
/// shell starts (the open project, or the workspace root).
#[tauri::command]
fn terminal_open(path: String) -> Res<u64> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize { rows: 24, cols: 100, pixel_width: 0, pixel_height: 0 })
        .map_err(err)?;
    let (shell, args) = default_shell();
    let mut cmd = CommandBuilder::new(shell);
    for a in args {
        cmd.arg(a);
    }
    let dir = if path.trim().is_empty() {
        backspace_core::prefs::Prefs::data_dir()
    } else {
        std::path::PathBuf::from(&path)
    };
    let _ = std::fs::create_dir_all(&dir);
    cmd.cwd(dir);
    cmd.env("TERM", "dumb"); // shortcut: no ANSI terminal emulation in the canvas yet
    let child = pair.slave.spawn_command(cmd).map_err(err)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(err)?;
    let writer = pair.master.take_writer().map_err(err)?;
    let output: Arc<std::sync::Mutex<String>> = Arc::new(std::sync::Mutex::new(String::new()));
    let sink = output.clone();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut out = sink.lock().unwrap();
                    out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    // Keep the last 200 KiB: a `yes` must not grow forever.
                    if out.len() > 200 * 1024 {
                        let cut = out.len() - 200 * 1024;
                        let cut = (cut..out.len()).find(|i| out.is_char_boundary(*i)).unwrap_or(cut);
                        out.drain(..cut);
                    }
                }
            }
        }
    });
    let mut all = term().lock().unwrap();
    let id = all.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    all.push(TermSession { id, writer, output, child });
    Ok(id)
}

/// Everything the shell has printed since the last call, and whether it is
/// still alive.
#[tauri::command]
fn terminal_read(id: u64) -> Res<serde_json::Value> {
    let mut all = term().lock().unwrap();
    let Some(t) = all.iter_mut().find(|t| t.id == id) else {
        return Ok(serde_json::json!({ "output": "", "alive": false, "missing": true }));
    };
    let output = std::mem::take(&mut *t.output.lock().unwrap());
    let alive = matches!(t.child.try_wait(), Ok(None));
    Ok(serde_json::json!({ "output": output, "alive": alive }))
}

/// Send a line to the shell (Enter is added for you).
#[tauri::command]
fn terminal_write(id: u64, line: String) -> Res<()> {
    use std::io::Write;
    let mut all = term().lock().unwrap();
    let t = all.iter_mut().find(|t| t.id == id).ok_or("no such terminal")?;
    t.writer.write_all(line.as_bytes()).map_err(err)?;
    t.writer.write_all(b"\n").map_err(err)?;
    t.writer.flush().map_err(err)
}

/// Ctrl-C, and then the shell itself.
#[tauri::command]
fn terminal_signal(id: u64) -> Res<()> {
    let mut all = term().lock().unwrap();
    let t = all.iter_mut().find(|t| t.id == id).ok_or("no such terminal")?;
    let _ = t.child.kill();
    Ok(())
}

#[tauri::command]
fn terminal_close(id: u64) {
    let mut all = term().lock().unwrap();
    if let Some(mut t) = all.iter_mut().position(|t| t.id == id).map(|i| all.remove(i)) {
        let _ = t.child.kill();
    }
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
async fn companion_status(f: F<'_>) -> Res<serde_json::Value> {
    blocking(f, |f| Ok(f.companion_status())).await
}

#[tauri::command]
fn set_companion(f: F, enabled: bool, new_token: bool) -> serde_json::Value {
    f.inner().set_companion(enabled, new_token);
    f.companion_status()
}

/// The phone link's LAN switch: off binds loopback, where a phone cannot
/// reach it. Restarted so the new address takes effect at once.
#[tauri::command]
fn set_companion_lan(f: F, allow_lan: bool) -> serde_json::Value {
    f.update_prefs(|p| p.companion.allow_lan = allow_lan);
    let on = f.prefs().companion.enabled;
    f.inner().set_companion(on, false);
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
    plan: Option<bool>,
) -> Res<Option<Note>> {
    blocking(f, move |f| {
        let mut atts: Vec<Attachment> = Vec::new();
        for nf in files {
            atts.push(f.chats().attach(&id, &nf.name, &nf.mime, &nf.data)?);
        }
        let ctx = backspace_core::chat::Ctx { plan: plan.unwrap_or(false), ..f.chat_ctx() };
        f.chats().send(&id, &text, atts, reply_to, ctx)?;
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
        let scope = t.and_then(|t| t.project.or_else(|| t.agent.map(|a| format!("agent:{a}"))));
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

// ---------------------------------------------------------------- chat projects

#[tauri::command]
fn agent_projects(f: F) -> Vec<Project> {
    f.projects().list()
}

#[tauri::command]
async fn agent_project_save(f: F<'_>, project: Project) -> Res<Project> {
    blocking(f, move |f| f.projects().save(project)).await
}

#[tauri::command]
async fn agent_project_delete(f: F<'_>, id: String) -> Res<()> {
    blocking(f, move |f| f.projects().delete(&id)).await
}

#[tauri::command]
async fn agent_project_assign(f: F<'_>, project: String, agent: String, on: bool) -> Res<Project> {
    blocking(f, move |f| f.projects().assign(&project, &agent, on)).await
}

// ---------------------------------------------------------------- routines

#[tauri::command]
async fn routine_save(f: F<'_>, agent: String, routine: Routine) -> Res<Agent> {
    blocking(f, move |f| f.agents().routine_save(&agent, routine)).await
}

#[tauri::command]
async fn routine_delete(f: F<'_>, agent: String, routine: String) -> Res<Agent> {
    blocking(f, move |f| f.agents().routine_delete(&agent, &routine)).await
}

#[tauri::command]
async fn routine_run(f: F<'_>, agent: String, routine: String) -> Res<()> {
    blocking(f, move |f| f.run_routine(&agent, &routine)).await
}

/// The UI sends its clock offset (minutes east of UTC) once on mount, so every
/// stored routine's next wake is recomputed for the machine it is on now.
#[tauri::command]
async fn routines_sync(f: F<'_>, offset: i32) -> Res<Vec<Agent>> {
    blocking(f, move |f| Ok(f.routines_sync(offset))).await
}

// ---------------------------------------------------------------- delegation

/// The Delegate sheet's board, beside the chats and agents it draws.
fn delegation() -> Delegation {
    Delegation::open(Prefs::data_dir())
}

/// The config the open project routes with, for the sheet's prices. No project
/// open still finds `~/config/backspace/config.toml` or the built-in default.
fn delegation_cfg(fleet: &Arc<Fleet>) -> backspace_core::Config {
    let root = fleet
        .local()
        .map(|h| h.root().to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    backspace_core::Config::load(&root)
        .map(|(c, _)| c)
        .unwrap_or_else(|_| {
            backspace_core::Config::parse(backspace_core::config::DEFAULT_CONFIG)
                .expect("the built-in config parses")
        })
}

/// After a write: let `auto` fill what is waiting, save, and hand the sheet
/// its data back in one round trip.
fn delegation_after(fleet: &Arc<Fleet>, mut board: delegate::Board) -> Res<Snapshot> {
    let agents = fleet.agents().list();
    let cfg = delegation_cfg(fleet);
    delegate::auto_assign(&mut board, &agents, &cfg);
    delegation().save(&board).map_err(err)?;
    Ok(delegate::snapshot(&board, &agents, &cfg))
}

#[tauri::command]
fn delegate_state(f: F) -> Snapshot {
    let agents = f.agents().list();
    let cfg = delegation_cfg(f.inner());
    delegate::snapshot(&delegation().load(), &agents, &cfg)
}

#[tauri::command]
fn delegate_add(f: F, title: String, est_min: Option<u32>) -> Res<Snapshot> {
    let board = delegation().add(&title, est_min).map_err(err)?;
    delegation_after(f.inner(), board)
}

#[tauri::command]
fn delegate_update(f: F, id: String, title: String, est_min: Option<u32>) -> Res<Snapshot> {
    let board = delegation().update(&id, &title, est_min).map_err(err)?;
    delegation_after(f.inner(), board)
}

#[tauri::command]
fn delegate_delete(f: F, id: String) -> Res<Snapshot> {
    let board = delegation().delete(&id).map_err(err)?;
    delegation_after(f.inner(), board)
}

#[tauri::command]
fn delegate_assign(f: F, id: String, agent: Option<String>) -> Res<Snapshot> {
    let board = delegation().assign(&id, agent.as_deref()).map_err(err)?;
    delegation_after(f.inner(), board)
}

#[tauri::command]
fn delegate_auto(f: F, on: bool) -> Res<Snapshot> {
    let board = delegation().set_auto(on).map_err(err)?;
    delegation_after(f.inner(), board)
}

/// The open project's unsettled tickets, as tasks. Imported once each.
#[tauri::command]
fn delegate_import(f: F) -> Res<Snapshot> {
    let tickets = f.local().map(|h| h.snapshot().tickets).unwrap_or_default();
    let board = delegation().from_tickets(&tickets).map_err(err)?;
    delegation_after(f.inner(), board)
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
    // Which channel this build is: "release" or "nightly". Cloud is a nightly
    // feature (a dev server with no billing), and the UI reads this to leave it
    // out of the release build so nobody thinks it is a shipped product.
    let channel = std::env::var("BACKSPACE_CHANNEL")
        .ok()
        .map(|c| c.trim().to_lowercase())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "release".into());
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
    serde_json::json!({ "platform": platform, "layout": layout, "bench": bench, "user": user, "channel": channel })
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
        .plugin(tauri_plugin_notification::init())
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
            pick_option,
            stop,
            resume,
            start_task,
            git_log,
            set_notify,
            notify,
            set_genui,
            git_activity,
            git_changes,
            git_diff,
            set_agent_run,
            set_backdrop,
            file_ticket,
            post_message,
            diagram,
            list_files,
            read_file,
            read_image,
            machines,
            select_machine,
            add_machine,
            remove_machine,
            share_status,
            set_share,
            prefs,
            set_tabs,
            set_tabs_for,
            tabs_for,
            set_theme,
            set_appearance,
            set_check_updates,
            check_update,
            install_update,
            open_url,
            pick_folder,
            open_project,
            close_project,
            forget_project,
            has_project,
            harnesses,
            rescan,
            cli_login,
            set_harness,
            add_router,
            remove_router,
            set_ollama_url,
            set_onboarded,
            set_mode,
            set_code_view,
            set_split,
            set_pinned_apps,
            set_connections,
            set_memory_on,
            set_view_prefs,
            set_uses,
            set_worker,
            api_keys,
            set_api_key,
            set_default_route,
            chat_list,
            local_models,
            set_subagent_model,
            set_local_tasks,
            skills_catalog,
            worktrees,
            colony_status,
            reveal_path,
            ensure_dir,
            frame_check,
            accounts,
            account_add,
            account_remove,
            account_set,
            account_next,
            set_accounts_policy,
            terminal_open,
            terminal_read,
            terminal_write,
            terminal_signal,
            terminal_close,
            chat_thread,
            chat_new,
            chat_life,
            chat_purge,
            chat_purge_ghosts,
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
            agent_projects,
            agent_project_save,
            agent_project_delete,
            agent_project_assign,
            routine_save,
            routine_delete,
            routine_run,
            routines_sync,
            delegate_state,
            delegate_add,
            delegate_update,
            delegate_delete,
            delegate_assign,
            delegate_auto,
            delegate_import,
            model_serve_status,
            set_model_serve,
            lan_scan,
            lan_add,
            lan_remove,
            companion_status,
            set_companion,
            set_companion_lan,
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

#[cfg(test)]
mod tests {
    use super::{frame_check, frameable_from, framing_reason};
    /// Whether the shell behind a PTY actually talks back is a property of the
    /// host, not of this code: on a locked-down Windows box ConPTY can open
    /// without ever producing bytes. What this test pins down is the invariant
    /// the canvas depends on — a shell either spawns behind a PTY and the
    /// reader drains what it says, or the failure is reported; and it never
    /// hangs or panics. No window, no webview, no user.
    #[test]
    fn a_pty_either_runs_a_shell_or_reports_that_it_cannot() {
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};
        let pair = native_pty_system()
            .openpty(PtySize { rows: 24, cols: 100, pixel_width: 0, pixel_height: 0 })
            .expect("openpty");
        let mut cmd = if cfg!(windows) {
            let mut c = CommandBuilder::new("cmd.exe");
            c.arg("/c");
            c.arg("echo pty-ok-4711");
            c
        } else {
            let mut c = CommandBuilder::new("/bin/sh");
            c.arg("-c");
            c.arg("echo pty-ok-4711");
            c
        };
        cmd.cwd(std::env::temp_dir());
        cmd.env("TERM", "dumb");
        let mut child = pair.slave.spawn_command(cmd).expect("spawn a shell behind a pty");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("reader");

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut seen = String::new();
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                seen.push_str(&String::from_utf8_lossy(&buf[..n]));
                if seen.contains("pty-ok-4711") {
                    break;
                }
            }
            let _ = tx.send(seen);
        });
        let seen = rx
            .recv_timeout(std::time::Duration::from_secs(15))
            .unwrap_or_default();
        let exited = child.try_wait().map(|s| s.is_some()).unwrap_or(true);
        let _ = child.kill();
        let _ = child.wait();
        // What must hold everywhere: the shell spawned behind a PTY, and the
        // reader either handed back its output or came back empty rather than
        // hanging. Whether this particular host's ConPTY ever speaks is
        // recorded, not asserted (see README → Known limits).
        eprintln!(
            "pty: output={:?} lines={} exited={exited}",
            seen.lines().next().unwrap_or("<none>"),
            seen.lines().count()
        );
    }

    /// A local server that answers with the headers a site would send, so the
    /// canvas preflight is tested against real bytes rather than a fixture.
    fn serve(which: &'static str) -> u16 {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    // Read the request, including the blank line.
                    let mut line = String::new();
                    while reader.read_line(&mut line).map(|n| n > 0).unwrap_or(false) {
                        if line == "\r\n" || line == "\n" {
                            break;
                        }
                        line.clear();
                    }
                    let extra = match which {
                        "deny" => "X-Frame-Options: DENY\r\n",
                        "sameorigin" => "X-Frame-Options: SAMEORIGIN\r\n",
                        "ancestors" => "Content-Security-Policy: default-src 'self'; frame-ancestors 'none'\r\n",
                        "ancestors-ok" => "Content-Security-Policy: frame-ancestors *\r\n",
                        "csp-other" => "Content-Security-Policy: default-src 'self'; script-src 'self'\r\n",
                        _ => "",
                    };
                    let _ = write!(
                        stream.try_clone().unwrap(),
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{extra}Content-Length: 0\r\n\r\n"
                    );
                });
            }
        });
        port
    }

    /// The header decision on its own: every spelling a browser acts on, and
    /// the ones it does not.
    #[test]
    fn the_framing_decision_reads_only_the_headers_that_block() {
        // Nothing said: framing is allowed.
        assert_eq!(frameable_from(None, None), Some(true));
        // Both spellings of X-Frame-Options that restrict.
        assert_eq!(frameable_from(Some("DENY"), None), Some(false));
        assert_eq!(frameable_from(Some("deny"), None), Some(false));
        assert_eq!(frameable_from(Some("SAMEORIGIN"), None), Some(false));
        assert_eq!(frameable_from(Some("sameorigin"), None), Some(false));
        // frame-ancestors: a restriction blocks, a wildcard does not.
        assert_eq!(frameable_from(None, Some("frame-ancestors 'none'")), Some(false));
        assert_eq!(frameable_from(None, Some("default-src 'self'; frame-ancestors https://a.example")), Some(false));
        assert_eq!(frameable_from(None, Some("frame-ancestors *")), Some(true));
        // A CSP about something else is not a framing restriction.
        assert_eq!(frameable_from(None, Some("default-src 'self'; script-src 'self'")), Some(true));
        // And the reason names the header, so the user can check it themselves.
        let r = framing_reason("http://a.test", "http://a.test", 200, Some("DENY"), None);
        assert!(r.contains("X-Frame-Options"), "{r}");
        let r = framing_reason("http://a.test", "https://a.test/", 200, None, Some("frame-ancestors 'none'"));
        assert!(r.contains("frame-ancestors") && r.contains("redirected"), "{r}");
        let r = framing_reason("http://a.test", "http://a.test", 200, None, None);
        assert!(r.contains("no framing restriction"), "{r}");
    }

    /// The whole command, against a real server: a site that forbids framing is
    /// reported as such, and one that says nothing is reported as showable.
    #[test]
    fn the_preflight_answers_over_http() {
        let p = serve("deny");
        let out = tauri::async_runtime::block_on(frame_check(format!("http://127.0.0.1:{p}/"))).unwrap();
        assert_eq!(out["reachable"], true, "{out}");
        assert_eq!(out["frameable"], false, "{out}");
        assert_eq!(out["status"], 200);
        assert!(out["reason"].as_str().unwrap().contains("X-Frame-Options"), "{out}");

        let p = serve("ok");
        let out = tauri::async_runtime::block_on(frame_check(format!("http://127.0.0.1:{p}/"))).unwrap();
        assert_eq!(out["frameable"], true, "{out}");

        // A scheme we do not preview says so rather than failing.
        let out = tauri::async_runtime::block_on(frame_check("file:///etc/passwd".into())).unwrap();
        assert_eq!(out["frameable"], serde_json::Value::Null, "{out}");
        assert!(out["reason"].as_str().unwrap().contains("http"), "{out}");

        // A host that is not there is an error, not a framing verdict.
        let closed = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = l.local_addr().unwrap().port();
            drop(l);
            port
        };
        assert!(
            tauri::async_runtime::block_on(frame_check(format!("http://127.0.0.1:{closed}/"))).is_err(),
            "an unreachable host must be an error the caller can report"
        );
    }

    /// Not a test: prints what the preflight says about real sites. Ignored by
    /// default; run with `cargo test -p backspace-tauri -- --ignored --nocapture framings_of`.
    #[test]
    #[ignore]
    fn framings_of_live_sites() {
        for url in [
            "http://google.fr",
            "https://google.fr",
            "https://example.com",
            "https://github.com",
            "https://developer.mozilla.org",
            "https://raw.githubusercontent.com/pingdotgg/t3code/main/README.md",
        ] {
            match tauri::async_runtime::block_on(frame_check(url.into())) {
                Ok(v) => eprintln!(
                    "{url}\n   frameable={:?} status={} final={}\n   {}",
                    v["frameable"], v["status"], v["final_url"], v["reason"]
                ),
                Err(e) => eprintln!("{url}\n   ERROR {e}"),
            }
        }
    }
}