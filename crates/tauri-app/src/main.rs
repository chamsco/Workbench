//! Tauri shell for the Backspace harness. The UI in `ui/` is the
//! design/workbench.html replica; this file only exposes the harness to it:
//! commands for reads and actions, and a "state" event whenever it changes.

use std::path::{Path, PathBuf};

use backspace_core::diagram::{self, Diagram};
use backspace_core::{Harness, ProjectState};
use tauri::{Emitter, Manager, State};

type Res<T> = Result<T, String>;

#[tauri::command]
fn snapshot(h: State<Harness>) -> ProjectState {
    h.snapshot()
}

#[tauri::command]
fn send(h: State<Harness>, text: String) {
    h.send(text);
}

#[tauri::command]
fn approve(h: State<Harness>, id: usize) {
    h.approve(id);
}

#[tauri::command]
fn reject(h: State<Harness>, id: usize, feedback: String) {
    h.reject(id, feedback);
}

#[tauri::command]
fn file_ticket(h: State<Harness>, title: String, body: String) -> Res<String> {
    h.file_ticket(&title, &body).map_err(|e| e.to_string())
}

#[tauri::command]
fn diagram(h: State<Harness>, id: usize) -> Res<Diagram> {
    let s = h.snapshot();
    let ap = s.approvals.get(id).ok_or("no such approval")?;
    Ok(diagram::for_approval(&s, ap))
}

/// (depth, name, path, is_dir) rows of an agent's worktree.
#[tauri::command]
fn list_files(h: State<Harness>, agent: usize) -> Vec<(usize, String, String, bool)> {
    let s = h.snapshot();
    let root = s
        .agents
        .get(agent)
        .and_then(|a| a.worktree.clone())
        .unwrap_or(s.workspace);
    let mut out = vec![];
    scan(&root, 0, &mut out);
    out
}

/// Text of a file inside the project (worktrees live under it too).
#[tauri::command]
fn read_file(h: State<Harness>, path: String) -> Res<String> {
    let root = h.snapshot().workspace;
    let p = PathBuf::from(&path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !p.starts_with(&root) {
        return Err("outside the project".into());
    }
    let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
    let cut = &bytes[..bytes.len().min(256 * 1024)];
    Ok(String::from_utf8_lossy(cut).into_owned())
}

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
    serde_json::json!({ "platform": platform, "layout": layout })
}

/// The UI's first frame with data is on screen; bench/ times launch to this.
#[tauri::command]
fn ready() {
    if let Ok(path) = std::env::var("BACKSPACE_READY_FILE") {
        let _ = std::fs::write(path, now_ms().to_string());
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

/// Depth-first walk, skipping build output and VCS internals.
fn scan(dir: &Path, depth: usize, out: &mut Vec<(usize, String, String, bool)>) {
    if depth > 4 || out.len() > 2000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = rd.flatten().collect();
    items.sort_by_key(|e| (!e.path().is_dir(), e.file_name()));
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if matches!(
            name.as_str(),
            ".git" | "target" | "node_modules" | ".backspace" | "dist" | ".astro"
        ) {
            continue;
        }
        let path = e.path();
        let dir = path.is_dir();
        out.push((depth, name, path.to_string_lossy().into_owned(), dir));
        if dir {
            scan(&path, depth + 1, out);
        }
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
    let changes = harness.changes();

    tauri::Builder::default()
        .manage(harness)
        .invoke_handler(tauri::generate_handler![
            snapshot,
            send,
            approve,
            reject,
            file_ticket,
            diagram,
            list_files,
            read_file,
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
            // The harness coalesces changes; the UI coalesces again per frame.
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
