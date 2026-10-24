//! Agents talking to each other, whatever runs them.
//!
//! Each project has a message board. Agents on Backspace's own loop get
//! `send_message` / `read_messages` / `list_agents` tools, and anything
//! addressed to them is added to their next turn. Agents running in a
//! coding CLI (Claude Code, Codex, Cursor, Grok, OpenCode) reach the same
//! board from their shell:
//!
//!   backspace msg agents
//!   backspace msg send <agent|main|all> "text"
//!   backspace msg read
//!
//! (any Backspace binary answers `msg`; the environment it was started
//! with says which project and which agent it is). Claude Code also gets
//! it as MCP tools through `backspace mcp` (mcp.rs).
//!
//! The bridge behind it is a token-protected HTTP listener on 127.0.0.1,
//! one per open project, started with the harness.

use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::project::ProjectState;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BoardMsg {
    pub id: usize,
    /// Sender's agent key ("main", a ticket key) or "you" for the human.
    pub from: String,
    /// An agent key, or "all".
    pub to: String,
    pub text: String,
    pub at: u64,
    /// The message this one answers, if any (a threaded reply).
    #[serde(default)]
    pub reply_to: Option<usize>,
    /// Which conversation this belongs to (see [`thread_of`]). Stored so a
    /// thread survives its parent message being pruned, and so the canvas can
    /// group without recomputing the whole board.
    #[serde(default)]
    pub thread: Option<String>,
}

/// The conversation a message belongs to.
///
/// A thread is named after the two parties, whichever way the message went
/// (`"api|main"`), so everything a pair says to each other lands in one
/// conversation. A broadcast gets its own thread, named after its sender, so
/// `all` does not glue unrelated exchanges together. A reply joins its
/// parent's thread.
pub fn thread_of(from: &str, to: &str) -> String {
    if to != "all" && to != from {
        pair(from, to)
    } else {
        format!("{from}~all")
    }
}

fn pair(a: &str, b: &str) -> String {
    if a <= b {
        format!("{a}|{b}")
    } else {
        format!("{b}|{a}")
    }
}

pub const ENV_URL: &str = "BACKSPACE_BRIDGE";
pub const ENV_TOKEN: &str = "BACKSPACE_BRIDGE_TOKEN";
pub const ENV_AGENT: &str = "BACKSPACE_AGENT";

/// Check the recipient and append. Returns the message.
pub fn post(s: &mut ProjectState, from: &str, to: &str, text: &str) -> Result<BoardMsg> {
    reply(s, from, to, text, None)
}

/// Post, optionally as the answer to an earlier message: the reply joins that
/// message's conversation and names it as the parent.
pub fn reply(
    s: &mut ProjectState,
    from: &str,
    to: &str,
    text: &str,
    reply_to: Option<usize>,
) -> Result<BoardMsg> {
    let to = to.trim();
    let text = text.trim();
    if text.is_empty() {
        bail!("empty message");
    }
    if text.len() > 8000 {
        bail!("messages are limited to 8000 characters");
    }
    // An agent key, or a ticket key meaning its newest agent.
    let to = if to == "all" || s.agents.iter().any(|a| a.key == to) {
        to.to_string()
    } else if let Some(a) = s
        .agents
        .iter()
        .rev()
        .find(|a| a.ticket.as_deref() == Some(to))
    {
        a.key.clone()
    } else {
        let keys: Vec<&str> = s.agents.iter().map(|a| a.key.as_str()).collect();
        bail!("no agent `{to}`; agents are: {}, or `all`", keys.join(", "));
    };
    // A reply to something we still have joins its conversation; otherwise the
    // pair (or the sender, for a broadcast) names it.
    let parent = reply_to.and_then(|id| s.board.iter().find(|m| m.id == id));
    if let Some(id) = reply_to {
        if parent.is_none() {
            bail!("no message {id} on this board");
        }
    }
    let thread = match parent {
        Some(p) => p.thread.clone().unwrap_or_else(|| thread_of(&p.from, &p.to)),
        None => thread_of(from, &to),
    };
    let m = BoardMsg {
        id: s.board.len(),
        from: from.to_string(),
        to: to.to_string(),
        text: text.to_string(),
        at: crate::chat::now_ms(),
        reply_to,
        thread: Some(thread),
    };
    s.board.push(m.clone());
    Ok(m)
}

/// Messages for `me` (to it or to all, not from it) after `cursor`.
pub fn unread<'a>(s: &'a ProjectState, me: &str, cursor: usize) -> Vec<&'a BoardMsg> {
    s.board
        .iter()
        .skip(cursor)
        .filter(|m| m.from != me && (m.to == me || m.to == "all"))
        .collect()
}

/// One conversation: its thread id, the two sides (a broadcast has one
/// participant), every message in order, and when it last moved.
pub struct Thread<'a> {
    pub id: String,
    pub sides: Vec<String>,
    pub messages: Vec<&'a BoardMsg>,
    pub last: u64,
}

/// Every conversation on the board, most recently active first.
pub fn conversations(s: &ProjectState) -> Vec<Thread<'_>> {
    let mut out: Vec<Thread> = Vec::new();
    for m in &s.board {
        let id = m
            .thread
            .clone()
            .unwrap_or_else(|| thread_of(&m.from, &m.to));
        match out.iter_mut().find(|t| t.id == id) {
            Some(t) => {
                for side in [&m.from, &m.to] {
                    if side != "all" && !t.sides.iter().any(|x| x == side) {
                        t.sides.push(side.clone());
                    }
                }
                t.last = t.last.max(m.at);
                t.messages.push(m);
            }
            None => {
                let mut sides: Vec<String> = Vec::new();
                for side in [&m.from, &m.to] {
                    if side != "all" && !sides.iter().any(|x| x == side) {
                        sides.push(side.clone());
                    }
                }
                sides.sort();
                out.push(Thread { id, sides, messages: vec![m], last: m.at });
            }
        }
    }
    out.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| a.id.cmp(&b.id)));
    out
}

/// The conversation between exactly these two keys, or an empty one.
pub fn between<'a>(s: &'a ProjectState, a: &str, b: &str) -> Vec<&'a BoardMsg> {
    let want = pair(a, b);
    s.board
        .iter()
        .filter(|m| {
            m.thread.clone().unwrap_or_else(|| thread_of(&m.from, &m.to)) == want
        })
        .collect()
}


pub fn format(msgs: &[&BoardMsg]) -> String {
    if msgs.is_empty() {
        return "No new messages.".into();
    }
    msgs.iter()
        .map(|m| {
            let to = if m.to == "all" { " (to everyone)" } else { "" };
            let re = m.reply_to.map(|id| format!(" [answer to #{id}]")).unwrap_or_default();
            format!("[#{} {}]{to}{re}: {}", m.id, m.from, m.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn agents_line(s: &ProjectState) -> String {
    s.agents
        .iter()
        .map(|a| {
            let on = a
                .decision
                .as_ref()
                .map(|d| d.model.clone())
                .unwrap_or_else(|| "not started".into());
            format!("{} — {} ({:?}, {on})", a.key, a.title, a.status)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Per-agent read positions, kept by the harness.
#[derive(Default)]
pub struct Cursors(HashMap<String, usize>);

impl Cursors {
    pub fn take(&mut self, s: &ProjectState, me: &str) -> String {
        let c = self.0.get(me).copied().unwrap_or(0);
        let out = format(&unread(s, me, c));
        self.0.insert(me.to_string(), s.board.len());
        out
    }
    /// New messages as owned values, advancing the cursor.
    pub fn drain(&mut self, s: &ProjectState, me: &str) -> Vec<BoardMsg> {
        let c = self.0.get(me).copied().unwrap_or(0);
        let out = unread(s, me, c).into_iter().cloned().collect();
        self.0.insert(me.to_string(), s.board.len());
        out
    }
}

// ---------------------------------------------------------------- client

pub(crate) struct Bridge {
    url: String,
    token: String,
    me: String,
}

pub(crate) fn bridge() -> Result<Bridge> {
    let get = |k: &str| {
        std::env::var(k)
            .map_err(|_| anyhow!("{k} is not set: run this from an agent Backspace started"))
    };
    Ok(Bridge {
        url: get(ENV_URL)?,
        token: get(ENV_TOKEN)?,
        me: get(ENV_AGENT)?,
    })
}

fn call(b: &Bridge, path: &str, body: Value) -> Result<Value> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let r = reqwest::Client::builder()
            .no_proxy()
            .build()?
            .post(format!("{}{path}", b.url))
            .bearer_auth(&b.token)
            .json(&body)
            .send()
            .await?;
        let status = r.status();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            bail!("{}", v["error"].as_str().unwrap_or("bridge error"));
        }
        Ok(v)
    })
}

pub(crate) fn do_send(b: &Bridge, to: &str, text: &str, reply_to: Option<u64>) -> Result<String> {
    call(
        b,
        "/v1/board/post",
        json!({"from": b.me, "to": to, "text": text, "reply_to": reply_to}),
    )?;
    Ok(format!("sent to {to}"))
}
pub(crate) fn do_read(b: &Bridge) -> Result<String> {
    Ok(call(b, "/v1/board/read", json!({"me": b.me}))?["text"]
        .as_str()
        .unwrap_or("")
        .to_string())
}
/// Everything `me` and one other agent have said to each other.
pub(crate) fn do_conversation(b: &Bridge, with: &str) -> Result<String> {
    Ok(call(b, "/v1/board/conversation", json!({"me": b.me, "with": with}))?["text"]
        .as_str()
        .unwrap_or("")
        .to_string())
}
/// The planner's tools, for a main agent in a CLI: `path` is
/// `/v1/plan/create` or `/v1/plan/dispatch`.
pub(crate) fn do_plan(b: &Bridge, path: &str, mut body: Value) -> Result<String> {
    body["me"] = json!(b.me);
    Ok(call(b, path, body)?["text"].as_str().unwrap_or("").to_string())
}
/// Open or close a canvas in the user's window (`op`: "open" or "close").
pub(crate) fn do_canvas(b: &Bridge, op: &str, mut body: Value) -> Result<String> {
    body["me"] = json!(b.me);
    body["op"] = json!(op);
    Ok(call(b, "/v1/canvas", body)?["text"].as_str().unwrap_or("").to_string())
}
/// Show the user visual options and wait for their pick (options.rs). The
/// bridge holds the call open until they choose, so the CLI gets the answer.
pub(crate) fn do_options(b: &Bridge, mut body: Value) -> Result<String> {
    body["me"] = json!(b.me);
    Ok(call(b, "/v1/options", body)?["text"].as_str().unwrap_or("").to_string())
}
pub(crate) fn is_planner(b: &Bridge) -> bool {
    b.me == "main"
}
pub(crate) fn do_agents(b: &Bridge) -> Result<String> {
    Ok(call(b, "/v1/board/agents", json!({}))?["text"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// `leave_note`: one line worth remembering, from a worker in a CLI.
pub(crate) fn do_note(b: &Bridge, text: &str) -> Result<String> {
    Ok(call(b, "/v1/note", json!({ "me": b.me, "text": text }))?["text"].as_str().unwrap_or("").to_string())
}

/// `msg ...`, `tile ...` and `mcp` for any Backspace binary. None when `args` (after
/// the program name) are not one of these, so the binary carries on.
pub fn client_main(args: &[String]) -> Option<i32> {
    match args.first().map(String::as_str) {
        Some("msg") => Some(match msg(&args[1..]) {
            Ok(out) => {
                println!("{out}");
                0
            }
            Err(e) => {
                eprintln!("backspace msg: {e:#}");
                1
            }
        }),
        Some("tile") => Some(match crate::relay::cli(&args[1..]) {
            Ok(out) => {
                println!("{out}");
                0
            }
            Err(e) => {
                eprintln!("backspace tile: {e:#}");
                1
            }
        }),
        Some("mcp") => Some(match crate::mcp::serve() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("backspace mcp: {e:#}");
                1
            }
        }),
        _ => None,
    }
}

fn msg(args: &[String]) -> Result<String> {
    let b = bridge()?;
    match args.first().map(String::as_str) {
        // `msg send <who> [--reply <id>] <text>`: --reply keeps a
        // back-and-forth in one conversation (the ids come from `msg read`).
        Some("send") if args.len() >= 3 => {
            let to = args[1].clone();
            let mut rest = args[2..].to_vec();
            if to == "--reply" {
                bail!("usage: msg send <agent|main|all> [--reply <id>] <text>");
            }
            let mut reply_to = None;
            if let Some(i) = rest.iter().position(|a| a == "--reply") {
                if i + 1 < rest.len() {
                    reply_to = rest[i + 1].parse::<u64>().ok();
                    rest.drain(i..=i + 1);
                }
            }
            do_send(&b, &to, &rest.join(" "), reply_to)
        }
        Some("read") => do_read(&b),
        Some("agents") => do_agents(&b),
        // `msg conversation <agent>`: everything the two of you said.
        Some("conversation") | Some("thread") if args.len() >= 2 => do_conversation(&b, &args[1]),
        _ => bail!("usage: msg agents | msg send <agent|main|all> [--reply <id>] <text> | msg read | msg conversation <agent>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_and_read() {
        let mut s = ProjectState::empty("p");
        let mut a = serde_json::from_value::<crate::project::AgentRecord>(json!({
            "id": 0, "role": "main", "kind": "main", "parent": null, "depth": 0, "key": "main",
            "title": "Main", "brief": "", "depends_on": [], "status": "idle", "decision": null,
            "log": [], "cost_usd": 0.0, "input_tokens": 0, "output_tokens": 0, "deliverable": null,
            "budget_usd": null, "ticket": null, "branch": null, "worktree": null, "escalations": []
        }))
        .unwrap();
        s.agents.push(a.clone());
        a.id = 1;
        a.key = "api".into();
        s.agents.push(a);
        assert!(post(&mut s, "api", "nobody", "hi").is_err());
        post(&mut s, "api", "main", "schema is in db.sql").unwrap();
        post(&mut s, "main", "all", "freeze the API").unwrap();
        let mut c = Cursors::default();
        assert_eq!(c.take(&s, "main"), "[#0 api]: schema is in db.sql");
        assert_eq!(c.take(&s, "main"), "No new messages.");
        assert_eq!(c.take(&s, "api"), "[#1 main] (to everyone): freeze the API");
    }

    #[test]
    fn a_thread_is_the_pair_whichever_way_it_went() {
        assert_eq!(thread_of("main", "api"), thread_of("api", "main"));
        assert_eq!(thread_of("main", "api"), "api|main");
        assert_eq!(thread_of("api", "all"), "api~all");
        // To yourself is still a note-to-self, not a broadcast.
        assert_eq!(thread_of("api", "api"), "api~all");
    }

    #[test]
    fn conversations_group_replies_and_broadcasts_separately() {
        let mut s = ProjectState::empty("p");
        for (i, key) in ["main", "api"].iter().enumerate() {
            let mut a = serde_json::from_value::<crate::project::AgentRecord>(json!({
                "id": i, "role": if i == 0 { "main" } else { "sub" }, "kind": "worker",
                "parent": null, "depth": 0, "key": key,
                "title": key, "brief": "", "depends_on": [], "status": "idle", "decision": null,
                "log": [], "cost_usd": 0.0, "input_tokens": 0, "output_tokens": 0, "deliverable": null,
                "budget_usd": null, "ticket": null, "branch": null, "worktree": null, "escalations": []
            }))
            .unwrap();
            a.key = key.to_string();
            s.agents.push(a);
        }
        post(&mut s, "api", "main", "how should the token be stored?").unwrap();
        reply(&mut s, "main", "api", "as a hash, in users.token", Some(0)).unwrap();
        post(&mut s, "main", "all", "freeze the API").unwrap();
        let ts = conversations(&s);
        assert_eq!(ts.len(), 2, "one pair + one broadcast");
        // Newest first, but all three messages can share a millisecond; find
        // the conversations by what they are, not by their order.
        let pair_thread = ts.iter().find(|t| t.sides.len() == 2).expect("the pair");
        let bcast = ts.iter().find(|t| t.sides.len() == 1).expect("the broadcast");
        assert_eq!(pair_thread.messages.len(), 2);
        assert_eq!(pair_thread.sides, vec!["api".to_string(), "main".to_string()]);
        assert_eq!(pair_thread.messages[1].reply_to, Some(0));
        assert_eq!(bcast.messages.len(), 1);
        assert_eq!(bcast.messages[0].to, "all");
        // Ties on time break by id, so the order is still deterministic.
        let by_extremes = conversations(&s);
        assert!(by_extremes[0].last >= by_extremes[1].last);
        assert_eq!(between(&s, "main", "api").len(), 2);
        assert!(between(&s, "main", "nobody").is_empty());
        // A reply to a message that is not on the board is refused, not ignored.
        assert!(reply(&mut s, "api", "main", "re: nothing", Some(99)).is_err());
        assert_eq!(s.board.len(), 3);
    }
}
