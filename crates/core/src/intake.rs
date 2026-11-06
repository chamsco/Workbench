//! M3 intake (PLAN §5.1, §6): what happens to a first message before any
//! planning. The message is classified (decision.rs); a task the classifier is
//! sure about goes down its path, and an unsure one is *asked about*, in code,
//! without a model call. Three paths:
//!
//! * research: a written report, nothing else. No ticket, branch, worker,
//!   approval or merge exists on this path, so there is no PR to open.
//! * one-shot: a lightweight spec (one ticket, a goal and a few checkable
//!   acceptance lines), handed off as a file, straight to a worker. The spec
//!   stands in for the plan, so there is no plan gate; merge stays a human call.
//! * long: unchanged (the planner interviews, plans and the user approves).
//!
//! The restrictions are enforced by [`allows`], which the harness applies to
//! every tool call and to the tool list a model is offered. A prompt describes
//! a path; this module is what keeps an agent on it.
//!
//! This module is pure (no model, no network): the harness wires it in.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::decision::{TaskType, Triage};
use crate::provider::ToolDef;

pub const DEFAULT_MAX_ASKS: u8 = 2;
/// "Lightweight": past this many acceptance lines a task is not a one-shot.
/// Provisional, like the triage threshold, until the §10 eval has run.
pub const ONE_SHOT_MAX_ACCEPTANCE: usize = 5;

pub const RESEARCH_PROMPT: &str = include_str!("prompts/research.md");
pub const ONE_SHOT_PROMPT: &str = include_str!("prompts/oneshot.md");

/// Where the current task stands in intake. Saved with the project, so a
/// question that is waiting and the path a task is on both survive a restart.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct IntakeState {
    /// The path the current task was given, until it is delivered. None: no
    /// task in progress (or intake is off).
    #[serde(default)]
    pub path: Option<TaskType>,
    /// A task waiting on the user's answer to "which kind is this?".
    #[serde(default)]
    pub pending: Option<PendingAsk>,
    /// The one-shot's ticket, once its spec is filed (there is only one).
    #[serde(default)]
    pub spec_key: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PendingAsk {
    /// The task as typed, plus anything the user said that was not an answer.
    pub text: String,
    /// Questions asked so far.
    pub asks: u8,
}

/// What intake does with a message.
#[derive(Debug, PartialEq)]
pub enum Next {
    /// Ask the user, and wait. The pending state to keep, and what to say.
    Ask(PendingAsk, String),
    /// Go: the task text (with anything the user added), its path, and what
    /// the router should take as the classification (None: classify it again).
    Go { text: String, path: TaskType, triage: Option<Triage>, note: Option<String> },
}

/// Which paths can run for this project's planner. Long always can. In a
/// coding CLI only long can (research and one-shot are refused before the CLI
/// starts, see [`cli_refusal`]), and the question must not offer what would be
/// refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paths {
    pub one_shot: bool,
    pub research: bool,
}

impl Paths {
    pub const ALL: Paths = Paths { one_shot: true, research: true };
    pub const LONG_ONLY: Paths = Paths { one_shot: false, research: false };

    /// What a planner that lives in a coding CLI (or not) can run.
    pub fn for_planner(in_cli: bool) -> Paths {
        if in_cli {
            Paths::LONG_ONLY
        } else {
            Paths::ALL
        }
    }
}

pub fn label(t: TaskType) -> &'static str {
    match t {
        TaskType::OneShot => "one-shot",
        TaskType::Long => "long",
        TaskType::Research => "research",
    }
}

/// A first message. `triage` is the classifier's answer (an error means it
/// could not decide, which is a reason to ask, not to guess). The first
/// question is always asked; `max_asks` only limits the re-asks in `answer`.
pub fn first(text: &str, triage: Result<Triage>) -> Next {
    first_in(text, triage, Paths::ALL)
}

/// [`first`], asking only about the paths that can run. A confident
/// classification is passed through whatever it names: an unavailable path is
/// refused by the harness with its own explanation, not guessed away here.
pub fn first_in(text: &str, triage: Result<Triage>, paths: Paths) -> Next {
    match triage {
        Ok(t @ Triage::Confident { task, .. }) => Next::Go { text: text.into(), path: task, triage: Some(t), note: None },
        Ok(Triage::Ask { best, p }) => ask(text.into(), 1, question_in(Some((best, p)), paths)),
        Err(_) => ask(text.into(), 1, question_in(None, paths)),
    }
}

/// The user's reply to a question. An answer starts that path on the original
/// task; anything else is kept as context and asked about again, up to
/// `max_asks` questions, after which the task goes the long way (the path that
/// plans with the user) and says so.
pub fn answer(pending: &PendingAsk, reply: &str, max_asks: u8) -> Next {
    answer_in(pending, reply, max_asks, Paths::ALL)
}

/// [`answer`], asking again only about the paths that can run. A reply that
/// names a path that cannot run still starts it (the harness refuses it with
/// the reason), so an answer is never silently turned into another path.
pub fn answer_in(pending: &PendingAsk, reply: &str, max_asks: u8, paths: Paths) -> Next {
    if let Some(path) = parse_answer(reply) {
        let triage = Some(Triage::Confident { task: path, p: 1.0 });
        return Next::Go { text: pending.text.clone(), path, triage, note: Some(format!("you said {}", label(path))) };
    }
    let text = format!("{}\n\n{}", pending.text, reply.trim());
    if pending.asks >= max_asks.max(1) {
        return Next::Go {
            text,
            path: TaskType::Long,
            triage: None,
            note: Some(format!("no clear answer after {} question{}, so this goes the long way: planned with you first", pending.asks, if pending.asks == 1 { "" } else { "s" })),
        };
    }
    ask(text, pending.asks + 1, format!("I didn't get a clear answer.\n\n{}", question_in(None, paths)))
}

fn ask(text: String, asks: u8, q: String) -> Next {
    Next::Ask(PendingAsk { text, asks }, q)
}

/// The question, with the classifier's guess when it has one.
pub fn question(best: Option<(TaskType, f64)>) -> String {
    question_in(best, Paths::ALL)
}

/// The question for the paths that can run. With all of them it is the
/// numbered menu; otherwise it lists only what can run and says, once, what
/// cannot and why, so nobody picks a path that would then be refused.
pub fn question_in(best: Option<(TaskType, f64)>, paths: Paths) -> String {
    let guess = match best {
        Some((t, p)) => format!(" My best guess is {} ({:.0}% sure), which is not enough to act on.", label(t), p * 100.0),
        None => String::new(),
    };
    if paths == Paths::ALL {
        return format!(
            "I can't tell what kind of task this is.{guess} Which is it?\n\n\
             1. one-shot: a small, contained change (one ticket, no interview)\n\
             2. long: needs planning with you first (an interview, then a plan you approve)\n\
             3. research: a written report only (no code changes, no PR)\n\n\
             Reply with the word or the number."
        );
    }
    let mut open = vec!["- long: needs planning with you first (an interview, then a plan you approve)"];
    let mut closed = Vec::new();
    for (can, name, line) in [
        (paths.one_shot, "one-shot", "- one-shot: a small, contained change (one ticket, no interview)"),
        (paths.research, "research", "- research: a written report only (no code changes, no PR)"),
    ] {
        if can {
            open.push(line);
        } else {
            closed.push(name);
        }
    }
    let (kinds, verb, them) = match closed.len() {
        1 => (closed[0].to_string(), "is", "it"),
        _ => (closed.join(" and "), "are", "them"),
    };
    format!(
        "I can't tell what kind of task this is.{guess}\n\n\
         This project's planner is a coding CLI, so only part of the menu can run until M6's sandbox:\n\n\
         {}\n\n\
         {kinds} {verb} not available: the CLI has its own file and shell tools, which Backspace can't restrict. Switch the planner to an API model to use {them}.\n\n\
         Reply with the word{}.",
        open.join("\n"),
        if open.len() == 1 { " `long` to go ahead" } else { "" },
    )
}

/// Which task type a reply names: a label (`research`, `long`, `one-shot`,
/// `one shot`, `one_shot`) or its number. A reply that names none, or more
/// than one, answers nothing.
pub fn parse_answer(reply: &str) -> Option<TaskType> {
    let lower = reply.to_lowercase();
    let bare = lower.trim().trim_end_matches([')', '.']);
    match bare {
        "1" => return Some(TaskType::OneShot),
        "2" => return Some(TaskType::Long),
        "3" => return Some(TaskType::Research),
        _ => {}
    }
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let mut found: Vec<TaskType> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let hit = match *w {
            "long" => Some(TaskType::Long),
            "research" => Some(TaskType::Research),
            "oneshot" => Some(TaskType::OneShot),
            "one" if words.get(i + 1) == Some(&"shot") => Some(TaskType::OneShot),
            _ => None,
        };
        if let Some(t) = hit {
            if !found.contains(&t) {
                found.push(t);
            }
        }
    }
    match found.as_slice() {
        [t] => Some(*t),
        _ => None,
    }
}

/// May an agent on `path` call `tool`? The one gate for what a path may do.
/// Long is unrestricted (the full planner, as before). The other two name
/// what they may use and nothing else is available: a tool added later is
/// off on those paths until someone adds it here.
pub fn allows(path: TaskType, tool: &str) -> bool {
    match path {
        TaskType::Long => true,
        TaskType::Research => ["read", "list", "submit_report"].contains(&tool),
        TaskType::OneShot => ["read", "list", "create_tickets", "work_tickets", "submit_deliverable"].contains(&tool),
    }
}

/// What a refused call says, so the model can get back on its path.
pub fn refusal(path: TaskType, tool: &str) -> String {
    match path {
        TaskType::Research => format!(
            "`{tool}` is not available on the research path: a research task reads and writes a report, and changes nothing (no tickets, branches or PRs). Use read and list to look, then submit_report."
        ),
        TaskType::OneShot => format!(
            "`{tool}` is not available on the one-shot path: you write the spec (create_tickets, exactly one ticket) and dispatch it (work_tickets); a worker does the work."
        ),
        TaskType::Long => format!("`{tool}` is not available to you"),
    }
}

/// Why a restricted path does not run when the planner lives in a coding CLI.
/// The CLI has its own file and shell tools, which code cannot gate until M6's
/// sandbox, so neither path can be held to its rules there. This is said to the
/// user as an error; the task is not turned into anything else.
pub fn cli_refusal(path: TaskType) -> String {
    match path {
        TaskType::Research => "research can't run in a coding CLI until M6's sandbox. The CLI has its own file and shell tools, and nothing here stops it changing files, running git or opening a PR, so a report-only task can't be kept report-only. Nothing was run and nothing was spent. Switch the planner to an API model to use the native research route, or ask for it as a long task.".to_string(),
        TaskType::OneShot => "a one-shot can't run in a coding CLI until M6's sandbox. The CLI has its own file and shell tools, which Backspace can't restrict, so it could edit files, run git or push instead of handing the ticket to a worker; the one-ticket rule covers only Backspace's own planning tools. Nothing was run and nothing was spent, and the task was not turned into a long one. Switch the planner to an API model to use the native one-shot route, or ask for it as a long task.".to_string(),
        TaskType::Long => "the long path runs in a coding CLI".to_string(),
    }
}

/// Why a side task (the shell's "start another task", POST /v1/task) is not
/// started while a research or one-shot task is open. It is the person's own
/// request, so it is refused to them in plain words, not as a tool error. A
/// research task makes no tickets or branches, and a one-shot has exactly one
/// ticket, its own; a side task would break either.
pub fn start_refusal(open: TaskType) -> String {
    match open {
        TaskType::Research => "a research task is open, and research makes no tickets or branches, so another task can't be started alongside it. Wait for its report, or press Stop to end it, then start the task.".to_string(),
        TaskType::OneShot => "a one-shot task is open, and a one-shot has exactly one ticket, its own, so another task can't be started alongside it (it would take that ticket's place). Wait for it to finish, or press Stop to end it, then start the task.".to_string(),
        TaskType::Long => "a long task is open".to_string(),
    }
}

/// Directory listing for the research and one-shot paths: `read` alone cannot
/// find a file, and the shell (`bash`) is not read-only until M6's sandbox.
pub fn list_tool() -> ToolDef {
    ToolDef {
        name: "list",
        description: "List a directory in the workspace (folders end in `/`). Defaults to the workspace root.",
        schema: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
    }
}

pub fn submit_report_tool() -> ToolDef {
    ToolDef {
        name: "submit_report",
        description: "Finish a research task: file the written report (Markdown). Call it once, when you have the answer. It ends the task; nothing is changed, branched or merged.",
        schema: serde_json::json!({"type": "object", "properties": {
            "title": {"type": "string"},
            "body": {"type": "string", "description": "the report, in Markdown: what you found, what you could not find, how sure you are, and where each claim came from"}
        }, "required": ["title", "body"]}),
    }
}

pub(crate) fn safe_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A one-shot's lightweight spec: one ticket, a goal, a few acceptance lines
/// and, when there is one, a command that must exit 0.
#[derive(Debug, Clone, PartialEq)]
pub struct OneShotSpec {
    pub key: String,
    pub title: String,
    pub goal: String,
    pub acceptance: Vec<String>,
    pub check: Option<String>,
}

impl OneShotSpec {
    /// Validate the planner's `create_tickets` input as a one-shot spec. The
    /// refusal says why, so the model can fix it (or say it is a long task).
    pub fn from_tickets(input: &Value) -> Result<OneShotSpec> {
        let tickets = input["tickets"].as_array().context("`tickets` must be an array")?;
        if tickets.len() != 1 {
            bail!(
                "a one-shot task is exactly one ticket, and you gave {}. If it needs more than one, it is a long task: tell the user it needs planning and stop.",
                tickets.len()
            );
        }
        let t = &tickets[0];
        let s = |k: &str| t[k].as_str().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
        let key = s("key").context("the ticket needs a `key`")?;
        if !safe_key(&key) {
            bail!("ticket `key` `{key}` must be letters, digits, - or _ (it names a git branch and a folder)");
        }
        let goal = s("what_to_build").or_else(|| s("description")).context("the ticket needs `what_to_build`: the goal")?;
        let acceptance: Vec<String> = t["acceptance"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::trim)).filter(|x| !x.is_empty()).map(str::to_owned).collect())
            .unwrap_or_default();
        if acceptance.is_empty() {
            bail!("a one-shot needs at least one acceptance line: something a person or a command can check");
        }
        if acceptance.len() > ONE_SHOT_MAX_ACCEPTANCE {
            bail!(
                "a one-shot has at most {ONE_SHOT_MAX_ACCEPTANCE} acceptance lines and this has {}: that is a long task. Tell the user it needs planning and stop.",
                acceptance.len()
            );
        }
        for (field, what) in [("blocked_by", "one ticket has nothing to wait for"), ("covers", "there is no SPEC.md to cover")] {
            if t[field].as_array().is_some_and(|a| !a.is_empty()) {
                bail!("`{field}` has no place in a one-shot: {what}");
            }
        }
        Ok(OneShotSpec { title: s("title").unwrap_or_else(|| key.clone()), key, goal, acceptance, check: s("check") })
    }

    pub fn markdown(&self) -> String {
        let mut m = format!("# Spec: {}\n\nTicket: `{}`\n\n## Goal\n\n{}\n\n## Acceptance\n\n", self.title, self.key, self.goal);
        for a in &self.acceptance {
            m.push_str(&format!("- {a}\n"));
        }
        if let Some(c) = &self.check {
            m.push_str(&format!("\n## Check\n\n`{c}` must exit 0.\n"));
        }
        m
    }

    /// The handoff file: `.backspace/handoff/<key>/spec.md` under `root`.
    /// Returns its path relative to `root`.
    pub fn write(&self, root: &Path) -> Result<String> {
        if !safe_key(&self.key) {
            bail!("ticket key `{}` cannot name a folder", self.key);
        }
        let rel = format!(".backspace/handoff/{}/spec.md", self.key);
        let path = root.join(&rel);
        std::fs::create_dir_all(path.parent().expect("has a parent"))?;
        std::fs::write(&path, self.markdown())?;
        Ok(rel)
    }
}

/// A research task's output.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub title: String,
    pub body: String,
}

impl Report {
    pub fn from_json(input: &Value) -> Result<Report> {
        let s = |k: &str| input[k].as_str().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
        Ok(Report {
            title: s("title").context("a report needs a `title`")?,
            body: s("body").context("a report needs a `body`: the report itself")?,
        })
    }

    /// Write it to `.backspace/reports/<title-slug>.md` under `root`, never over
    /// an earlier report. Returns the path relative to `root`.
    pub fn write(&self, root: &Path) -> Result<String> {
        let mut slug: String = self.title.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        slug = slug.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
        slug.truncate(60);
        let slug = slug.trim_matches('-');
        let slug = if slug.is_empty() { "report" } else { slug };
        let dir = root.join(".backspace/reports");
        std::fs::create_dir_all(&dir)?;
        let text = format!("# {}\n\n{}\n", self.title.replace(['\r', '\n'], " "), self.body);
        for n in 1.. {
            let name = if n == 1 { format!("{slug}.md") } else { format!("{slug}-{n}.md") };
            // `create_new`: two reports with one title cannot pick the same name.
            match std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(&name)) {
                Ok(mut f) => {
                    use std::io::Write;
                    f.write_all(text.as_bytes())?;
                    return Ok(format!(".backspace/reports/{name}"));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        unreachable!("the loop returns or continues")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_asks_when_the_classifier_is_unsure_or_fails() {
        let unsure = Ok(Triage::Ask { best: TaskType::Long, p: 0.5 });
        match first("do the thing", unsure) {
            Next::Ask(p, q) => {
                assert_eq!((p.text.as_str(), p.asks), ("do the thing", 1));
                assert!(q.contains("long") && q.contains("50%"), "{q}");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(first("x", Err(anyhow::anyhow!("boom"))), Next::Ask(..)));
    }

    #[test]
    fn first_goes_when_the_classifier_is_sure() {
        let t = Triage::Confident { task: TaskType::Research, p: 0.9 };
        match first("compare databases", Ok(t)) {
            Next::Go { text, path, triage, note } => {
                assert_eq!((text.as_str(), path, triage, note), ("compare databases", TaskType::Research, Some(t), None));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_non_answer_is_kept_and_asked_again_until_the_limit() {
        let p = PendingAsk { text: "hmm".into(), asks: 1 };
        let Next::Ask(p2, q) = answer(&p, "dunno", 2) else { panic!("expected a second question") };
        assert_eq!((p2.text.as_str(), p2.asks), ("hmm\n\ndunno", 2));
        assert!(q.starts_with("I didn't get a clear answer"));
        let Next::Go { text, path, triage, note } = answer(&p2, "no idea", 2) else { panic!("expected to go") };
        assert_eq!((path, triage), (TaskType::Long, None), "a task nobody classified does not start on the frontier");
        assert_eq!(text, "hmm\n\ndunno\n\nno idea");
        assert!(note.unwrap().contains("2 questions"));
        // One question allowed: the first non-answer already goes.
        assert!(matches!(answer(&p, "dunno", 1), Next::Go { path: TaskType::Long, .. }));
    }

    #[test]
    fn an_answer_is_confident_by_definition() {
        let p = PendingAsk { text: "hmm".into(), asks: 1 };
        let Next::Go { text, path, triage, .. } = answer(&p, "Research", 2) else { panic!() };
        assert_eq!((text.as_str(), path), ("hmm", TaskType::Research));
        assert_eq!(triage, Some(Triage::Confident { task: TaskType::Research, p: 1.0 }));
    }
}
