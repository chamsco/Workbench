//! Delegation: the tasks waiting on the fleet, and which bot each one goes to.
//!
//! The Delegate sheet in Chat's Agents side reads this and edits it by drag: a
//! card moved onto a bot is a human attribution, and `auto` lets a rule fill
//! the rest. Every task keeps the line that says why it went where it did, and
//! the snapshot carries each bot's model and per-minute price beside its queue,
//! so the routing is legible before it costs anything.
//!
//! This is deliberately not the harness's ticket scheduler: a task here is
//! work you are planning to hand a bot, not a branch and a worktree. Tickets
//! from the open project can be imported (`from_tickets`), so the plan's work
//! can be delegated the same way without a second list to maintain.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::agents::Agent;
use crate::chat::{new_id, now_ms, Route, RouteKind};
use crate::config::Config;
use crate::ticket::Ticket;

/// The board's file, under the app's data directory.
pub const FILE: &str = "delegation.json";

/// What a minute of an agent's work is assumed to cost, in tokens: enough
/// reading to keep a turn going, and a fraction of that written back. The
/// sheet labels the result "~" because it is an estimate, not a meter.
pub const IN_TOKENS_PER_MIN: f64 = 4_000.0;
pub const OUT_TOKENS_PER_MIN: f64 = 1_200.0;

pub const DEFAULT_EST_MIN: u32 = 10;
const MAX_EST_MIN: u32 = 480;

/// One piece of work waiting to be delegated.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Task {
    #[serde(default)]
    pub id: String,
    pub title: String,
    /// The ticket body or a note, when there is one.
    #[serde(default)]
    pub detail: String,
    /// Minutes it is expected to take; what the queue and its price are built
    /// from.
    #[serde(default = "default_est")]
    pub est_min: u32,
    /// The bot it is delegated to; None is Unassigned.
    #[serde(default)]
    pub agent: Option<String>,
    /// "auto", "manual" or "" (unassigned).
    #[serde(default)]
    pub source: String,
    /// One line saying why it went where it did.
    #[serde(default)]
    pub note: String,
    /// A Workbench ticket key, when it came from the plan; "" otherwise.
    #[serde(default)]
    pub ticket: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
}

fn default_est() -> u32 {
    DEFAULT_EST_MIN
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Board {
    /// Fill unassigned tasks automatically.
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub tasks: Vec<Task>,
}

/// One bot, its queue and what that queue would cost.
#[derive(Serialize, Clone, Debug)]
pub struct AgentLoad {
    pub id: String,
    pub name: String,
    /// The first line of its job, for the row.
    pub job: String,
    pub emoji: String,
    pub color: String,
    /// "claude-sonnet-5-5", "codex", …
    pub model: String,
    pub provider: String,
    /// Rough dollars a minute; 0 for a subscription CLI and for a model the
    /// config does not know.
    pub usd_per_min: f64,
    /// A CLI agent costs nothing at the margin, so the sheet says "Free".
    pub free: bool,
    pub queued_min: u32,
    /// `queued_min` at `usd_per_min`.
    pub cost_usd: f64,
    /// When its queue would be done, unix ms; 0 when nothing is queued.
    pub busy_until: u64,
    pub tasks: Vec<Task>,
}

/// Everything the Delegate sheet draws, computed once on the core side.
#[derive(Serialize, Clone, Debug)]
pub struct Snapshot {
    pub auto: bool,
    pub unassigned: Vec<Task>,
    pub agents: Vec<AgentLoad>,
    /// Every agent's queue, added up.
    pub queues_usd: f64,
    pub queued_min: u32,
}

/// The board on disk. One file, replaced atomically.
pub struct Delegation {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Delegation {
    /// `dir` is the app's data directory (`Prefs::data_dir`).
    pub fn open(dir: impl AsRef<Path>) -> Self {
        Self {
            path: dir.as_ref().join(FILE),
            lock: Mutex::new(()),
        }
    }

    pub fn load(&self) -> Board {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, board: &Board) -> Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(board)?)?;
        std::fs::rename(tmp, &self.path)?;
        Ok(())
    }

    /// Add a task to the end of the queue.
    pub fn add(&self, title: &str, est_min: Option<u32>) -> Result<Board> {
        let title = clean_title(title)?;
        let mut board = self.load();
        let now = now_ms();
        board.tasks.push(Task {
            id: new_id(),
            title,
            detail: String::new(),
            est_min: est_min.unwrap_or(DEFAULT_EST_MIN).clamp(1, MAX_EST_MIN),
            agent: None,
            source: String::new(),
            note: String::new(),
            ticket: String::new(),
            created: now,
            updated: now,
        });
        self.save(&board)?;
        Ok(board)
    }

    /// Change a task's title or estimate. A key that is not on the board is an
    /// error, not a silent no-op: the sheet would otherwise look like it saved.
    pub fn update(&self, id: &str, title: &str, est_min: Option<u32>) -> Result<Board> {
        let title = clean_title(title)?;
        let mut board = self.load();
        let task = board
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("no such task"))?;
        task.title = title;
        if let Some(min) = est_min {
            task.est_min = min.clamp(1, MAX_EST_MIN);
        }
        task.updated = now_ms();
        self.save(&board)?;
        Ok(board)
    }

    pub fn delete(&self, id: &str) -> Result<Board> {
        let mut board = self.load();
        board.tasks.retain(|t| t.id != id);
        self.save(&board)?;
        Ok(board)
    }

    /// Put a task on a bot, or (None) back in Unassigned. A manual move wins
    /// over `auto` until the next time you turn auto on.
    pub fn assign(&self, id: &str, agent: Option<&str>) -> Result<Board> {
        let mut board = self.load();
        let task = board
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("no such task"))?;
        match agent.map(str::trim).filter(|a| !a.is_empty()) {
            Some(a) => {
                task.agent = Some(a.to_string());
                task.source = "manual".into();
                task.note = "you moved it".into();
            }
            None => {
                task.agent = None;
                task.source = String::new();
                task.note = String::new();
            }
        }
        task.updated = now_ms();
        self.save(&board)?;
        Ok(board)
    }

    /// Turn automatic attribution on (and fill what is waiting) or off.
    pub fn set_auto(&self, on: bool) -> Result<Board> {
        let mut board = self.load();
        board.auto = on;
        self.save(&board)?;
        Ok(board)
    }

    /// Turn the open project's unsettled tickets into tasks, once each: a task
    /// remembers the ticket key it came from, so importing again does not
    /// duplicate a ticket already on the board.
    pub fn from_tickets(&self, tickets: &[Ticket]) -> Result<Board> {
        let mut board = self.load();
        let seen: BTreeSet<String> = board
            .tasks
            .iter()
            .map(|t| t.ticket.clone())
            .filter(|k| !k.is_empty())
            .collect();
        let now = now_ms();
        for t in tickets.iter().filter(|t| !t.state.is_settled() && !seen.contains(t.key.as_str())) {
            let est = (8 + 4 * t.acceptance.len() as u32).clamp(5, 120);
            board.tasks.push(Task {
                id: new_id(),
                title: t.title.trim().to_string(),
                detail: t.what_to_build.chars().take(400).collect(),
                est_min: est,
                agent: None,
                source: String::new(),
                note: String::new(),
                ticket: t.key.clone(),
                created: now,
                updated: now,
            });
        }
        self.save(&board)?;
        Ok(board)
    }
}

fn clean_title(title: &str) -> Result<String> {
    let title = title.trim().chars().take(200).collect::<String>();
    if title.is_empty() {
        bail!("a task needs a title");
    }
    Ok(title)
}

/// A minute of this route, in dollars. A subscription CLI is free at the
/// margin; an API model is priced from the config; a model the config does not
/// know reads as free rather than guessing a number.
pub fn usd_per_min(cfg: &Config, route: &Route) -> f64 {
    if route.kind == RouteKind::Cli {
        return 0.0;
    }
    route
        .model
        .as_deref()
        .and_then(|id| cfg.model(id))
        .map(|m| {
            (m.input_usd_per_mtok * IN_TOKENS_PER_MIN + m.output_usd_per_mtok * OUT_TOKENS_PER_MIN) / 1_000_000.0
        })
        .unwrap_or(0.0)
}

/// The model name the sheet shows; a CLI with no model of its own shows the
/// CLI.
pub fn model_name(route: &Route) -> String {
    route
        .model
        .clone()
        .unwrap_or_else(|| route.provider.clone())
}

fn is_free(route: &Route) -> bool {
    route.kind == RouteKind::Cli
}

/// Fill every unassigned task, in the order it was added, with the bot whose
/// queue is shortest; ties go to the cheaper minute, then the older bot.
///
/// The rule is small on purpose: it is visible in a sentence, and the note on
/// each task says which bot won and why, so a human can disagree with it and
/// drag the card.
pub fn auto_assign(board: &mut Board, agents: &[Agent], cfg: &Config) {
    if !board.auto {
        return;
    }
    let known: BTreeSet<String> = agents.iter().filter(|a| !a.id.is_empty()).map(|a| a.id.clone()).collect();
    // What every bot already has, including manual work.
    let mut load: BTreeMap<String, u32> = BTreeMap::new();
    for t in &board.tasks {
        if let Some(a) = t.agent.as_deref().filter(|a| known.contains(*a)) {
            *load.entry(a.to_string()).or_default() += t.est_min;
        }
    }
    let mut queue: Vec<usize> = board
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.agent.as_deref().map_or(true, |a| !known.contains(a)))
        .map(|(i, _)| i)
        .collect();
    queue.sort_by_key(|&i| board.tasks[i].created);
    for i in queue {
        let pick = agents
            .iter()
            .filter(|a| !a.id.is_empty())
            .min_by(|a, b| {
                let ka = (
                    load.get(&a.id).copied().unwrap_or(0),
                    usd_per_min(cfg, &a.route),
                    a.created,
                );
                let kb = (
                    load.get(&b.id).copied().unwrap_or(0),
                    usd_per_min(cfg, &b.route),
                    b.created,
                );
                ka.0.cmp(&kb.0)
                    .then(ka.1.total_cmp(&kb.1))
                    .then(ka.2.cmp(&kb.2))
                    .then_with(|| a.id.cmp(&b.id))
            })
            .cloned();
        let Some(a) = pick else { return };
        let ahead = load.entry(a.id.clone()).or_default();
        let task = &mut board.tasks[i];
        task.agent = Some(a.id.clone());
        task.source = "auto".into();
        task.note = format!("auto · {} had the shortest queue ({ahead} min)", a.name);
        task.updated = now_ms();
        *ahead += task.est_min;
    }
}

/// The sheet's data: the board, plus every bot's queue and price.
///
/// A task whose bot was deleted reads as Unassigned with a note, rather than
/// vanishing with it.
pub fn snapshot(board: &Board, agents: &[Agent], cfg: &Config) -> Snapshot {
    let now = now_ms();
    let known: BTreeSet<&str> = agents.iter().map(|a| a.id.as_str()).collect();
    let loads: Vec<AgentLoad> = agents
        .iter()
        .filter(|a| !a.id.is_empty())
        .map(|a| {
            let tasks: Vec<Task> = board
                .tasks
                .iter()
                .filter(|t| t.agent.as_deref() == Some(a.id.as_str()))
                .cloned()
                .collect();
            let queued_min: u32 = tasks.iter().map(|t| t.est_min).sum();
            let usd = usd_per_min(cfg, &a.route);
            AgentLoad {
                id: a.id.clone(),
                name: a.name.clone(),
                job: a.job.split('\n').find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string(),
                emoji: a.avatar.emoji.clone(),
                color: a.avatar.color.clone(),
                model: model_name(&a.route),
                provider: a.route.provider.clone(),
                usd_per_min: usd,
                free: is_free(&a.route),
                queued_min,
                cost_usd: queued_min as f64 * usd,
                busy_until: if queued_min > 0 { now + u64::from(queued_min) * 60_000 } else { 0 },
                tasks,
            }
        })
        .collect();
    let unassigned: Vec<Task> = board
        .tasks
        .iter()
        .filter(|t| t.agent.as_deref().map_or(true, |a| !known.contains(a)))
        .cloned()
        .map(|mut t| {
            if t.agent.is_some() {
                t.agent = None;
                t.note = "its bot is gone".into();
            }
            t
        })
        .collect();
    Snapshot {
        auto: board.auto,
        unassigned,
        queues_usd: loads.iter().map(|l| l.cost_usd).sum(),
        queued_min: board.tasks.iter().map(|t| t.est_min).sum(),
        agents: loads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_CONFIG;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("delegate-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn bot(id: &str, name: &str, model: &str) -> Agent {
        Agent {
            id: id.into(),
            name: name.into(),
            job: format!("{name}'s job\n\nmore"),
            avatar: crate::agents::Avatar {
                emoji: "🙂".into(),
                color: "#64748b".into(),
                ..Default::default()
            },
            personality: String::new(),
            route: Route {
                kind: RouteKind::Router,
                provider: "openrouter".into(),
                model: Some(model.into()),
            },
            shared: vec![],
            computer: Default::default(),
            memory: true,
            off: vec![],
            apps_off: vec![],
            hook: String::new(),
            routines: vec![],
            created: 1,
            updated: 1,
        }
    }

    fn cli_bot(id: &str, name: &str) -> Agent {
        let mut a = bot(id, name, "claude-code");
        a.route = Route {
            kind: RouteKind::Cli,
            provider: "claude".into(),
            model: None,
        };
        a
    }

    #[test]
    fn adds_assigns_and_survives_a_reopen() {
        let dir = tmpdir("roundtrip");
        let d = Delegation::open(&dir);
        let board = d.add("Update the pricing page", Some(10)).unwrap();
        assert_eq!(board.tasks.len(), 1);
        let id = board.tasks[0].id.clone();

        let board = d.assign(&id, Some("bot-a")).unwrap();
        assert_eq!(board.tasks[0].agent.as_deref(), Some("bot-a"));
        assert_eq!(board.tasks[0].source, "manual");

        // A fresh store reads the same board back.
        let again = Delegation::open(&dir).load();
        assert_eq!(again.tasks[0].title, "Update the pricing page");
        assert_eq!(again.tasks[0].agent.as_deref(), Some("bot-a"));

        let board = d.assign(&id, None).unwrap();
        assert!(board.tasks[0].agent.is_none());
        d.delete(&id).unwrap();
        assert!(Delegation::open(&dir).load().tasks.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_title_is_refused() {
        let dir = tmpdir("empty");
        let d = Delegation::open(&dir);
        assert!(d.add("   ", None).is_err());
        assert!(d.update("nope", "x", None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_picks_the_shortest_queue_then_the_cheaper_minute() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        let dir = tmpdir("auto");
        let d = Delegation::open(&dir);
        d.add("first", Some(10)).unwrap();
        d.add("second", Some(10)).unwrap();
        d.add("third", Some(10)).unwrap();
        let board = d.set_auto(true).unwrap();

        // Both free at the start, so the cheaper model takes the first task.
        let cheap = bot("bot-b", "Cheap", "claude-haiku-4-5");
        let dear = bot("bot-a", "Dear", "claude-opus-5-5");
        let agents = vec![dear.clone(), cheap.clone()];
        let mut board = board;
        auto_assign(&mut board, &agents, &cfg);
        let first = board.tasks.iter().find(|t| t.title == "first").unwrap();
        assert_eq!(first.agent.as_deref(), Some("bot-b"), "haiku is the cheaper free minute");
        assert!(first.note.starts_with("auto · "));
        assert_eq!(first.source, "auto");

        // With bot-b holding 10 minutes, the next one goes to the empty queue.
        let second = board.tasks.iter().find(|t| t.title == "second").unwrap();
        assert_eq!(second.agent.as_deref(), Some("bot-a"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_off_leaves_work_unassigned() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        let dir = tmpdir("manual");
        let d = Delegation::open(&dir);
        d.add("waits", Some(5)).unwrap();
        let mut board = d.load();
        auto_assign(&mut board, &[bot("b", "B", "claude-haiku-4-5")], &cfg);
        assert!(board.tasks[0].agent.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_snapshot_prices_a_queue_and_a_cli_is_free() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        let dir = tmpdir("snap");
        let d = Delegation::open(&dir);
        d.add("one", Some(30)).unwrap();
        d.add("two", Some(30)).unwrap();
        let mut board = d.load();
        let haiku = bot("b1", "Cheap", "claude-haiku-4-5");
        let codex = cli_bot("b2", "Codex", );
        // Both tasks onto the API bot, by hand.
        for t in board.tasks.iter_mut() {
            t.agent = Some("b1".into());
        }
        let snap = snapshot(&board, &[haiku.clone(), codex], &cfg);
        let b1 = snap.agents.iter().find(|a| a.id == "b1").unwrap();
        assert_eq!(b1.queued_min, 60);
        // $1 in / $5 out a million, at 4k in + 1.2k out a minute.
        let expected = (1.0 * 4_000.0 + 5.0 * 1_200.0) / 1_000_000.0;
        assert!((b1.usd_per_min - expected).abs() < 1e-9);
        assert!((b1.cost_usd - expected * 60.0).abs() < 1e-9);
        assert!(b1.busy_until > now_ms());
        let b2 = snap.agents.iter().find(|a| a.id == "b2").unwrap();
        assert!(b2.free && b2.usd_per_min == 0.0 && b2.busy_until == 0);
        assert_eq!(snap.unassigned.len(), 0);
        assert!((snap.queues_usd - b1.cost_usd).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_whose_bot_is_gone_comes_back_unassigned() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        let dir = tmpdir("gone");
        let d = Delegation::open(&dir);
        let board = d.add("orphan", Some(5)).unwrap();
        let mut board = board;
        board.tasks[0].agent = Some("deleted-bot".into());
        let snap = snapshot(&board, &[], &cfg);
        assert_eq!(snap.unassigned.len(), 1);
        assert_eq!(snap.unassigned[0].note, "its bot is gone");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn importing_tickets_twice_keeps_one_task_each() {
        let dir = tmpdir("tickets");
        let d = Delegation::open(&dir);
        let ticket = Ticket {
            num: 1,
            key: "t1".into(),
            title: "Update the pricing page".into(),
            category: crate::ticket::Category::Task,
            state: crate::ticket::TicketState::ReadyForAgent,
            what_to_build: "Change the numbers.".into(),
            acceptance: vec!["A1".into(), "A2".into()],
            check: None,
            out_of_scope: vec![],
            blocked_by: vec![],
            owner: 0,
            assignee: None,
            budget_usd: None,
            notes: vec![],
            covers: vec![],
        };
        let first = d.from_tickets(std::slice::from_ref(&ticket)).unwrap();
        assert_eq!(first.tasks.len(), 1);
        assert_eq!(first.tasks[0].ticket, "t1");
        assert_eq!(first.tasks[0].est_min, 16, "8 + 4 per acceptance line");
        let second = d.from_tickets(&[ticket]).unwrap();
        assert_eq!(second.tasks.len(), 1, "a second import is a no-op");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
