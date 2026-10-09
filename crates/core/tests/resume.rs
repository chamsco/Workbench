//! A reply cut off by quitting comes back as "interrupted" and can be
//! resumed: on Claude Code's own session when the thread has one, else from
//! the conversation plus what was written so far.

use std::sync::Arc;
use std::time::{Duration, Instant};

use backspace_core::chat::{Chats, Ctx, Status};
use backspace_core::prefs::Prefs;

fn fake_claude(dir: &std::path::Path) {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = r#"#!/bin/sh
input=$(cat)
printf '%s' "$*" > "$(dirname "$0")/args"
printf '%s' "$input" > "$(dirname "$0")/input"
say() { printf '{"type":"result","result":"%s","session_id":"s1","is_error":false}\n' "$1"; }
case "$input" in
  *"Carry on from exactly where you stopped"*) say "Second half." ;;
  *) say "Hello." ;;
esac
"#;
    let p = bin.join("claude");
    std::fs::write(&p, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::env::set_var("BACKSPACE_CLI_PATH", &bin);
}

/// A thread as it was on disk when the app closed mid-reply.
fn cut_off_thread(dir: &std::path::Path, id: &str, session: Option<&str>) {
    let t = serde_json::json!({
        "id": id, "title": "Plan", "created": 1, "updated": 1,
        "route": {"kind": "cli", "provider": "claude", "model": null},
        "messages": [
            {"id": "u1", "role": "user", "text": "Write the plan", "at": 1, "status": "done", "reactions": [], "attachments": []},
            {"id": "a1", "role": "assistant", "text": "First half.", "at": 2, "status": "streaming", "reactions": [], "attachments": []}
        ],
        "cli_session": session,
    });
    std::fs::create_dir_all(dir.join("chats/threads")).unwrap();
    std::fs::write(dir.join(format!("chats/threads/{id}.json")), t.to_string()).unwrap();
}

fn wait_idle(chats: &Arc<Chats>, id: &str) {
    let start = Instant::now();
    while chats.is_busy(id) {
        assert!(start.elapsed() < Duration::from_secs(30), "never finished");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[cfg(unix)]
fn interrupted_replies_resume() {
    let dir = std::env::temp_dir().join(format!("bs-resume-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    fake_claude(&dir);
    cut_off_thread(&dir, "t1", Some("s1"));
    cut_off_thread(&dir, "t2", None);
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let (tx, _rx) = async_channel::bounded(16);
    let chats = Chats::open(dir.join("chats"), rt.handle().clone(), tx);
    let ctx = Ctx { prefs: Prefs::default(), memory: None, agents: None, plan: false };

    let t = chats.thread("t1").unwrap();
    let a = t.messages.last().unwrap();
    assert!(a.interrupted && a.status == Status::Stopped);

    // With a session: only "carry on", on that session.
    chats.resume("t1", ctx.clone()).unwrap();
    wait_idle(&chats, "t1");
    let a = chats.thread("t1").unwrap().messages.last().unwrap().clone();
    assert_eq!(a.text, "First half.\n\nSecond half.");
    assert!(!a.interrupted && a.status == Status::Done);
    let args = std::fs::read_to_string(dir.join("bin/args")).unwrap();
    assert!(args.contains("--resume s1"), "{args}");
    let input = std::fs::read_to_string(dir.join("bin/input")).unwrap();
    assert!(!input.contains("Write the plan"), "the session already has the conversation");

    // Without one: the conversation and the partial answer, then "carry on".
    chats.resume("t2", ctx.clone()).unwrap();
    wait_idle(&chats, "t2");
    let input = std::fs::read_to_string(dir.join("bin/input")).unwrap();
    assert!(input.contains("User: Write the plan") && input.contains("Assistant: First half."), "{input}");
    assert_eq!(chats.thread("t2").unwrap().messages.last().unwrap().text, "First half.\n\nSecond half.");

    // Nothing to resume once it's done.
    assert!(chats.resume("t2", ctx).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
