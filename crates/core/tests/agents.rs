//! Agents and groups end to end, with a fake Claude Code on PATH: each
//! member reads the transcript and takes its turn; one that has nothing to
//! add says PASS and leaves no message; naming another member (@Name) hands
//! it a turn; a 1:1 agent thread runs in the agent's own folder with its
//! brief.

use std::sync::Arc;
use std::time::{Duration, Instant};

use backspace_core::agents::{Agent, Agents, Avatar, Computer};
use backspace_core::chat::{Chats, Ctx, Route, RouteKind, Scope, Status};
use backspace_core::prefs::Prefs;

fn fake_claude(dir: &std::path::Path) {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    // Answers by who it is asked to be; records its cwd and brief.
    let script = r#"#!/bin/sh
input=$(cat)
args="$*"
pwd > "$(dirname "$0")/last_cwd"
printf '%s' "$BACKSPACE_MEMORY" > "$(dirname "$0")/last_mem"
printf '%s' "$args" > "$(dirname "$0")/last_args"
say() { printf '{"type":"result","result":"%s","session_id":"s1","total_cost_usd":0.001,"is_error":false}\n' "$1"; }
case "$input" in
  *"Write Kira's next message"*) say "Plan: ship Thursday. @Andre please draft the post." ;;
  *"Write Andre's next message"*) say "Draft: Almanac 2.0 ships Thursday." ;;
  *"Write Mara's next message"*) say "PASS" ;;
  *) say "Hi, I am here." ;;
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

fn agent(name: &str, job: &str) -> Agent {
    Agent {
        id: String::new(),
        name: name.into(),
        job: job.into(),
        avatar: Avatar::default(),
        route: Route { kind: RouteKind::Cli, provider: "claude".into(), model: None },
        shared: vec![],
        computer: Computer::default(),
        memory: true,
        created: 0,
        updated: 0,
    }
}

fn wait_idle(chats: &Arc<Chats>, id: &str) {
    let start = Instant::now();
    loop {
        let busy = chats.list().iter().any(|t| t.id == id && t.busy);
        if !busy {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(30), "reply never finished");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[cfg(unix)]
fn groups_take_turns_and_agents_work_in_their_folder() {
    let dir = std::env::temp_dir().join(format!("bs-agents-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    fake_claude(&dir);
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let (tx, _rx) = async_channel::bounded(16);
    let chats = Chats::open(dir.join("chats"), rt.handle().clone(), tx);
    let agents = Arc::new(Agents::open(dir.join("agents")));
    let kira = agents.save(agent("Kira", "Plans the launch.")).unwrap();
    std::thread::sleep(Duration::from_millis(3));
    let andre = agents.save(agent("Andre", "Writes the copy.")).unwrap();
    std::thread::sleep(Duration::from_millis(3));
    let mara = agents.save(agent("Mara", "Makes the film.")).unwrap();
    let memory = Arc::new(backspace_core::memory::Memory::open(dir.join("data")));
    let ctx = Ctx { prefs: Prefs::default(), memory: Some(memory), agents: Some(agents.clone()) };

    // A group: Kira, Andre and Mara, in that order.
    let g = chats.create_in(
        kira.route.clone(),
        Scope {
            members: vec![kira.id.clone(), andre.id.clone(), mara.id.clone()],
            goal: Some("Ship Almanac 2.0".into()),
            title: Some("Launch week".into()),
            ..Default::default()
        },
    );
    assert_eq!(g.title, "Launch week");
    chats.send(&g.id, "Thursday is the day. What's the plan?", vec![], None, ctx.clone()).unwrap();
    wait_idle(&chats, &g.id);
    let t = chats.thread(&g.id).unwrap();
    let said: Vec<(String, String)> = t
        .messages
        .iter()
        .skip(1)
        .map(|m| (m.author_name.clone().unwrap_or_default(), m.text.clone()))
        .collect();
    // Kira, Andre (in order, Andre also named by Kira but not twice), Mara passed.
    assert_eq!(said.len(), 2, "{said:?}");
    assert_eq!(said[0].0, "Kira");
    assert!(said[0].1.contains("@Andre"));
    assert_eq!(said[1].0, "Andre");
    assert!(t.messages.iter().all(|m| m.status == Status::Done));
    assert_eq!(t.title, "Launch week", "a group keeps its name");
    // Every reply has a trace naming who wrote it.
    let tr = chats.trace(t.messages[1].trace.as_deref().expect("a trace")).expect("saved");
    assert_eq!(tr.spans[0].name, "reply · Kira");
    assert_eq!(tr.spans[0].attrs["gen_ai.agent.name"], "Kira");

    // Naming one member gives only that member the turn.
    chats.send(&g.id, "@Andre shorter please", vec![], None, ctx.clone()).unwrap();
    wait_idle(&chats, &g.id);
    let t = chats.thread(&g.id).unwrap();
    let last = t.messages.last().unwrap();
    assert_eq!(last.author_name.as_deref(), Some("Andre"));
    assert_eq!(t.messages.len(), 5);

    // A 1:1 thread with Kira runs in Kira's folder with her brief.
    let one = chats.create_in(kira.route.clone(), Scope { agent: Some(kira.id.clone()), ..Default::default() });
    chats.send(&one.id, "hello", vec![], None, ctx.clone()).unwrap();
    wait_idle(&chats, &one.id);
    let t = chats.thread(&one.id).unwrap();
    assert_eq!(t.messages.last().unwrap().author_name.as_deref(), Some("Kira"));
    let cwd = std::fs::read_to_string(dir.join("bin/last_cwd")).unwrap();
    assert_eq!(
        std::path::Path::new(cwd.trim()).canonicalize().unwrap(),
        agents.workspace(&kira.id).canonicalize().unwrap()
    );
    let args = std::fs::read_to_string(dir.join("bin/last_args")).unwrap();
    assert!(args.contains("You are Kira") && args.contains("Plans the launch."), "{args}");
    assert!(args.contains("acceptEdits"), "an agent may edit its own folder");
    // It gets the memory tools, scoped to its own notes.
    assert!(args.contains("--mcp-config") && args.contains("mcp__backspace"), "{args}");
    let mem = std::fs::read_to_string(dir.join("bin/last_mem")).unwrap();
    assert_eq!(mem, format!("agent:{}", kira.id));
    let _ = std::fs::remove_dir_all(&dir);
}
