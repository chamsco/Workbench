//! Plan mode end to end with a fake model and the real gate: the app writes
//! SPEC.md from the reply, the rules refuse it twice (an open question, then
//! an acceptance line nobody can check), the refusal reaches the next turn,
//! and the third spec opens the gate and moves the project to Plan.

// Unix-only: the fake `claude` is a shell script. Imports are gated with it so
// Windows doesn't warn about them.
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
use backspace_core::chat::{Chats, Ctx, Route, RouteKind, Scope};
#[cfg(unix)]
use backspace_core::phase::{Phase, PhaseState};
#[cfg(unix)]
use backspace_core::prefs::Prefs;

#[cfg(unix)]
const SPEC: &str = "# Dark mode\n\n## Problem\n\nToo bright at night.\n\n## Solution\n\nA theme switch.\n\n\
    ## User stories\n\n1. As a night owl, I want a dark theme, so that my eyes don't hurt.\n\n\
    ## Decisions\n\n- Follows the OS until picked.\n\n\
    ## Acceptance\n\n- `cargo test theme` passes\n- Click Settings, then Dark; the window turns dark.\n\n\
    ## Out of scope\n\n- Per-thread themes.\n\n## Open questions\n\nNone\n";

/// A fake `claude` that answers with reply-1, reply-2, … in turn, keeping
/// what it was asked.
#[cfg(unix)]
fn fake_claude(dir: &std::path::Path, replies: &[String]) {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for (i, r) in replies.iter().enumerate() {
        let line = serde_json::json!({"type": "result", "result": r, "session_id": "s1", "is_error": false});
        std::fs::write(bin.join(format!("reply-{}", i + 1)), format!("{line}\n")).unwrap();
    }
    let script = r#"#!/bin/sh
d=$(dirname "$0")
n=$(( $(cat "$d/n" 2>/dev/null || echo 0) + 1 ))
echo $n > "$d/n"
cat > "$d/input-$n"
printf '%s' "$*" > "$d/args-$n"
cat "$d/reply-$n"
"#;
    let p = bin.join("claude");
    std::fs::write(&p, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("BACKSPACE_CLI_PATH", &bin);
}

#[test]
#[cfg(unix)]
fn the_gate_refuses_twice_then_opens() {
    let dir = std::env::temp_dir().join(format!("bs-spec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let project = dir.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let open_question = SPEC.replace("None\n", "- Which shade of grey?\n");
    let vague = SPEC.replace("- Click Settings, then Dark; the window turns dark.", "- It looks nice.");
    let block = |s: &str| format!("Here is the spec.\n\n```spec\n{s}```\n");
    fake_claude(&dir, &[block(&open_question), block(&vague), block(SPEC)]);

    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let (tx, _rx) = async_channel::bounded(64);
    let chats = Chats::open(dir.join("chats"), rt.handle().clone(), tx);
    let route = Route { kind: RouteKind::Cli, provider: "claude".into(), model: None };
    let t = chats.create_in(route, Scope { project: Some(project.to_string_lossy().into()), ..Default::default() });
    let ctx = Ctx { prefs: Prefs::default(), memory: None, agents: None, plan: true };

    let mut verdicts = vec![];
    for say in ["Add dark mode", "Plain grey", "Fine"] {
        chats.send(&t.id, say, vec![], None, ctx.clone()).unwrap();
        let start = Instant::now();
        while chats.is_busy(&t.id) {
            assert!(start.elapsed() < Duration::from_secs(30), "never finished");
            std::thread::sleep(Duration::from_millis(50));
        }
        // `is_busy` goes false when the reply task drops its entry, a few lines before it
        // files the spec and attaches the verdict, so the verdict is waited for on its own.
        let verdict = loop {
            let last = chats.thread(&t.id).unwrap().messages.last().unwrap().clone();
            if let Some(v) = last.spec {
                break v;
            }
            assert!(start.elapsed() < Duration::from_secs(30), "a Plan reply with a spec gets a verdict: {:?}", last.status);
            std::thread::sleep(Duration::from_millis(20));
        };
        verdicts.push(verdict);
    }

    assert!(!verdicts[0].passed && verdicts[0].gaps[0].starts_with("Open questions: 1 still open"), "{:?}", verdicts[0]);
    assert!(!verdicts[1].passed && verdicts[1].gaps[0].starts_with("Acceptance 2 has no command"), "{:?}", verdicts[1]);
    assert!(verdicts[2].passed, "{:?}", verdicts[2]);
    assert_eq!(std::fs::read_to_string(project.join("SPEC.md")).unwrap(), SPEC);

    let bin = dir.join("bin");
    let asked = |n: u32| {
        std::fs::read_to_string(bin.join(format!("args-{n}"))).unwrap() + &std::fs::read_to_string(bin.join(format!("input-{n}"))).unwrap()
    };
    assert!(asked(1).contains("design tree"), "Plan mode carries the grilling skill");
    assert!(asked(2).contains("The gate refused the last spec") && asked(2).contains("Open questions: 1 still open"));
    assert!(asked(3).contains("Acceptance 2 has no command"));
    assert!(asked(2).contains("--permission-mode plan"), "Plan mode stays read-only: {}", asked(2));

    let phase = PhaseState::load(&project);
    assert_eq!(phase.phase, Phase::Plan);
    assert_eq!(phase.gates.iter().map(|g| g.passed).collect::<Vec<_>>(), vec![false, false, true]);
    let _ = std::fs::remove_dir_all(&dir);
}
