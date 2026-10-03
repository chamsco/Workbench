//! Tauri shell for the Backspace harness. The UI in `ui/` draws; this file
//! exposes the machines (`Fleet`: this harness plus followed remotes) to it:
//! commands for reads and actions, and a "state" event whenever anything
//! changes.

use std::path::PathBuf;
use std::sync::Arc;

use backspace_core::diagram::{self, Diagram};
use backspace_core::files::FileEntry;
use backspace_core::fleet::{Fleet, MachineInfo};
use backspace_core::prefs::{Prefs, TabSpec};
use backspace_core::update::UpdateInfo;
use backspace_core::{Harness, ProjectState};
use tauri::{Emitter, Manager, State};

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
    let ws = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    // WebKitGTK's DMA-BUF renderer draws nothing on X servers without DRI3
    // (Xvfb, some VMs); the fallback costs nothing elsewhere.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    let harness = Harness::open(ws)?;
    // Scripted runs (bench/) hand the goal over without typing it.
    if let Ok(goal) = std::env::var("BACKSPACE_GOAL") {
        harness.send(goal);
    }
    let fleet = Fleet::new(harness, Prefs::load())?;
    let changes = fleet.changes();

    tauri::Builder::default()
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
