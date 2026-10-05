//! Tauri shell for Backspace. The UI in `ui/` draws; this file exposes the
//! core to it: chats, the project open on this machine and the machines
//! followed (`Fleet`), the CLI/model scan, Cloud plans. Commands for reads
//! and actions, and a "state" event whenever anything changes.

use std::path::PathBuf;
use std::sync::Arc;

use backspace_core::chat::{Attachment, Route, Thread, ThreadInfo};
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
fn set_mode(f: F, mode: String) {
    f.update_prefs(|p| p.mode = mode);
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
fn chat_new(f: F, route: Route) -> Thread {
    f.chats().create(route)
}

#[tauri::command]
async fn chat_send(
    f: F<'_>,
    id: String,
    text: String,
    files: Vec<NewFile>,
    reply_to: Option<String>,
) -> Res<()> {
    blocking(f, move |f| {
        let mut atts: Vec<Attachment> = Vec::new();
        for nf in files {
            atts.push(f.chats().attach(&id, &nf.name, &nf.mime, &nf.data)?);
        }
        f.chats().send(&id, &text, atts, reply_to, f.chat_ctx())
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
    serde_json::json!({ "platform": platform, "layout": layout, "bench": bench })
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
    // A folder on the command line opens it; otherwise the app starts with
    // no project (never the launch folder, which is `/` from the Dock).
    let ws = std::env::args().nth(1).map(PathBuf::from);
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

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(fleet)
        .invoke_handler(tauri::generate_handler![
            snapshot,
            send,
            approve,
            reject,
            file_ticket,
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
            set_default_route,
            chat_list,
            chat_thread,
            chat_new,
            chat_send,
            chat_stop,
            chat_retry,
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
