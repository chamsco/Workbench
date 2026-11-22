//! An agent's sessions (Charles, 22 Nov): talking to an agent is one long thread, cut into
//! sessions. A session ends once nothing has happened for `GAP_MS` after its last reply finished
//! (a reply that takes ten minutes does not cut it); the next message starts a new one. An
//! automation run is always a session of its own, so an automation keeps its own history.

use crate::chat::{Msg, Role, Thread};

/// Quiet time after the last reply before the session is over.
pub const GAP_MS: u64 = 5 * 60_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// You talking with the agent.
    Talk,
    /// An automation run (its author is `Automation · <name>` or `Routine · <name>`).
    Automation(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    /// Messages `start..end` of the thread.
    pub start: usize,
    pub end: usize,
    /// When it began and when its last message finished (unix ms).
    pub from: u64,
    pub to: u64,
    /// Named from what it was about: the first thing asked in it.
    pub title: String,
    pub kind: Kind,
    /// Still going: a reply is running, or the gap has not passed yet.
    pub live: bool,
}

/// The automation a message is a run of, from its author line.
fn automation(m: &Msg) -> Option<String> {
    if m.role != Role::User {
        return None;
    }
    let a = m.author_name.as_deref()?;
    a.strip_prefix("Automation \u{b7} ").or_else(|| a.strip_prefix("Routine \u{b7} ")).map(|n| n.trim().to_string())
}

/// When a message was over: a reply when it finished (or its last step ended), else when it was sent.
fn ended(m: &Msg) -> u64 {
    let acts = m.acts.iter().filter_map(|a| a.end).max().unwrap_or(0);
    m.at.max(m.done_at).max(acts)
}

fn running(m: &Msg) -> bool {
    matches!(m.status, crate::chat::Status::Sending | crate::chat::Status::Streaming)
}

/// The thread's sessions, oldest first. `now` decides whether the last one is still live.
pub fn split(t: &Thread, now: u64) -> Vec<Session> {
    let mut out: Vec<Session> = Vec::new();
    for (i, m) in t.messages.iter().enumerate() {
        let auto = automation(m);
        let starts = match out.last() {
            None => true,
            Some(s) => auto.is_some() || (m.role == Role::User && m.at.saturating_sub(s.to) > GAP_MS),
        };
        if starts {
            let kind = auto.map_or(Kind::Talk, Kind::Automation);
            let title = match &kind {
                Kind::Automation(n) => n.clone(),
                Kind::Talk => crate::chat::title_from(&m.text),
            };
            out.push(Session { start: i, end: i + 1, from: m.at, to: ended(m), title, kind, live: false });
        } else if let Some(s) = out.last_mut() {
            s.end = i + 1;
            s.to = s.to.max(ended(m));
            if s.title.is_empty() && m.role == Role::User {
                s.title = crate::chat::title_from(&m.text);
            }
        }
    }
    if let Some(s) = out.last_mut() {
        s.live = t.messages[s.start..s.end].iter().any(running) || now.saturating_sub(s.to) <= GAP_MS;
    }
    for s in &mut out {
        // A name a model gave it (Chats::name_sessions); empty means it could not be named.
        if let Some(n) = t.session_names.get(&t.messages[s.start].id).filter(|n| !n.is_empty()) {
            s.title = n.clone();
        }
        if s.title.is_empty() {
            s.title = "Session".into();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::Status;

    fn msg(role: Role, text: &str, at: u64, done: u64, author: Option<&str>) -> Msg {
        let mut m: Msg = serde_json::from_value(serde_json::json!({ "id": format!("m{at}"), "role": if role == Role::User { "user" } else { "assistant" }, "text": text, "at": at, "status": "done" })).unwrap();
        m.done_at = done;
        m.author_name = author.map(str::to_string);
        m
    }

    fn thread(messages: Vec<Msg>) -> Thread {
        let mut t: Thread = serde_json::from_value(serde_json::json!({ "id": "t", "title": "Ez", "created": 0, "updated": 0, "route": { "kind": "cli", "provider": "claude", "model": null }, "messages": [] })).unwrap();
        t.messages = messages;
        t
    }

    const MIN: u64 = 60_000;

    #[test]
    fn a_session_ends_five_minutes_after_its_last_reply_finished() {
        let t = thread(vec![
            msg(Role::User, "Fix the login bug", 0, 0, None),
            // A long reply: started at 1 min, finished at 12 min.
            msg(Role::Assistant, "Fixed.", MIN, 12 * MIN, None),
            // 4 minutes after it finished: the same session, though 15 after it started.
            msg(Role::User, "And the logout?", 16 * MIN, 0, None),
            msg(Role::Assistant, "Done too.", 17 * MIN, 18 * MIN, None),
            // 10 minutes later: a new session, named after what it asks.
            msg(Role::User, "Write the release notes", 28 * MIN, 0, None),
            msg(Role::Assistant, "Written.", 29 * MIN, 30 * MIN, None),
        ]);
        let s = split(&t, 31 * MIN);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].start, s[0].end, s[0].title.as_str(), s[0].to), (0, 4, "Fix the login bug", 18 * MIN));
        assert_eq!((s[1].start, s[1].end, s[1].title.as_str()), (4, 6, "Write the release notes"));
        assert!(!s[0].live && s[1].live, "the last one is live until its gap passes");
        assert!(!split(&t, 36 * MIN)[1].live);
    }

    #[test]
    fn an_automation_run_is_a_session_of_its_own() {
        let t = thread(vec![
            msg(Role::User, "Hi", 0, 0, None),
            msg(Role::Assistant, "Hello.", MIN, 2 * MIN, None),
            msg(Role::User, "Automation is due…", 3 * MIN, 0, Some("Automation \u{b7} Morning PR triage")),
            msg(Role::Assistant, "Three PRs.", 3 * MIN, 4 * MIN, None),
        ]);
        let s = split(&t, 100 * MIN);
        assert_eq!(s.len(), 2, "within the gap, but a run starts its own session");
        assert_eq!(s[1].kind, Kind::Automation("Morning PR triage".into()));
        assert_eq!(s[1].title, "Morning PR triage");
    }

    #[test]
    fn a_session_takes_the_name_a_model_gave_it() {
        let mut t = thread(vec![msg(Role::User, "hey can you look at the thing from yesterday", 0, 0, None), msg(Role::Assistant, "Fixed.", MIN, 2 * MIN, None)]);
        assert_eq!(split(&t, 100 * MIN)[0].title, crate::chat::title_from("hey can you look at the thing from yesterday"));
        t.session_names.insert("m0".into(), "Login redirect fix".into());
        assert_eq!(split(&t, 100 * MIN)[0].title, "Login redirect fix");
        t.session_names.insert("m0".into(), String::new());
        assert_ne!(split(&t, 100 * MIN)[0].title, "", "an empty name keeps the first message");
    }

    #[test]
    fn a_running_reply_keeps_its_session_live() {
        let mut r = msg(Role::Assistant, "", MIN, 0, None);
        r.status = Status::Streaming;
        let t = thread(vec![msg(Role::User, "Long job", 0, 0, None), r]);
        assert!(split(&t, 60 * MIN)[0].live);
    }
}
