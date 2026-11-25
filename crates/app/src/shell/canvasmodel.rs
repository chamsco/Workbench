//! What a stream's Canvas shows (Charles, 24 Nov), read from the thread as it is: each ask is a
//! start node; what it took in (skills, memory, files) hangs off its left; the questions it asked
//! and your answers sit under it; the helpers it sent out (side streams, subagents, web searches)
//! branch to the right with a live preview each; then what came back, then the plan, whose steps
//! fill the Tasks tray. Nothing here is stored: the canvas is laid out again from the thread.

use backspace_core::chat::{Msg, Role, Status, Thread};

use super::plan::{self, Step};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Start,
    Input,
    Ask,
    Helper,
    Result,
    Plan,
    /// One version of what the stream made (the coding canvas).
    Version,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Live {
    Working,
    Done,
    Failed,
}

#[derive(Clone, Debug)]
pub(crate) struct Card {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    /// Under the title: an input's kind, a helper's state, a result's source.
    pub sub: String,
    pub lines: Vec<String>,
    /// A picture to preview (a path), when there is one.
    pub image: Option<String>,
    /// Your answer to a question, as a chip under it.
    pub answer: Option<String>,
    pub live: Live,
    /// The stream a click opens (a helper's side stream).
    pub open: Option<String>,
    pub at: u64,
    /// A version's board node, which a click opens.
    pub node: Option<String>,
    /// The newest version.
    pub latest: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Pill {
    pub text: String,
    /// Still working: the orb turns and the words shimmer.
    pub live: bool,
    /// The plan is ready: Next opens the Tasks tray.
    pub next: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Flow {
    pub cards: Vec<Card>,
    /// From card, to card (indices), and whether the line is a side branch (the inputs).
    pub edges: Vec<(usize, usize, bool)>,
    pub tasks: Vec<Step>,
    pub pill: Pill,
    /// Where the newest ask's cards begin in `cards`: the camera follows that turn.
    pub last_turn: usize,
}

pub(crate) const START_W: f32 = 440.;
const INPUT_W: f32 = 170.;
const HELPER_W: f32 = 300.;
const RESULT_W: f32 = 380.;
const PLAN_W: f32 = 400.;
const COL_GAP: f32 = 90.;
const TURN_GAP: f32 = 140.;

/// A user message that carries on the turn before it rather than asking something new: a side
/// stream's result coming back, or an edit to the plan.
fn carries_on(m: &Msg) -> bool {
    let t = m.text.trim_start();
    t.starts_with("Result from the side thread") || t.starts_with("Plan edit:") || t.starts_with("Letter from ")
}

/// A finished reply that only asks something: ends on a question and ships no plan.
fn asks(m: &Msg) -> Option<String> {
    if m.role != Role::Assistant || m.status != Status::Done {
        return None;
    }
    let text = plan::strip(&m.text);
    let last = text.trim().lines().rev().find(|l| !l.trim().is_empty())?.trim().to_string();
    (last.ends_with('?') && plan::parse(&m.text).is_none()).then_some(last.trim_start_matches(['*', '-', ' ']).to_string())
}

fn first_line(s: &str, n: usize) -> String {
    let l = s.trim().lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().trim_start_matches(['#', '*', '-', ' ']).trim_end_matches('*').to_string();
    clip(&l, n)
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>().trim_end())
    }
}

fn json_field(input: &str, names: &[&str]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(input).ok()?;
    names.iter().find_map(|n| v[*n].as_str().map(str::to_string))
}

fn host(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or(url).trim_start_matches("www.").to_string()
}

/// Paragraphs of a reply worth showing on its result card: not the plan, not code, not headings
/// alone; up to `n` lines.
fn gist(text: &str, n: usize) -> Vec<String> {
    let body = plan::strip(text);
    let mut out = Vec::new();
    let mut fence = false;
    for l in body.lines() {
        let t = l.trim();
        if t.starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence || t.is_empty() || t.starts_with('#') || t.starts_with('|') || t.starts_with("![") {
            continue;
        }
        out.push(clip(t.trim_start_matches(['-', '*', ' ']).trim_end_matches('*'), 120));
        if out.len() == n {
            break;
        }
    }
    out
}

/// The first picture a reply made or pointed at: an attachment, else a markdown image path.
fn picture(m: &Msg, thread: &str, files: &dyn Fn(&str, &str) -> Option<String>) -> Option<String> {
    if let Some(a) = m.attachments.iter().find(|a| a.mime.starts_with("image/")) {
        if let Some(p) = files(thread, &a.id) {
            return Some(p);
        }
    }
    let i = m.text.find("](")?;
    let rest = &m.text[i + 2..];
    let p = rest.split(')').next()?.trim();
    let lower = p.to_ascii_lowercase();
    ([".png", ".jpg", ".jpeg", ".gif", ".webp"].iter().any(|e| lower.ends_with(e)) && !p.starts_with("http")).then(|| p.to_string())
}

struct Turn<'a> {
    ask: &'a Msg,
    /// The rest of the turn in order: replies, your answers, results coming back.
    rest: Vec<&'a Msg>,
}

fn turns(t: &Thread) -> Vec<Turn<'_>> {
    let mut out: Vec<Turn> = Vec::new();
    let mut asked = false;
    for m in &t.messages {
        match m.role {
            Role::User if out.is_empty() || !(asked || carries_on(m)) => {
                out.push(Turn { ask: m, rest: Vec::new() });
                asked = false;
            }
            _ => {
                if let Some(turn) = out.last_mut() {
                    turn.rest.push(m);
                }
                if m.role == Role::Assistant {
                    asked = asks(m).is_some();
                } else {
                    asked = false;
                }
            }
        }
    }
    out
}

/// The canvas of `t`. `sides` are its side streams; `busy` whether it is working now; `files`
/// finds an attachment's file on disk (thread, attachment id).
pub(crate) fn build(t: &Thread, sides: &[Thread], busy: bool, files: &dyn Fn(&str, &str) -> Option<String>) -> Flow {
    let mut f = Flow::default();
    let ts = turns(t);
    let mut y0 = 0.0f32;
    let n_turns = ts.len();
    for (ti, turn) in ts.iter().enumerate() {
        let last_turn = ti + 1 == n_turns;
        let end_at = ts.get(ti + 1).map_or(u64::MAX, |n| n.ask.at);
        let mut col_bottom = Vec::new();

        // The ask.
        let title = clip(turn.ask.text.trim(), 220);
        let lines_est = (title.chars().count() as f32 / 44.).ceil().max(1.);
        let start_h = 28. + lines_est * 26.;
        let start = f.cards.len();
        f.last_turn = start;
        f.cards.push(Card { id: format!("start-{}", turn.ask.id), kind: Kind::Start, title, sub: String::new(), lines: Vec::new(), image: None, answer: None, live: Live::Done, open: None, at: turn.ask.at, x: 0., y: y0, w: START_W, h: start_h, node: None, latest: false });

        // What it took in, on the start's left.
        let mut inputs: Vec<(String, &'static str)> = Vec::new();
        for a in &turn.ask.attachments {
            inputs.push((clip(&a.name, 24), "File"));
        }
        for m in turn.rest.iter().filter(|m| m.role == Role::Assistant) {
            for a in &m.acts {
                let item = match a.name.as_str() {
                    "Skill" => json_field(&a.input, &["skill", "command", "name"]).map(|s| (s, "Skill")),
                    "Read" => {
                        let p = a.path.clone().or_else(|| json_field(&a.input, &["file_path", "path"])).unwrap_or_default();
                        let low = p.to_ascii_lowercase();
                        if low.contains("memory") || low.ends_with("claude.md") || low.ends_with("agents.md") {
                            Some((if low.contains("memory") { "Your memory".to_string() } else { p.rsplit(['/', '\\']).next().unwrap_or(&p).to_string() }, "Memory"))
                        } else {
                            None
                        }
                    }
                    n if n.starts_with("mcp__") => n.strip_prefix("mcp__").and_then(|r| r.split_once("__")).map(|(s, _)| (s.to_string(), "Connector")),
                    _ => None,
                };
                if let Some((name, kind)) = item {
                    if !inputs.iter().any(|(n, _)| n == &name) {
                        inputs.push((name, kind));
                    }
                }
            }
        }
        inputs.truncate(5);
        let in_h = 38.;
        let in_top = y0 + (start_h - inputs.len() as f32 * (in_h + 6.)) / 2.;
        for (k, (name, kind)) in inputs.into_iter().enumerate() {
            let i = f.cards.len();
            f.cards.push(Card { id: format!("in-{}-{k}", turn.ask.id), kind: Kind::Input, title: name, sub: kind.to_string(), lines: Vec::new(), image: None, answer: None, live: Live::Done, open: None, at: turn.ask.at, x: -INPUT_W - 56., y: in_top + k as f32 * (in_h + 6.), w: INPUT_W, h: in_h, node: None, latest: false });
            f.edges.push((i, start, true));
        }

        // Questions and your answers, under the ask.
        let mut qy = y0 + start_h + 26.;
        for (k, m) in turn.rest.iter().enumerate() {
            let Some(q) = asks(m) else { continue };
            let answer = turn.rest[k + 1..].iter().find(|n| n.role == Role::User).map(|n| clip(n.text.trim(), 80));
            let h = 30. + (q.chars().count() as f32 / 52.).ceil() * 20. + if answer.is_some() { 38. } else { 0. };
            f.cards.push(Card { id: format!("ask-{}", m.id), kind: Kind::Ask, title: clip(&q, 200), sub: String::new(), lines: Vec::new(), image: None, answer, live: Live::Done, open: None, at: m.at, x: 0., y: qy, w: START_W, h, node: None, latest: false });
            qy += h + 18.;
        }
        col_bottom.push(qy);

        // Helpers, to the right: side streams made in this turn, subagents, web reads.
        let hx = START_W + COL_GAP;
        let mut helpers: Vec<Card> = Vec::new();
        for s in sides.iter().filter(|s| s.created >= turn.ask.at && s.created < end_at) {
            let last = s.messages.iter().rev().find(|m| m.role == Role::Assistant);
            let working = s.messages.last().is_some_and(|m| matches!(m.status, Status::Sending | Status::Streaming));
            let failed = last.is_some_and(|m| m.status == Status::Error);
            let image = s.messages.iter().rev().find_map(|m| picture(m, &s.id, files));
            let doing = last.and_then(|m| m.acts.iter().rev().find(|a| a.end.is_none()).map(|a| a.name.clone()));
            let sub = if working { doing.map_or_else(|| "Working…".into(), |d| format!("{d}…")) } else if failed { "Stopped on an error".into() } else { "Done".into() };
            let lines = last.map(|m| gist(&m.text, 3)).unwrap_or_default();
            let title = s.title.trim_start_matches("↳ ").to_string();
            helpers.push(Card { id: format!("side-{}", s.id), kind: Kind::Helper, title: clip(&title, 48), sub, lines, image, answer: None, live: if working { Live::Working } else if failed { Live::Failed } else { Live::Done }, open: Some(s.id.clone()), at: s.created, x: hx, y: 0., w: HELPER_W, h: 0., node: None, latest: false });
        }
        for m in turn.rest.iter().filter(|m| m.role == Role::Assistant) {
            let running = matches!(m.status, Status::Sending | Status::Streaming);
            for a in &m.acts {
                let (title, sub) = match a.name.as_str() {
                    "Task" | "Agent" => (json_field(&a.input, &["description", "subagent_type"]).unwrap_or_else(|| "Subagent".into()), "Subagent"),
                    "WebFetch" => (json_field(&a.input, &["url"]).map(|u| host(&u)).unwrap_or_else(|| "A page".into()), "Reading the page"),
                    "WebSearch" => (json_field(&a.input, &["query"]).unwrap_or_else(|| "The web".into()), "Searching the web"),
                    _ => continue,
                };
                let working = a.end.is_none() && running;
                let lines = a.output.lines().map(str::trim).filter(|l| !l.is_empty()).take(3).map(|l| clip(l, 90)).collect();
                helpers.push(Card { id: format!("act-{}-{}", m.id, a.id), kind: Kind::Helper, title: clip(&title, 48), sub: if working { format!("{sub}…") } else { sub.to_string() }, lines, image: None, answer: None, live: if a.error { Live::Failed } else if working { Live::Working } else { Live::Done }, open: None, at: a.start, x: hx, y: 0., w: HELPER_W, h: 0., node: None, latest: false });
            }
        }
        helpers.sort_by_key(|h| h.at);
        helpers.truncate(6);
        let mut hy = y0;
        let mut helper_ix = Vec::new();
        for mut h in helpers {
            if h.image.is_some() {
                h.lines.truncate(1);
            }
            h.h = 60. + if h.image.is_some() { 152. } else { 0. } + 17. * h.lines.len() as f32;
            h.y = hy;
            hy += h.h + 24.;
            helper_ix.push(f.cards.len());
            f.edges.push((start, f.cards.len(), false));
            f.cards.push(h);
        }
        col_bottom.push(hy);

        // What came back: the turn's last reply that is not only a question.
        let rx = if helper_ix.is_empty() { hx } else { hx + HELPER_W + COL_GAP };
        let reply = turn.rest.iter().rev().find(|m| m.role == Role::Assistant && asks(m).is_none() && !plan::strip(&m.text).trim().is_empty());
        let mut result_ix = None;
        if let Some(m) = reply {
            let working = matches!(m.status, Status::Sending | Status::Streaming);
            let image = picture(m, &t.id, files);
            let changed: Vec<String> = m.acts.iter().filter(|a| matches!(a.name.as_str(), "Edit" | "Write" | "MultiEdit")).filter_map(|a| a.path.clone().or_else(|| json_field(&a.input, &["file_path", "path"]))).map(|p| p.rsplit(['/', '\\']).next().unwrap_or(&p).to_string()).fold(Vec::new(), |mut v, p| {
                if !v.contains(&p) {
                    v.push(p);
                }
                v
            });
            let mut lines = gist(&m.text, if image.is_some() { 3 } else { 5 });
            let title = lines.first().map(|l| clip(l, 70)).unwrap_or_else(|| "Reply".into());
            if !lines.is_empty() {
                lines.remove(0);
            }
            let sub = if changed.is_empty() { String::new() } else { format!("Changed {}", clip(&changed.join(", "), 60)) };
            let h = 52. + if image.is_some() { 196. } else { 0. } + 17. * lines.len() as f32 + if sub.is_empty() { 0. } else { 22. };
            let i = f.cards.len();
            f.cards.push(Card { id: format!("res-{}", m.id), kind: Kind::Result, title, sub, lines, image, answer: None, live: if working { Live::Working } else if m.status == Status::Error { Live::Failed } else { Live::Done }, open: None, at: m.at, x: rx, y: y0, w: RESULT_W, h, node: None, latest: false });
            if helper_ix.is_empty() {
                f.edges.push((start, i, false));
            } else {
                for &h in &helper_ix {
                    f.edges.push((h, i, false));
                }
            }
            col_bottom.push(y0 + h);
            result_ix = Some(i);
        }

        // The plan this turn left, after what came back.
        if let Some((m, steps)) = turn.rest.iter().rev().find_map(|m| plan::parse(&m.text).map(|s| (m, s))) {
            let px_ = rx + if result_ix.is_some() { RESULT_W + COL_GAP } else { 0. };
            let done = steps.iter().filter(|s| s.state == "done").count();
            let h = 86. + 20. * steps.len().min(8) as f32;
            let i = f.cards.len();
            f.cards.push(Card {
                id: format!("plan-{}", m.id),
                kind: Kind::Plan,
                title: first_line(&plan::strip(&m.text), 60).chars().take(60).collect::<String>(),
                sub: format!("{done} of {} done", steps.len()),
                lines: steps.iter().take(8).map(|s| format!("{}\u{1f}{}", s.state, s.text)).collect(),
                image: None,
                answer: None,
                live: Live::Done,
                open: None,
                at: m.at,
                x: px_,
                y: y0,
                w: PLAN_W,
                h,
                node: None,
                latest: false,
            });
            f.edges.push((result_ix.unwrap_or(start), i, false));
            col_bottom.push(y0 + h);
        }

        if last_turn {
            f.pill = pill(t, turn, busy, &f, &helper_ix);
        }
        y0 = col_bottom.into_iter().fold(y0 + start_h, f32::max) + TURN_GAP;
    }
    f.tasks = plan::current(t).unwrap_or_default();
    if ts.is_empty() {
        f.pill = Pill { text: "Ask something to start".into(), live: false, next: false };
    }
    f
}

fn pill(t: &Thread, turn: &Turn, busy: bool, f: &Flow, helpers: &[usize]) -> Pill {
    let working = helpers.iter().filter(|&&i| f.cards[i].live == Live::Working).count();
    let last = t.messages.last();
    if busy || working > 0 {
        let text = if working > 1 {
            format!("{working} helpers at work")
        } else if working == 1 {
            format!("A helper on {}", f.cards[helpers.iter().copied().find(|&i| f.cards[i].live == Live::Working).unwrap_or(0)].title.to_lowercase())
        } else if let Some(a) = last.and_then(|m| m.acts.iter().rev().find(|a| a.end.is_none())) {
            match a.name.as_str() {
                "Thinking" => "Thinking".into(),
                "TodoWrite" => "Writing the plan".into(),
                "Read" | "Grep" | "Glob" | "LS" => "Reading".into(),
                "Edit" | "Write" | "MultiEdit" => "Making the changes".into(),
                "Bash" | "PowerShell" => "Running a command".into(),
                "WebSearch" | "WebFetch" => "Reading the web".into(),
                n => format!("Using {}", n.strip_prefix("mcp__").and_then(|r| r.split_once("__")).map_or(n, |(s, _)| s)),
            }
        } else if last.is_some_and(|m| m.role == Role::Assistant && !m.text.trim().is_empty()) {
            "Writing".into()
        } else {
            "Thinking".into()
        };
        return Pill { text, live: true, next: false };
    }
    if turn.rest.last().is_some_and(|m| asks(m).is_some()) {
        return Pill { text: "A question before I go on".into(), live: false, next: false };
    }
    if last.is_some_and(|m| m.status == Status::Error) {
        return Pill { text: "Stopped on an error".into(), live: false, next: false };
    }
    let open = f.tasks_open_count(t);
    if open > 0 {
        return Pill { text: format!("The plan is ready, with {open} task{}", if open == 1 { "" } else { "s" }), live: false, next: true };
    }
    Pill { text: "Done".into(), live: false, next: false }
}

impl Flow {
    fn tasks_open_count(&self, t: &Thread) -> usize {
        plan::current(t).map_or(0, |s| s.iter().filter(|s| s.state != "done").count())
    }

    /// The box round the newest ask's cards, which the camera follows.
    pub(crate) fn extent(&self) -> Option<(f32, f32, f32, f32)> {
        let mut it = self.cards.iter().skip(self.last_turn);
        let c0 = it.next()?;
        let mut b = (c0.x, c0.y, c0.x + c0.w, c0.y + c0.h);
        for c in it {
            b = (b.0.min(c.x), b.1.min(c.y), b.2.max(c.x + c.w), b.3.max(c.y + c.h));
        }
        Some(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(msgs: serde_json::Value) -> Thread {
        let route = serde_json::json!({ "kind": "cli", "provider": "claude", "model": null });
        serde_json::from_value(serde_json::json!({ "id": "t", "title": "Trip", "created": 1, "updated": 1, "route": route, "messages": msgs })).unwrap()
    }

    fn m(id: &str, role: &str, text: &str, at: u64) -> serde_json::Value {
        serde_json::json!({ "id": id, "role": role, "text": text, "at": at, "status": "done" })
    }

    #[test]
    fn an_answer_to_a_question_stays_in_the_ask_it_answers() {
        // Catches: every user message read as a new ask, which splits one piece of work in two
        // and loses the answer chip under the question.
        let t = thread(serde_json::json!([
            m("1", "user", "Plan two weeks in San Francisco", 1),
            m("2", "assistant", "Which two weeks in March?", 2),
            m("3", "user", "Mar 2 to 16", 3),
            m("4", "assistant", "Here is the plan.\n\nTake the SoMa loft.", 4),
            m("5", "user", "Now book the flights", 5),
        ]));
        let f = build(&t, &[], false, &|_, _| None);
        let starts: Vec<&str> = f.cards.iter().filter(|c| c.kind == Kind::Start).map(|c| c.title.as_str()).collect();
        assert_eq!(starts, ["Plan two weeks in San Francisco", "Now book the flights"]);
        let ask = f.cards.iter().find(|c| c.kind == Kind::Ask).expect("the question is on the canvas");
        assert_eq!(ask.answer.as_deref(), Some("Mar 2 to 16"));
    }
}

/// The coding canvas (Charles, 24 Nov): the lineage of what the stream made. Each node of its
/// Board is a version, numbered in order along the main line with the newest marked Latest; side
/// streams' work forks off on a row below and joins where it was sent back. Built from the
/// Board's nodes and links, so a click opens the same node as the Board does.
pub(crate) fn lineage(b: &backspace_core::whiteboard::Board) -> Flow {
    let mut f = Flow::default();
    let mut main: Vec<&backspace_core::whiteboard::Node> = b.nodes.iter().filter(|n| n.via.is_none()).collect();
    main.sort_by_key(|n| n.at);
    let mut lanes: Vec<String> = Vec::new();
    for n in b.nodes.iter().filter(|n| n.via.is_some()) {
        let v = n.via.clone().unwrap_or_default();
        if !lanes.contains(&v) {
            lanes.push(v);
        }
    }
    let height = |n: &backspace_core::whiteboard::Node| 70. + if n.images.is_empty() { 0. } else { 128. } + 17. * n.changes.len().min(3) as f32 + 30.;
    // Versions in time order across all lanes, so a version number says when it was made.
    let mut order: Vec<&backspace_core::whiteboard::Node> = b.nodes.iter().collect();
    order.sort_by_key(|n| n.at);
    let newest = main.last().map(|n| n.id.clone());
    let mut ix: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let colw = 280.;
    let gap = 90.;
    for (k, n) in order.iter().enumerate() {
        let lane = n.via.as_ref().and_then(|v| lanes.iter().position(|l| l == v)).map_or(0, |p| p + 1);
        let x = k as f32 * (colw + gap);
        let y = lane as f32 * 330.;
        let mut lines: Vec<String> = n.changes.iter().filter(|c| !c.contains("](")).take(3).map(|c| clip(c, 70)).collect();
        for r in n.refs.iter().filter(|r| r.kind == "Page" || r.kind == "Artifact") {
            lines.push(format!("{}: {}", r.kind, clip(r.text.rsplit(['/', '\\']).next().unwrap_or(&r.text), 40)));
        }
        let commit = n.refs.iter().find(|r| r.kind == "Commit").map(|r| r.text.clone());
        let i = f.cards.len();
        ix.insert(n.id.clone(), i);
        f.cards.push(Card {
            id: format!("ver-{}", n.id),
            kind: Kind::Version,
            title: clip(&n.title, 44),
            sub: format!("v{}\u{1f}{}\u{1f}{}", k + 1, super::boardview::clock(n.at), commit.unwrap_or_default()),
            lines,
            image: n.images.first().cloned(),
            answer: n.via.as_ref().map(|v| format!("via {}", clip(v, 30))),
            live: Live::Done,
            open: None,
            at: n.at,
            node: Some(n.id.clone()),
            latest: newest.as_deref() == Some(n.id.as_str()),
            x,
            y,
            w: colw,
            h: height(n),
        });
    }
    for l in &b.links {
        if let (Some(&a), Some(&c)) = (ix.get(&l.from), ix.get(&l.to)) {
            f.edges.push((a, c, l.fork));
        }
    }
    f.last_turn = f.cards.len().saturating_sub(4);
    f
}
