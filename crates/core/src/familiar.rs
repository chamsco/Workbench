//! Familiars (Copilots board rounds 2-4): a goal you hand to a harness on the
//! long path. It plans (you approve the spec), Rampart seals the tests,
//! Builders work the tickets, and once the main agent is done the familiar
//! carries the run through Verify, Ship and Learn on its own (deliver.rs).
//! It stops for you at every ask the harness already has: the spec's yes,
//! the baseline commands, Rampart's spot-check, a delivery to approve, a
//! refused Verify.
//!
//! This module is the app-facing part: the list of familiars
//! (`<data>/familiars.json`), what a familiar looks like right now ([`view`],
//! a pure function of the harness state and the project's files), and
//! [`drive`], the step the app takes on every change.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::deliver::{self, DeliveryRecord, Step, Target, TicketFacts, VerifyReport};
use crate::phase::{Phase, PhaseState};
use crate::project::{AgentStatus, LogKind, ProjectState};

pub const FILE: &str = "familiars.json";
/// Events kept in the view: the newest ones.
const EVENTS: usize = 14;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Record {
    pub id: String,
    /// The sidebar label: the goal's first words.
    pub name: String,
    pub goal: String,
    pub folder: PathBuf,
    /// Where Ship delivers.
    pub target: Target,
    pub created: u64,
}

pub fn load(data: &Path) -> Vec<Record> {
    std::fs::read_to_string(data.join(FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(data: &Path, all: &[Record]) -> Result<()> {
    std::fs::create_dir_all(data)?;
    let p = data.join(FILE);
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(all)?)?;
    std::fs::rename(tmp, p)?;
    Ok(())
}

fn short_name(goal: &str) -> String {
    let words: Vec<&str> = goal.split_whitespace().take(5).collect();
    words.join(" ").trim_end_matches([',', '.', ':', ';']).to_string()
}

/// Remember a new familiar. The folder must exist: a familiar works in one.
pub fn add(data: &Path, goal: &str, folder: &Path, target: Target) -> Result<Record> {
    let goal = goal.trim();
    if goal.is_empty() {
        bail!("a familiar needs a goal");
    }
    if !folder.is_dir() {
        bail!("{} is not a folder: a familiar works in one", folder.display());
    }
    let folder = dunce::canonicalize(folder).context("reading the folder")?;
    let created = crate::chat::now_ms();
    let rec = Record { id: format!("fam-{created}"), name: short_name(goal), goal: goal.to_string(), folder, target, created };
    let mut all = load(data);
    all.push(rec.clone());
    save(data, &all)?;
    Ok(rec)
}

pub fn remove(data: &Path, id: &str) -> Result<()> {
    let mut all = load(data);
    all.retain(|r| r.id != id);
    save(data, &all)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    NeedsYou,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Done,
    Agent,
    Alert,
    Note,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub kind: EventKind,
    pub text: String,
}

/// One subagent: a Builder on a ticket, Rampart, a scout.
#[derive(Clone, Debug, PartialEq)]
pub struct Crew {
    pub name: String,
    pub ticket: String,
    pub state: State,
    pub log: Vec<String>,
}

/// What the app draws for one familiar.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub state: State,
    pub stage: Phase,
    /// The approved spec's title, once there is one.
    pub plan: Option<String>,
    pub events: Vec<Event>,
    /// What the familiar is waiting on you for.
    pub ask: Option<String>,
    /// The reply that says yes to the ask, when a plain yes answers it.
    pub yes_reply: Option<String>,
    /// A delivery waiting for approval: its approval id.
    pub approval: Option<usize>,
    pub crew: Vec<Crew>,
    pub verify: Option<VerifyReport>,
    pub delivery: Option<DeliveryRecord>,
    pub cost_usd: f64,
    pub tickets_done: usize,
    pub tickets: usize,
}

fn first_line(t: &str, n: usize) -> String {
    let l = t.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if l.chars().count() > n {
        format!("{}…", l.chars().take(n).collect::<String>())
    } else {
        l.to_string()
    }
}

fn agent_state(s: AgentStatus) -> State {
    match s {
        AgentStatus::Approved => State::Done,
        AgentStatus::Failed => State::Failed,
        AgentStatus::AwaitingApproval => State::NeedsYou,
        _ => State::Running,
    }
}

/// The familiar as it stands: harness state plus the project's phase,
/// verification and delivery files under `root`.
pub fn view(rec: &Record, s: &ProjectState, root: &Path) -> View {
    let phase = PhaseState::load(root);
    let verify = deliver::last_verify(root);
    let delivery = deliver::last_delivery(root);
    let main = s.agents.first();
    let main_status = main.map_or(AgentStatus::Idle, |m| m.status);

    let mut events: Vec<Event> = main
        .map(|m| {
            m.log
                .iter()
                .filter(|e| matches!(e.kind, LogKind::System | LogKind::Assistant | LogKind::Error))
                .map(|e| Event {
                    kind: match e.kind {
                        LogKind::Error => EventKind::Alert,
                        LogKind::Assistant => EventKind::Note,
                        _ => EventKind::Agent,
                    },
                    text: first_line(&e.text, 160),
                })
                .filter(|e| !e.text.is_empty())
                .collect()
        })
        .unwrap_or_default();
    for g in &phase.gates {
        if g.passed {
            events.push(Event { kind: EventKind::Done, text: format!("{:?} passed: {}", g.phase.stage(), first_line(&g.note, 120)) });
        }
    }
    if events.len() > EVENTS {
        events.drain(..events.len() - EVENTS);
    }

    let approval = s.pending_approvals().next();
    let (ask, yes_reply, approval_id) = if s.planning.awaiting_commands {
        (Some("The plan wants to run its baseline commands. Reply yes to let them run.".to_string()), Some("yes".to_string()), None)
    } else if s.planning.awaiting_yes {
        (Some(format!("The spec is ready ({}). Reply yes to approve it, or say what to change.", crate::planning::SPEC)), Some("yes".to_string()), None)
    } else if s.rampart.awaiting_yes {
        (Some("Rampart sealed a test suite and waits for your spot-check. Reply yes to seal it.".to_string()), Some("yes".to_string()), None)
    } else if let Some(a) = approval {
        let who = s.agents.get(a.agent).map_or("an agent".to_string(), |x| x.key.clone());
        (Some(format!("Approve {who}'s delivery: {}", first_line(&a.deliverable.summary, 200))), None, Some(a.id))
    } else if phase.phase == Phase::Test && verify.as_ref().is_some_and(|r| !r.passed) && main_status == AgentStatus::Approved {
        let r = verify.as_ref().unwrap();
        (Some(format!("Verify didn't pass, so nothing shipped: {}", r.issues.iter().chain(&r.weakened).cloned().collect::<Vec<_>>().join("; "))), None, None)
    } else if main_status == AgentStatus::Idle && main.and_then(|m| m.log.last()).is_some_and(|e| e.kind == LogKind::Assistant) {
        (main.and_then(|m| m.log.last()).map(|e| first_line(&e.text, 300)), None, None)
    } else {
        (None, None, None)
    };

    let state = if phase.phase == Phase::Done {
        State::Done
    } else if main_status == AgentStatus::Failed {
        State::Failed
    } else if ask.is_some() {
        State::NeedsYou
    } else {
        State::Running
    };

    let crew = s
        .agents
        .iter()
        .skip(1)
        .map(|a| Crew {
            name: if a.title.trim().is_empty() { a.key.clone() } else { format!("{} {}", a.key, a.title) },
            ticket: a.ticket.clone().unwrap_or_else(|| first_line(&a.brief, 120)),
            state: agent_state(a.status),
            log: a.log.iter().rev().take(12).rev().map(|e| first_line(&e.text, 160)).filter(|t| !t.is_empty()).collect(),
        })
        .collect();
    let plan = crate::deliver::load_plan(root).map(|p| p.title).filter(|t| !t.trim().is_empty());
    let tickets_done = s.tickets.iter().filter(|t| matches!(t.state, crate::ticket::TicketState::Done | crate::ticket::TicketState::Wontfix)).count();

    View {
        id: rec.id.clone(),
        name: rec.name.clone(),
        goal: rec.goal.clone(),
        state,
        stage: phase.phase,
        plan,
        events,
        ask,
        yes_reply,
        approval: approval_id,
        crew,
        verify,
        delivery,
        cost_usd: s.total_cost_usd,
        tickets_done,
        tickets: s.tickets.len(),
    }
}

/// The step the app takes on every change: once the main agent is done,
/// carry the run through Verify, Ship and Learn. None when there is nothing
/// to do: still running, already done, or a refused Verify on the same commit
/// (it would only refuse again; a new commit or your reply moves it).
pub fn drive(root: &Path, main: AgentStatus, tickets: &[TicketFacts], target: Target, learn: &mut dyn FnMut() -> Result<String>) -> Result<Option<Vec<Step>>> {
    if main != AgentStatus::Approved {
        return Ok(None);
    }
    let phase = PhaseState::load(root).phase;
    match phase {
        Phase::Specify | Phase::Plan | Phase::Done => return Ok(None),
        Phase::Test => {
            if let Some(r) = deliver::last_verify(root) {
                let head = std::process::Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .current_dir(deliver::run_dir(root))
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                if !r.passed && head.as_deref() == Some(r.head.as_str()) {
                    return Ok(None);
                }
            }
        }
        _ => {}
    }
    deliver::advance_rules(root, tickets, &deliver::ShipOpts { target, gh: None }, learn).map(Some)
}

/// Learn on rules alone: the main agent's notes go to the Librarian's
/// memory commit; on failure they go back to the agent for the next try.
pub fn learn_with_rules(h: &crate::Harness, memory: &crate::memory::Memory, data: &Path) -> Result<String> {
    let notes = h.take_notes();
    let fates = crate::stages::rule_fates(&notes);
    match crate::stages::file(memory, data, &h.root().display().to_string(), &notes, &fates) {
        Ok((dropped, Some(r))) => Ok(format!("Librarian: kept {}, merged {}, dropped {}", r.kept.len(), r.merged.len(), r.dropped.len().max(dropped.len()))),
        Ok((dropped, None)) => Ok(format!("Librarian: nothing to file ({} dropped)", dropped.len())),
        Err(e) => {
            h.return_notes(notes);
            Err(e)
        }
    }
}
