//! One project = one main agent you talk to, plus a tree of agents working
//! tickets under it.
//!
//! Flow: the main agent grills you, turns the goal into tickets (you approve
//! the plan), then dispatches them. Each ticket agent works on its own git
//! branch in its own worktree, starts cheap, and climbs the model/effort
//! ladder only on evidence of failure. A ticket's `check` must pass before
//! anyone reviews it; accepted work is merged into the parent's branch.
//! Deliverables from the top `human_review_depth` levels come to you; deeper
//! ones go to the agent that planned them. Agents file out-of-scope findings
//! as tickets, which a cheap triage agent classifies.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures::future::{join_all, BoxFuture, FutureExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, watch, Semaphore};

use crate::config::{Config, Isolation};
use crate::effort::Effort;
use crate::git;
use crate::project::*;
use crate::provider::{self, Block, Message, Role, Stop, ToolDef};
use crate::router::{truncate, AgentRole, Decision, Router};
use crate::skills::{Skills, ADAPTER};
use crate::ticket::{Ticket, TicketState};
use crate::tools::{file_tools, Workspace};

const LOG_CAP: usize = 4000;

enum Verdict {
    Approved,
    Rejected(String),
}

struct Inner {
    cfg: Config,
    http: reqwest::Client,
    router: Router,
    root: PathBuf,
    /// Branch checked out in the root workspace; the main agent's branch.
    base_branch: String,
    agents_md: Option<String>,
    skills: Skills,
    state: Mutex<ProjectState>,
    verdicts: Mutex<HashMap<usize, oneshot::Sender<Verdict>>>,
    /// Per ticket key: flips to true once the ticket settles.
    done: Mutex<HashMap<String, watch::Receiver<bool>>>,
    /// Finished conversations, kept so a reviewer can reopen them.
    parked: Mutex<HashMap<AgentId, Conversation>>,
    limiter: Semaphore,
    /// Worktrees share one ref store; git runs one command sequence at a time.
    git: tokio::sync::Mutex<()>,
    changed: async_channel::Sender<()>,
}

pub struct Harness {
    inner: Arc<Inner>,
    to_main: mpsc::UnboundedSender<String>,
    changed: async_channel::Receiver<()>,
    rt: tokio::runtime::Runtime,
}

fn new_record(
    id: AgentId,
    kind: AgentKind,
    parent: Option<AgentId>,
    depth: usize,
    key: String,
    title: String,
) -> AgentRecord {
    AgentRecord {
        id,
        role: if kind == AgentKind::Main {
            AgentRole::Main
        } else {
            AgentRole::Sub
        },
        kind,
        parent,
        depth,
        key,
        title,
        brief: String::new(),
        depends_on: vec![],
        status: AgentStatus::Queued,
        decision: None,
        log: vec![],
        cost_usd: 0.0,
        input_tokens: 0,
        output_tokens: 0,
        deliverable: None,
        budget_usd: None,
        ticket: None,
        branch: None,
        worktree: None,
        escalations: vec![],
    }
}

impl Harness {
    /// Open (or create) a project rooted at `workspace`. Owns its own tokio
    /// runtime so any UI toolkit can drive it through plain sync calls.
    pub fn open(workspace: PathBuf) -> Result<Harness> {
        std::fs::create_dir_all(&workspace)?;
        let root = workspace.canonicalize()?;
        let (cfg, config_source) = Config::load(&root)?;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        let (git_note, base_branch) = if cfg.orchestrator.isolation == Isolation::Worktree {
            rt.block_on(async {
                let note = git::ensure_repo(&root).await?;
                anyhow::Ok((note, git::current_branch(&root).await?))
            })
            .context("setting up git for worktree isolation (set isolation = \"shared\" to skip)")?
        } else {
            (None, String::new())
        };

        let http = reqwest::Client::builder().build()?;
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into());
        let mut main = new_record(
            MAIN,
            AgentKind::Main,
            None,
            0,
            "main".into(),
            "Main agent".into(),
        );
        main.status = AgentStatus::Idle;
        main.branch = (!base_branch.is_empty()).then(|| base_branch.clone());
        main.worktree = Some(root.clone());
        if let Some(n) = git_note {
            main.log.push(LogEntry {
                kind: LogKind::System,
                text: n,
            });
        }

        let (changed_tx, changed_rx) = async_channel::bounded(1);
        let inner = Arc::new(Inner {
            router: Router::new(http.clone(), cfg.clone()),
            agents_md: std::fs::read_to_string(root.join("AGENTS.md")).ok(),
            skills: Skills::load(&root),
            limiter: Semaphore::new(cfg.orchestrator.max_parallel_calls),
            state: Mutex::new(ProjectState {
                name,
                workspace: root.clone(),
                agents: vec![main],
                approvals: vec![],
                tickets: vec![],
                total_cost_usd: 0.0,
                router_cost_usd: 0.0,
                config_source,
            }),
            verdicts: Mutex::new(HashMap::new()),
            done: Mutex::new(HashMap::new()),
            parked: Mutex::new(HashMap::new()),
            git: tokio::sync::Mutex::new(()),
            changed: changed_tx,
            base_branch,
            root,
            cfg,
            http,
        });

        let (to_main, inbox) = mpsc::unbounded_channel();
        rt.spawn(main_loop(inner.clone(), inbox));
        Ok(Harness {
            inner,
            to_main,
            changed: changed_rx,
            rt,
        })
    }

    /// Talk to the main agent. The first message is the project goal.
    pub fn send(&self, text: impl Into<String>) {
        let _ = self.to_main.send(text.into());
    }

    pub fn approve(&self, approval: usize) {
        self.resolve(approval, Verdict::Approved);
    }

    pub fn reject(&self, approval: usize, feedback: impl Into<String>) {
        self.resolve(approval, Verdict::Rejected(feedback.into()));
    }

    /// File a ticket as the human. It is triaged, then the main agent can
    /// schedule it with work_tickets.
    pub fn file_ticket(&self, title: &str, description: &str) -> Result<String> {
        let input = json!({"title": title, "what_to_build": description});
        let inner = self.inner.clone();
        self.rt
            .block_on(async move { file_ticket(&inner, MAIN, MAIN, &input) })
    }

    pub fn snapshot(&self) -> ProjectState {
        self.inner.state.lock().unwrap().clone()
    }

    /// Fires (coalesced) whenever state changes. Runtime-agnostic.
    pub fn changes(&self) -> async_channel::Receiver<()> {
        self.changed.clone()
    }

    pub fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        self.rt.block_on(f)
    }

    fn resolve(&self, approval: usize, verdict: Verdict) {
        if let Some(tx) = self.inner.verdicts.lock().unwrap().remove(&approval) {
            let _ = tx.send(verdict);
        }
    }
}

impl Inner {
    fn update<R>(&self, f: impl FnOnce(&mut ProjectState) -> R) -> R {
        let r = f(&mut self.state.lock().unwrap());
        let _ = self.changed.try_send(());
        r
    }

    fn agent<R>(&self, id: AgentId, f: impl FnOnce(&mut AgentRecord) -> R) -> R {
        self.update(|s| f(&mut s.agents[id]))
    }

    fn ticket<R>(&self, key: &str, f: impl FnOnce(&mut Ticket) -> R) -> Option<R> {
        let (r, t) = self.update(|s| {
            let t = s.ticket_mut(key)?;
            let r = f(t);
            Some((r, t.clone()))
        })?;
        t.write(&self.root);
        Some(r)
    }

    fn log(&self, id: AgentId, kind: LogKind, text: impl Into<String>) {
        let text = text.into();
        let text = if text.len() > LOG_CAP {
            format!("{}…", truncate(&text, LOG_CAP))
        } else {
            text
        };
        self.agent(id, |a| a.log.push(LogEntry { kind, text }));
    }

    fn set_status(&self, id: AgentId, status: AgentStatus) {
        self.agent(id, |a| a.status = status);
        self.save();
    }

    /// Record of the run, not a resumable checkpoint.
    fn save(&self) {
        let snap = self.state.lock().unwrap().clone();
        let dir = snap.workspace.join(".backspace");
        if std::fs::create_dir_all(&dir).is_ok() {
            if let Ok(json) = serde_json::to_string_pretty(&snap) {
                let _ = std::fs::write(dir.join("state.json"), json);
            }
        }
    }

    fn isolated(&self) -> bool {
        self.cfg.orchestrator.isolation == Isolation::Worktree
    }

    fn can_spawn(&self, depth: usize) -> bool {
        depth < self.cfg.orchestrator.max_depth
    }

    fn workspace(&self, dir: &Path) -> Workspace {
        Workspace {
            root: dir.to_path_buf(),
            bash_timeout: Duration::from_secs(self.cfg.orchestrator.bash_timeout_secs),
        }
    }

    /// Where an agent's work lives, and the branch it is on.
    fn home(&self, id: AgentId) -> (PathBuf, String) {
        let s = self.state.lock().unwrap();
        let a = &s.agents[id];
        (
            a.worktree.clone().unwrap_or_else(|| self.root.clone()),
            a.branch.clone().unwrap_or_else(|| self.base_branch.clone()),
        )
    }

    fn system_prompt(&self, kind: AgentKind, depth: usize, decision: &Decision) -> String {
        let remaining = self
            .cfg
            .orchestrator
            .max_subagents
            .saturating_sub(self.state.lock().unwrap().subagent_count());
        let wf = &self.cfg.workflow;
        let mut s = match kind {
            AgentKind::Main => format!(
                include_str!("prompts/main.md"),
                remaining = remaining,
                grill = if wf.grill {
                    "Before planning, grill the user following the `grilling` skill: rounds of numbered questions, each with your recommended answer, so they can reply \"go\" to accept all. Stop as soon as the frontier is empty; do not re-ask what the goal already answers."
                } else {
                    "Do not interview the user; make reasonable decisions and state them in PLAN.md."
                },
                plan = if wf.plan_approval && !self.cfg.orchestrator.auto_approve {
                    "create_tickets blocks until the user approves the batch; if they reject it, revise and call it again."
                } else {
                    "Tickets are ready as soon as you create them."
                },
            ),
            AgentKind::Worker => include_str!("prompts/worker.md").to_string(),
            AgentKind::Triage => include_str!("prompts/triage.md").to_string(),
        };
        if kind == AgentKind::Worker && self.can_spawn(depth) {
            s.push_str("\n\n");
            s.push_str(&format!(
                include_str!("prompts/manager.md"),
                remaining = remaining,
                depth = depth,
                max_depth = self.cfg.orchestrator.max_depth
            ));
        }
        s.push_str(&format!(
            "\n\nYou are running on {} at {} effort.\n\n# Skills\n\nLoad any of these with the `skill` tool when it fits the task.\n{}\n\n{ADAPTER}",
            decision.model,
            decision.effort,
            self.skills.index()
        ));
        let injected = match kind {
            AgentKind::Main => &wf.main_skills,
            AgentKind::Worker => &wf.worker_skills,
            AgentKind::Triage => &wf.triage_skills,
        };
        for name in injected {
            if name == "grilling" && !wf.grill {
                continue;
            }
            if let Some(body) = self.skills.read(name, None) {
                s.push_str(&format!("\n\n# Skill: {name}\n\n{body}"));
            }
        }
        if let Some(md) = &self.agents_md {
            s.push_str("\n\n# Project instructions (AGENTS.md)\n\n");
            s.push_str(md);
        }
        s
    }

    /// Workers start at most at the configured caps; escalation lifts them.
    fn cap_start(&self, d: &mut Decision) {
        let esc = &self.cfg.escalation;
        let before = format!("{} @ {}", d.model, d.effort);
        if d.effort > esc.start_max_effort {
            d.effort = esc.start_max_effort;
        }
        let tier = self.cfg.model(&d.model).map(|m| m.tier).unwrap_or(0);
        if tier > esc.start_max_tier && !d.source.contains("pinned") {
            let usable = self.cfg.usable_models();
            if let Some(m) = usable
                .iter()
                .filter(|m| m.tier <= esc.start_max_tier)
                .max_by(|a, b| {
                    a.tier
                        .cmp(&b.tier)
                        .then(b.output_usd_per_mtok.total_cmp(&a.output_usd_per_mtok))
                })
            {
                d.model = m.id.clone();
            }
        }
        let after = format!("{} @ {}", d.model, d.effort);
        if before != after {
            let note =
                format!("router asked for {before}; starting at {after}, escalating on evidence");
            d.note = Some(match d.note.take() {
                Some(n) => format!("{n}\n{note}"),
                None => note,
            });
        }
    }
}

// ------------------------------------------------------------------ agents

enum TurnEnd {
    Replied,
    Delivered(Deliverable),
}

struct Conversation {
    id: AgentId,
    kind: AgentKind,
    ticket: Option<String>,
    can_spawn: bool,
    ws: Workspace,
    initial: Decision,
    decision: Decision,
    system: String,
    messages: Vec<Message>,
    tools: Vec<ToolDef>,
    budget_warned_at: Option<usize>,
    turns_total: usize,
    recent_calls: Vec<String>,
}

fn tools_for(kind: AgentKind, can_spawn: bool) -> Vec<ToolDef> {
    let mut tools: Vec<ToolDef> = file_tools();
    if kind == AgentKind::Triage {
        tools.retain(|t| t.name == "read" || t.name == "bash");
        tools.push(skill_tool());
        tools.push(triage_tool());
        return tools;
    }
    tools.push(skill_tool());
    if can_spawn {
        tools.extend([create_tickets_tool(), work_tickets_tool(), revise_tool()]);
    }
    tools.push(file_ticket_tool());
    tools.push(deliver_tool());
    tools
}

async fn main_loop(inner: Arc<Inner>, mut inbox: mpsc::UnboundedReceiver<String>) {
    let mut convo: Option<Conversation> = None;
    while let Some(text) = inbox.recv().await {
        inner.log(MAIN, LogKind::User, text.clone());
        inner.set_status(MAIN, AgentStatus::Running);

        if convo.is_none() {
            match inner.router.route(AgentRole::Main, &text, "").await {
                Ok(d) => {
                    announce_route(&inner, MAIN, &d);
                    inner.agent(MAIN, |a| a.brief = text.clone());
                    convo = Some(Conversation {
                        id: MAIN,
                        kind: AgentKind::Main,
                        ticket: None,
                        can_spawn: true,
                        ws: inner.workspace(&inner.root),
                        initial: d.clone(),
                        system: inner.system_prompt(AgentKind::Main, 0, &d),
                        decision: d,
                        messages: vec![],
                        tools: tools_for(AgentKind::Main, true),
                        budget_warned_at: None,
                        turns_total: 0,
                        recent_calls: vec![],
                    });
                }
                Err(e) => {
                    inner.log(MAIN, LogKind::Error, format!("{e:#}"));
                    inner.set_status(MAIN, AgentStatus::Failed);
                    continue;
                }
            }
        }
        let c = convo.as_mut().unwrap();
        c.messages.push(Message::user_text(text));
        match drive(&inner, c).await {
            Ok(TurnEnd::Delivered(d)) => {
                inner.agent(MAIN, |a| a.deliverable = Some(d));
                inner.set_status(MAIN, AgentStatus::Approved);
            }
            Ok(TurnEnd::Replied) => inner.set_status(MAIN, AgentStatus::Idle),
            Err(e) => {
                inner.log(MAIN, LogKind::Error, format!("{e:#}"));
                inner.set_status(MAIN, AgentStatus::Idle);
            }
        }
    }
}

fn announce_route(inner: &Inner, id: AgentId, d: &Decision) {
    let mut msg = format!("routed by {} → {} @ {}", d.source, d.model, d.effort);
    if d.confidence > 0.0 {
        msg.push_str(&format!(" ({:.0}% confident)", d.confidence * 100.0));
    }
    if let Some(n) = &d.note {
        msg.push_str(&format!("\n{n}"));
    }
    inner.log(id, LogKind::System, msg);
    inner.update(|s| {
        s.router_cost_usd += d.router_cost_usd;
        s.total_cost_usd += d.router_cost_usd;
        s.agents[id].decision = Some(d.clone());
    });
}

/// One rung up: more effort while the model has headroom below `high`, then
/// the cheapest stronger model, then more effort. Workers only.
fn escalate(inner: &Inner, c: &mut Conversation, reason: &str) -> bool {
    if c.kind != AgentKind::Worker {
        return false;
    }
    let steps = inner.state.lock().unwrap().agents[c.id].escalations.len();
    if steps >= inner.cfg.escalation.max_steps {
        inner.log(
            c.id,
            LogKind::System,
            format!("{reason}; escalation limit ({steps}) reached"),
        );
        return false;
    }
    let Some(spec) = inner.cfg.model(&c.decision.model) else {
        return false;
    };
    let next_effort = Effort::from_index(c.decision.effort.index() + 1);
    let effort_moves = |upto: Effort| {
        c.decision.effort < upto
            && next_effort.clamp_to(&spec.efforts).is_some()
            && next_effort.clamp_to(&spec.efforts) != c.decision.effort.clamp_to(&spec.efforts)
    };
    let stronger = inner
        .cfg
        .usable_models()
        .into_iter()
        .filter(|m| m.tier > spec.tier)
        .min_by(|a, b| {
            a.tier
                .cmp(&b.tier)
                .then(a.output_usd_per_mtok.total_cmp(&b.output_usd_per_mtok))
        })
        .map(|m| m.id.clone());

    let old = format!("{} @ {}", c.decision.model, c.decision.effort);
    if effort_moves(Effort::High) {
        c.decision.effort = next_effort;
    } else if let Some(m) = stronger {
        c.decision.model = m;
        c.decision.effort = c.decision.effort.max(Effort::Medium);
    } else if effort_moves(Effort::Max) {
        c.decision.effort = next_effort;
    } else {
        inner.log(
            c.id,
            LogKind::System,
            format!("{reason}; already at the top of the ladder"),
        );
        return false;
    }
    let line = format!(
        "{old} → {} @ {} ({reason})",
        c.decision.model, c.decision.effort
    );
    inner.log(c.id, LogKind::System, format!("escalated: {line}"));
    let d = c.decision.clone();
    inner.agent(c.id, |a| {
        a.escalations.push(line);
        a.decision = Some(d);
    });
    true
}

/// Drive an agent until it delivers, nudging it if it stops early.
async fn finish(inner: &Arc<Inner>, c: &mut Conversation) -> Result<Deliverable> {
    let tool = if c.kind == AgentKind::Triage {
        "triage_ticket"
    } else {
        "submit_deliverable"
    };
    for _nudge in 0..2 {
        match drive(inner, c).await? {
            TurnEnd::Delivered(d) => return Ok(d),
            TurnEnd::Replied => {
                let nudge = format!(
                    "You ended your turn without calling {tool}. Finish, then call {tool}."
                );
                inner.log(c.id, LogKind::System, nudge.clone());
                c.messages.push(Message::user_text(nudge));
            }
        }
    }
    bail!("agent stopped without calling {tool}")
}

/// `Some(reason)` when the project or any budget covering this agent is spent.
fn over_budget(inner: &Inner, id: AgentId) -> Option<String> {
    let s = inner.state.lock().unwrap();
    if let Some(max) = inner.cfg.orchestrator.max_project_usd {
        if s.total_cost_usd >= max {
            return Some(format!("project budget ${max:.2} spent"));
        }
    }
    s.ancestry(id).into_iter().find_map(|a| {
        let budget = s.agents[a].budget_usd?;
        let spent = s.subtree_cost(a);
        (spent >= budget).then(|| {
            format!(
                "budget of `{}` spent (${spent:.2} of ${budget:.2})",
                s.agents[a].key
            )
        })
    })
}

/// Run the model/tool loop until the agent replies without tools or gets a
/// deliverable accepted.
async fn drive(inner: &Arc<Inner>, c: &mut Conversation) -> Result<TurnEnd> {
    let mut turn = 0usize;
    loop {
        let budget = c.decision.effort.max_turns();
        if turn == budget {
            if escalate(inner, c, "turn budget exhausted") {
                turn = 0;
                continue;
            }
            let msg = "Turn budget for your effort level is exhausted. Wrap up now: submit what you have (or reply) and state what is left undone.";
            inner.log(c.id, LogKind::System, msg);
            c.messages.push(Message::user_text(msg));
        }
        if turn > budget + 3 {
            bail!("exceeded turn budget ({budget})");
        }
        if let Some(reason) = over_budget(inner, c.id) {
            match c.budget_warned_at {
                None => {
                    let msg = format!("{reason}. Stop starting new work: submit what you have now and list what is left undone.");
                    inner.log(c.id, LogKind::System, msg.clone());
                    c.messages.push(Message::user_text(msg));
                    c.budget_warned_at = Some(c.turns_total);
                }
                Some(t) if c.turns_total > t + 2 => bail!("{reason}"),
                Some(_) => {}
            }
        }
        turn += 1;
        c.turns_total += 1;

        let spec = inner
            .cfg
            .model(&c.decision.model)
            .ok_or_else(|| anyhow!("routed to unknown model {}", c.decision.model))?
            .clone();
        let provider_cfg = inner.cfg.providers[&spec.provider].clone();
        let completion = {
            let _permit = inner.limiter.acquire().await?;
            provider::complete(
                &inner.http,
                provider::Request {
                    provider: &provider_cfg,
                    model: &spec,
                    effort: c.decision.effort,
                    system: &c.system,
                    messages: &c.messages,
                    tools: &c.tools,
                },
            )
            .await?
        };

        let cost = spec.cost(completion.input_tokens, completion.output_tokens);
        inner.update(|s| {
            let a = &mut s.agents[c.id];
            a.cost_usd += cost;
            a.input_tokens += completion.input_tokens;
            a.output_tokens += completion.output_tokens;
            s.total_cost_usd += cost;
        });

        let text = completion.text();
        if !text.trim().is_empty() {
            inner.log(c.id, LogKind::Assistant, text.clone());
        }
        c.messages.push(Message {
            role: Role::Assistant,
            content: completion.content.clone(),
        });

        match completion.stop {
            Stop::Refusal => bail!("model refused the request"),
            Stop::ToolUse => {}
            Stop::MaxTokens if !has_tool_use(&completion.content) => {
                c.messages.push(Message::user_text(
                    "Your reply hit the output limit. Continue, in smaller steps.",
                ));
                continue;
            }
            Stop::EndTurn | Stop::MaxTokens => {
                if !has_tool_use(&completion.content) {
                    return Ok(TurnEnd::Replied);
                }
            }
        }

        let calls: Vec<(String, String, Value)> = completion
            .content
            .iter()
            .filter_map(|b| match b {
                Block::ToolUse { id, name, input } => {
                    Some((id.clone(), name.clone(), input.clone()))
                }
                _ => None,
            })
            .collect();

        let results = {
            let cref = &*c;
            join_all(calls.iter().map(|(_, name, input)| {
                inner.log(
                    cref.id,
                    LogKind::ToolCall,
                    format!("{name} {}", truncate(&input.to_string(), 400)),
                );
                run_tool(inner, cref, name, input)
            }))
            .await
        };

        let mut blocks = Vec::new();
        let mut delivered = None;
        let mut failures = Vec::new();
        for ((call_id, _, _), res) in calls.iter().zip(results) {
            let (content, is_error) = match res {
                Ok(ToolOutcome::Text(t)) => (t, false),
                Ok(ToolOutcome::Failure(t, reason)) => {
                    failures.push(reason);
                    (t, false)
                }
                Ok(ToolOutcome::Delivered(d, note)) => {
                    delivered = Some(d);
                    (note, false)
                }
                Err(e) => (format!("error: {e:#}"), true),
            };
            inner.log(
                c.id,
                if is_error {
                    LogKind::Error
                } else {
                    LogKind::ToolResult
                },
                content.clone(),
            );
            blocks.push(Block::ToolResult {
                tool_use_id: call_id.clone(),
                content,
                is_error,
            });
        }
        c.messages.push(Message {
            role: Role::User,
            content: blocks,
        });
        if let Some(d) = delivered {
            return Ok(TurnEnd::Delivered(d));
        }

        // Loop detection: the same call N times in a row is a stuck agent.
        for (_, name, input) in &calls {
            c.recent_calls.push(format!("{name}{input}"));
        }
        let n = inner.cfg.escalation.loop_repeats.max(2);
        if c.recent_calls.len() >= n {
            let tail = &c.recent_calls[c.recent_calls.len() - n..];
            if tail.iter().all(|x| x == &tail[0]) {
                c.recent_calls.clear();
                failures.push("repeated the same tool call".into());
                c.messages.push(Message::user_text(format!(
                    "You have made the same call {n} times in a row. Step back: re-read the last results and try a different approach."
                )));
            }
        }
        if c.recent_calls.len() > 32 {
            c.recent_calls.drain(..16);
        }
        if let Some(reason) = failures.first() {
            if escalate(inner, c, reason) {
                turn = 0;
            }
        }
    }
}

fn has_tool_use(content: &[Block]) -> bool {
    content.iter().any(|b| matches!(b, Block::ToolUse { .. }))
}

enum ToolOutcome {
    Text(String),
    /// A tool result that is also evidence the agent is struggling.
    Failure(String, String),
    Delivered(Deliverable, String),
}

async fn run_tool(
    inner: &Arc<Inner>,
    c: &Conversation,
    name: &str,
    input: &Value,
) -> Result<ToolOutcome> {
    let text = ToolOutcome::Text;
    match name {
        "skill" => {
            let n = input["name"].as_str().context("missing `name`")?;
            inner
                .skills
                .read(n, input["file"].as_str())
                .map(text)
                .ok_or_else(|| anyhow!("no skill or file named {n}"))
        }
        "create_tickets" if c.can_spawn => create_tickets(inner, c.id, input).await.map(text),
        "work_tickets" if c.can_spawn => work_tickets(inner, c.id, input).await.map(text),
        "revise_ticket" if c.can_spawn => revise(inner, c.id, input).await.map(text),
        "file_ticket" if c.kind != AgentKind::Triage => {
            let owner = inner.state.lock().unwrap().agents[c.id]
                .parent
                .unwrap_or(MAIN);
            file_ticket(inner, c.id, owner, input).map(text)
        }
        "triage_ticket" if c.kind == AgentKind::Triage => triage(inner, c, input),
        "submit_deliverable" if c.kind != AgentKind::Triage => submit(inner, c, input).await,
        "read" | "bash" => c.ws.run(name, input).await.map(text),
        "write" | "edit" if c.kind != AgentKind::Triage => c.ws.run(name, input).await.map(text),
        other => bail!("tool `{other}` is not available to you"),
    }
}

// ----------------------------------------------------------------- review

async fn await_verdict(inner: &Arc<Inner>, approval: Approval) -> Verdict {
    let (tx, rx) = oneshot::channel();
    let agent = approval.agent;
    let id = inner.update(|s| {
        let id = s.approvals.len();
        s.approvals.push(Approval { id, ..approval });
        id
    });
    inner.verdicts.lock().unwrap().insert(id, tx);
    inner.set_status(agent, AgentStatus::AwaitingApproval);
    let verdict = rx
        .await
        .unwrap_or(Verdict::Rejected("approval channel closed".into()));
    inner.update(|s| {
        s.approvals[id].state = match &verdict {
            Verdict::Approved => ApprovalState::Approved,
            Verdict::Rejected(f) => ApprovalState::Rejected {
                feedback: f.clone(),
            },
        }
    });
    inner.set_status(agent, AgentStatus::Running);
    verdict
}

async fn submit(inner: &Arc<Inner>, c: &Conversation, input: &Value) -> Result<ToolOutcome> {
    let mut deliverable = Deliverable {
        summary: input["summary"]
            .as_str()
            .context("missing summary")?
            .to_string(),
        files: input["files"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|f| f.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        diff_stat: None,
    };
    let (depth, parent) = {
        let s = inner.state.lock().unwrap();
        (s.agents[c.id].depth, s.agents[c.id].parent)
    };
    let ticket = c
        .ticket
        .as_ref()
        .and_then(|k| inner.state.lock().unwrap().ticket(k).cloned());
    let isolated_worker = inner.isolated() && c.kind == AgentKind::Worker;

    // 1. Snapshot the work on the agent's branch.
    if isolated_worker {
        let (dir, branch) = inner.home(c.id);
        let (_, parent_branch) = inner.home(parent.unwrap_or(MAIN));
        let _g = inner.git.lock().await;
        let first = deliverable.summary.lines().next().unwrap_or("").to_string();
        let msg = format!(
            "{}: {}",
            c.ticket.as_deref().unwrap_or("work"),
            truncate(&first, 60)
        );
        git::commit_all(&dir, &msg).await?;
        deliverable.diff_stat = git::diff_stat(&dir, &parent_branch, &branch)
            .await
            .ok()
            .filter(|d| !d.is_empty());
    }

    // 2. The ticket's check gates review. Failing it is escalation evidence.
    if let Some(check) = ticket.as_ref().and_then(|t| t.check.clone()) {
        let (code, out) = c.ws.sh(&check).await?;
        if code != 0 {
            return Ok(ToolOutcome::Failure(
                format!(
                    "CHECK FAILED: `{check}` exited {code}. Nothing was sent for review.\n{}\nFix it, then call submit_deliverable again.",
                    truncate(&out, 3000)
                ),
                "check failed".into(),
            ));
        }
    }
    inner.agent(c.id, |a| a.deliverable = Some(deliverable.clone()));

    // 3. Review: the agent's manager for deep work, otherwise the human.
    let manager_reviews =
        c.kind == AgentKind::Worker && depth > inner.cfg.orchestrator.human_review_depth;
    if !manager_reviews && !inner.cfg.orchestrator.auto_approve {
        if let Some(k) = &c.ticket {
            inner.ticket(k, |t| t.state = TicketState::InReview);
        }
        let approval = Approval {
            id: 0,
            kind: ApprovalKind::Deliverable,
            agent: c.id,
            deliverable: deliverable.clone(),
            tickets: c.ticket.iter().cloned().collect(),
            state: ApprovalState::Pending,
        };
        if let Verdict::Rejected(feedback) = await_verdict(inner, approval).await {
            if let Some(k) = &c.ticket {
                inner.ticket(k, |t| {
                    t.state = TicketState::InProgress;
                    t.notes.push(format!("rejected by the user: {feedback}"));
                });
            }
            return Ok(ToolOutcome::Failure(
                format!("REJECTED by the user. Feedback: {feedback}\nAddress it, then call submit_deliverable again."),
                "rejected by the user".into(),
            ));
        }
    }

    // 4. Land it on the parent's branch.
    if isolated_worker {
        let (_, branch) = inner.home(c.id);
        let (into, parent_branch) = inner.home(parent.unwrap_or(MAIN));
        let _g = inner.git.lock().await;
        let msg = format!(
            "backspace: merge {}",
            c.ticket.as_deref().unwrap_or(&branch)
        );
        if let git::Merge::Conflict(files) = git::merge(&into, &branch, &msg).await? {
            return Ok(ToolOutcome::Text(format!(
                "Accepted, but merging into `{parent_branch}` conflicts in:\n{files}\nIn your worktree run `git merge {parent_branch}`, resolve the conflicts, commit, and call submit_deliverable again."
            )));
        }
    }

    let note = if manager_reviews {
        "Merged and sent to your manager for review. If they request changes, you will be reopened."
    } else if inner.cfg.orchestrator.auto_approve {
        "Auto-approved."
    } else {
        "Approved by the user."
    };
    Ok(ToolOutcome::Delivered(deliverable, note.into()))
}

// ---------------------------------------------------------------- tickets

async fn create_tickets(inner: &Arc<Inner>, caller: AgentId, input: &Value) -> Result<String> {
    let specs = input["tickets"]
        .as_array()
        .context("`tickets` must be an array")?;
    if specs.is_empty() {
        bail!("no tickets given");
    }
    let needs_approval =
        caller == MAIN && inner.cfg.workflow.plan_approval && !inner.cfg.orchestrator.auto_approve;
    let state = if needs_approval {
        TicketState::Proposed
    } else {
        TicketState::ReadyForAgent
    };

    let batch: Vec<Ticket> = inner.update(|s| -> Result<Vec<Ticket>> {
        let mut batch: Vec<Ticket> = Vec::new();
        for (i, spec) in specs.iter().enumerate() {
            let t = Ticket::from_json(spec, s.tickets.len() + i + 1, caller, state)?;
            if s.ticket(&t.key).is_some()
                || s.agents.iter().any(|a| a.key == t.key)
                || batch.iter().any(|b| b.key == t.key)
            {
                bail!("duplicate ticket key `{}`", t.key);
            }
            // Sibling-only, listed-before edges keep the wait graph acyclic.
            for b in &t.blocked_by {
                let known = batch.iter().any(|x| &x.key == b)
                    || s.ticket(b).is_some_and(|x| x.owner == caller);
                if !known {
                    bail!(
                        "ticket `{}` is blocked by `{b}`, which is not one of your earlier tickets",
                        t.key
                    );
                }
            }
            batch.push(t);
        }
        s.tickets.extend(batch.iter().cloned());
        Ok(batch)
    })?;
    for t in &batch {
        t.write(&inner.root);
    }
    let keys: Vec<String> = batch.iter().map(|t| t.key.clone()).collect();
    if !needs_approval {
        return Ok(format!(
            "Created {}. Dispatch them with work_tickets.",
            keys.join(", ")
        ));
    }

    let summary = batch
        .iter()
        .map(|t| {
            let blocked = if t.blocked_by.is_empty() {
                String::new()
            } else {
                format!(" (after {})", t.blocked_by.join(", "))
            };
            let criteria: Vec<String> = t.acceptance.iter().map(|a| format!("  - {a}")).collect();
            format!(
                "{}. {}: {}{blocked}\n{}",
                t.num,
                t.key,
                t.title,
                criteria.join("\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let approval = Approval {
        id: 0,
        kind: ApprovalKind::Plan,
        agent: caller,
        deliverable: Deliverable {
            summary,
            files: vec![],
            diff_stat: None,
        },
        tickets: keys.clone(),
        state: ApprovalState::Pending,
    };
    match await_verdict(inner, approval).await {
        Verdict::Approved => {
            for k in &keys {
                inner.ticket(k, |t| t.state = TicketState::ReadyForAgent);
            }
            Ok(format!(
                "Plan APPROVED by the user: {}. Dispatch with work_tickets.",
                keys.join(", ")
            ))
        }
        Verdict::Rejected(feedback) => {
            inner.update(|s| s.tickets.retain(|t| !keys.contains(&t.key)));
            for t in &batch {
                let _ = std::fs::remove_file(
                    inner
                        .root
                        .join(format!(".backspace/tickets/{:02}-{}.md", t.num, t.key)),
                );
            }
            Ok(format!(
                "Plan REJECTED by the user; those tickets were discarded. Feedback: {feedback}\nRevise the plan and call create_tickets again."
            ))
        }
    }
}

async fn work_tickets(inner: &Arc<Inner>, caller: AgentId, input: &Value) -> Result<String> {
    let requested: Option<Vec<String>> = input["keys"].as_array().map(|a| {
        a.iter()
            .filter_map(|k| k.as_str().map(String::from))
            .collect()
    });

    let ids: Vec<(AgentId, String)> = inner.update(|s| -> Result<Vec<(AgentId, String)>> {
        let keys: Vec<String> = match &requested {
            Some(k) => k.clone(),
            None => s
                .tickets
                .iter()
                .filter(|t| t.owner == caller && t.state == TicketState::ReadyForAgent)
                .map(|t| t.key.clone())
                .collect(),
        };
        if keys.is_empty() {
            bail!("no tickets ready for agents; create_tickets first");
        }
        for k in &keys {
            let t = s.ticket(k).ok_or_else(|| anyhow!("no ticket `{k}`"))?;
            if t.owner != caller {
                bail!("ticket `{k}` is not yours to dispatch");
            }
            if !matches!(t.state, TicketState::ReadyForAgent | TicketState::Failed) {
                bail!("ticket `{k}` is {}, not ready-for-agent", t.state.label());
            }
            for b in &t.blocked_by {
                let bt = s.ticket(b).ok_or_else(|| anyhow!("no ticket `{b}`"))?;
                if !bt.state.is_settled() && !keys.contains(b) {
                    bail!("ticket `{k}` is blocked by `{b}` ({}); dispatch them together", bt.state.label());
                }
            }
        }
        let remaining = inner.cfg.orchestrator.max_subagents.saturating_sub(s.subagent_count());
        if keys.len() > remaining {
            bail!("agent cap reached: {} tickets, {remaining} agents left. Merge tickets or do small ones yourself.", keys.len());
        }
        let depth = s.agents[caller].depth + 1;
        let mut out = Vec::new();
        for k in keys {
            let id = s.agents.len();
            let t = s.ticket_mut(&k).unwrap();
            t.assignee = Some(id);
            t.state = TicketState::Queued;
            let mut rec = new_record(id, AgentKind::Worker, Some(caller), depth, format!("{k}#{id}"), t.title.clone());
            rec.ticket = Some(k.clone());
            rec.budget_usd = t.budget_usd;
            rec.brief = t.what_to_build.clone();
            s.agents.push(rec);
            out.push((id, k));
        }
        Ok(out)
    })?;
    for (_, k) in &ids {
        let t = inner.state.lock().unwrap().ticket(k).cloned();
        if let Some(t) = t {
            t.write(&inner.root);
        }
    }
    inner.save();

    let mut handles = Vec::new();
    for (id, key) in &ids {
        let (tx, rx) = watch::channel(false);
        inner.done.lock().unwrap().insert(key.clone(), rx);
        let (inner2, id, key) = (inner.clone(), *id, key.clone());
        handles.push(tokio::spawn(async move {
            let res = run_worker(inner2.clone(), id).await;
            settle(&inner2, id, &key, &res);
            let _ = tx.send(true);
            res
        }));
    }

    let mut report = Vec::new();
    for ((id, _), h) in ids.iter().zip(handles) {
        let res = h.await.map_err(|e| anyhow!("crashed: {e}")).and_then(|r| r);
        report.push(report_line(inner, *id, res));
    }
    // Tickets filed and triaged while these ran.
    let fresh: Vec<String> = inner
        .state
        .lock()
        .unwrap()
        .tickets
        .iter()
        .filter(|t| {
            t.owner == caller
                && matches!(
                    t.state,
                    TicketState::ReadyForAgent
                        | TicketState::ReadyForHuman
                        | TicketState::NeedsInfo
                        | TicketState::NeedsTriage
                )
        })
        .map(|t| format!("- `{}` {} [{}]", t.key, t.title, t.state.label()))
        .collect();
    if !fresh.is_empty() {
        report.push(format!(
            "## Open tickets you own\n{}\nDispatch the ready-for-agent ones with work_tickets if they belong in scope.",
            fresh.join("\n")
        ));
    }
    Ok(report.join("\n\n"))
}

/// Mark a finished worker and its ticket, and log the routing outcome.
fn settle(inner: &Inner, id: AgentId, key: &str, res: &Result<Deliverable>) {
    match res {
        Ok(_) => inner.set_status(id, AgentStatus::Approved),
        Err(e) => {
            inner.log(id, LogKind::Error, format!("{e:#}"));
            inner.set_status(id, AgentStatus::Failed);
        }
    }
    inner.ticket(key, |t| {
        t.state = if res.is_ok() {
            TicketState::Done
        } else {
            TicketState::Failed
        }
    });

    // Outcome log: the raw material for learning which routes actually work.
    let line = {
        let s = inner.state.lock().unwrap();
        let a = &s.agents[id];
        json!({
            "ticket": key,
            "title": a.title,
            "brief_chars": a.brief.len(),
            "final": a.decision.as_ref().map(|d| json!({"model": d.model, "effort": d.effort})),
            "escalations": a.escalations,
            "cost_usd": a.cost_usd,
            "outcome": if res.is_ok() { "done" } else { "failed" },
        })
    };
    let path = inner.root.join(".backspace/outcomes.jsonl");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

/// Explicit BoxFuture return type breaks the drive → work_tickets →
/// run_worker → drive async type cycle.
fn run_worker(inner: Arc<Inner>, id: AgentId) -> BoxFuture<'static, Result<Deliverable>> {
    async move {
        let key = inner.state.lock().unwrap().agents[id]
            .ticket
            .clone()
            .context("worker without ticket")?;
        let blockers = inner
            .state
            .lock()
            .unwrap()
            .ticket(&key)
            .context("ticket vanished")?
            .blocked_by
            .clone();
        for b in &blockers {
            let rx = inner.done.lock().unwrap().get(b).cloned();
            if let Some(mut rx) = rx {
                let _ = rx.wait_for(|done| *done).await;
            }
        }

        let (brief, context, depth, parent) = {
            let s = inner.state.lock().unwrap();
            let me = &s.agents[id];
            let t = s.ticket(&key).context("ticket vanished")?;
            let deps: Vec<String> = t
                .blocked_by
                .iter()
                .filter_map(|b| s.ticket(b))
                .map(|bt| {
                    let del = bt.assignee.and_then(|a| s.agents[a].deliverable.clone());
                    match (bt.state, del) {
                        (TicketState::Done, Some(d)) => {
                            format!(
                                "## `{}` ({}), done and merged into your base\n{}",
                                bt.key, bt.title, d.summary
                            )
                        }
                        _ => format!(
                            "## `{}` ({}) FAILED; work around it or report the gap.",
                            bt.key, bt.title
                        ),
                    }
                })
                .collect();
            // Goal ancestry: every agent knows why its ticket exists.
            let chain: Vec<String> = s
                .ancestry(id)
                .into_iter()
                .skip(1)
                .rev()
                .map(|a| {
                    format!(
                        "- {}: {}",
                        s.agents[a].title,
                        truncate(&s.agents[a].brief, 600)
                    )
                })
                .collect();
            let mut ctx = format!(
                "# Why this ticket exists (project goal first)\n\n{}",
                chain.join("\n")
            );
            if !deps.is_empty() {
                ctx.push_str("\n\n# Tickets that blocked this one\n\n");
                ctx.push_str(&deps.join("\n\n"));
            }
            if let Some(b) = me.budget_usd {
                ctx.push_str(&format!(
                    "\n\nYour budget, including anything you delegate: ${b:.2}."
                ));
            }
            (t.brief(), ctx, me.depth, me.parent.unwrap_or(MAIN))
        };

        inner.set_status(id, AgentStatus::Running);
        inner.ticket(&key, |t| t.state = TicketState::InProgress);
        let mut decision = inner.router.route(AgentRole::Sub, &brief, &context).await?;
        inner.cap_start(&mut decision);
        announce_route(&inner, id, &decision);

        // Own branch, own worktree, forked from the parent's current work.
        let dir = if inner.isolated() {
            let (_, parent_branch) = inner.home(parent);
            let path = git::worktree_path(&inner.root, &key);
            let branch = git::branch_name(&key);
            {
                let _g = inner.git.lock().await;
                git::add_worktree(&inner.root, &path, &branch, &parent_branch).await?;
            }
            let (p, b) = (path.clone(), branch.clone());
            inner.agent(id, |a| {
                a.worktree = Some(p);
                a.branch = Some(b);
            });
            inner.log(
                id,
                LogKind::System,
                format!("working on branch `{branch}` from `{parent_branch}`"),
            );
            path
        } else {
            inner.root.clone()
        };

        let first = format!("{brief}\n{context}");
        inner.log(id, LogKind::User, first.clone());
        let can_spawn = inner.can_spawn(depth);
        let mut c = Conversation {
            id,
            kind: AgentKind::Worker,
            ticket: Some(key),
            can_spawn,
            ws: inner.workspace(&dir),
            initial: decision.clone(),
            system: inner.system_prompt(AgentKind::Worker, depth, &decision),
            decision,
            messages: vec![Message::user_text(first)],
            tools: tools_for(AgentKind::Worker, can_spawn),
            budget_warned_at: None,
            turns_total: 0,
            recent_calls: vec![],
        };
        let res = finish(&inner, &mut c).await;
        if c.initial.model != c.decision.model || c.initial.effort != c.decision.effort {
            inner.log(
                id,
                LogKind::System,
                format!(
                    "finished at {} @ {} (started {} @ {})",
                    c.decision.model, c.decision.effort, c.initial.model, c.initial.effort
                ),
            );
        }
        inner.parked.lock().unwrap().insert(id, c);
        res
    }
    .boxed()
}

fn report_line(inner: &Inner, id: AgentId, res: Result<Deliverable>) -> String {
    let (key, title, depth, escalations, spent) = {
        let s = inner.state.lock().unwrap();
        let a = &s.agents[id];
        (
            a.ticket.clone().unwrap_or_else(|| a.key.clone()),
            a.title.clone(),
            a.depth,
            a.escalations.len(),
            s.subtree_cost(id),
        )
    };
    let esc = if escalations > 0 {
        format!(", escalated {escalations}x")
    } else {
        String::new()
    };
    let body = |d: &Deliverable| {
        format!(
            "{}\nFiles: {}\n{}",
            d.summary,
            d.files.join(", "),
            d.diff_stat.clone().unwrap_or_default()
        )
    };
    match res {
        Ok(d) if depth > inner.cfg.orchestrator.human_review_depth => format!(
            "## `{key}` {title}: MERGED, FOR YOUR REVIEW (${spent:.3}{esc})\n{}\nVerify it in your worktree (read the files, run the checks). If it falls short, call revise_ticket with specific feedback.",
            body(&d)
        ),
        Ok(d) => format!("## `{key}` {title}: APPROVED by the user and merged (${spent:.3}{esc})\n{}", body(&d)),
        Err(e) => format!("## `{key}` {title}: FAILED (${spent:.3}{esc})\n{e:#}"),
    }
}

/// Reopen one of the caller's tickets with feedback, one rung up the ladder.
async fn revise(inner: &Arc<Inner>, caller: AgentId, input: &Value) -> Result<String> {
    let key = input["key"].as_str().context("missing `key`")?.to_string();
    let feedback = input["feedback"].as_str().context("missing `feedback`")?;
    let id = {
        let s = inner.state.lock().unwrap();
        let t = s.ticket(&key).ok_or_else(|| anyhow!("no ticket `{key}`"))?;
        if t.owner != caller {
            bail!("`{key}` is not one of your tickets");
        }
        t.assignee
            .ok_or_else(|| anyhow!("`{key}` was never worked"))?
    };
    let mut c = inner
        .parked
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or_else(|| anyhow!("`{key}` is still running or cannot be reopened"))?;

    let msg = format!("Your reviewer requested changes: {feedback}\nAddress this, then call submit_deliverable again.");
    inner.log(id, LogKind::User, msg.clone());
    c.messages.push(Message::user_text(msg));
    c.budget_warned_at = None;
    inner.ticket(&key, |t| {
        t.state = TicketState::InProgress;
        t.notes.push(format!("revision requested: {feedback}"));
    });
    inner.set_status(id, AgentStatus::Running);
    escalate(inner, &mut c, "reviewer requested changes");

    let res = finish(inner, &mut c).await;
    inner.parked.lock().unwrap().insert(id, c);
    settle(inner, id, &key, &res);
    Ok(report_line(inner, id, res))
}

/// Record a ticket that needs triage and start a triage agent for it.
fn file_ticket(
    inner: &Arc<Inner>,
    filer: AgentId,
    owner: AgentId,
    input: &Value,
) -> Result<String> {
    let title = input["title"].as_str().context("missing `title`")?;
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join("-");
    let mut v = input.clone();
    let t = inner.update(|s| -> Result<Ticket> {
        let num = s.tickets.len() + 1;
        v["key"] = json!(format!(
            "{}-{num}",
            if slug.is_empty() { "ticket" } else { &slug }
        ));
        let mut t = Ticket::from_json(&v, num, owner, TicketState::NeedsTriage)?;
        t.notes.push(format!("filed by {}", s.agents[filer].title));
        s.tickets.push(t.clone());
        Ok(t)
    })?;
    t.write(&inner.root);
    tokio::spawn(run_triage(inner.clone(), t.key.clone()));
    Ok(format!(
        "Filed as `{}`. A triage agent will classify it; whoever owns it decides whether to schedule it.",
        t.key
    ))
}

fn run_triage(inner: Arc<Inner>, key: String) -> BoxFuture<'static, ()> {
    async move {
        let Some((id, brief, others, owner)) = ({
            let mut s = inner.state.lock().unwrap();
            s.ticket(&key).cloned().map(|t| {
                let others: Vec<String> = s
                    .tickets
                    .iter()
                    .filter(|o| o.key != key)
                    .map(|o| {
                        format!(
                            "- `{}` {} [{}]: {}",
                            o.key,
                            o.title,
                            o.state.label(),
                            truncate(&o.what_to_build, 200)
                        )
                    })
                    .collect();
                let id = s.agents.len();
                let depth = s.agents[t.owner].depth + 1;
                let mut rec = new_record(
                    id,
                    AgentKind::Triage,
                    Some(t.owner),
                    depth,
                    format!("triage-{key}"),
                    format!("Triage: {}", t.title),
                );
                rec.ticket = Some(key.clone());
                s.agents.push(rec);
                (id, t.markdown(), others.join("\n"), t.owner)
            })
        }) else {
            return;
        };
        let _ = inner.changed.try_send(());

        let res: Result<Deliverable> = async {
            let mut d = inner
                .router
                .route(AgentRole::Sub, &format!("Triage this ticket:\n{brief}"), "")
                .await?;
            inner.cap_start(&mut d);
            d.effort = d.effort.min(Effort::Low);
            announce_route(&inner, id, &d);
            let (dir, _) = inner.home(owner);
            let first =
                format!(
                "# Ticket to triage\n\n{brief}\n\n# Existing tickets (check for duplicates)\n\n{}",
                if others.is_empty() { "none".to_string() } else { others }
            );
            inner.log(id, LogKind::User, first.clone());
            inner.set_status(id, AgentStatus::Running);
            let mut c = Conversation {
                id,
                kind: AgentKind::Triage,
                ticket: Some(key.clone()),
                can_spawn: false,
                ws: inner.workspace(&dir),
                initial: d.clone(),
                system: inner.system_prompt(AgentKind::Triage, 0, &d),
                decision: d,
                messages: vec![Message::user_text(first)],
                tools: tools_for(AgentKind::Triage, false),
                budget_warned_at: None,
                turns_total: 0,
                recent_calls: vec![],
            };
            finish(&inner, &mut c).await
        }
        .await;
        match res {
            Ok(_) => inner.set_status(id, AgentStatus::Approved),
            Err(e) => {
                inner.log(id, LogKind::Error, format!("{e:#}"));
                inner.set_status(id, AgentStatus::Failed);
                inner.ticket(&key, |t| t.notes.push(format!("triage failed: {e:#}")));
            }
        }
    }
    .boxed()
}

fn triage(inner: &Arc<Inner>, c: &Conversation, input: &Value) -> Result<ToolOutcome> {
    use crate::ticket::Category;
    let key = c.ticket.clone().context("no ticket to triage")?;
    let state_s = input["state"].as_str().context("missing `state`")?;
    let state = TicketState::parse(state_s).ok_or_else(|| anyhow!("unknown state `{state_s}`"))?;
    let notes = input["notes"].as_str().unwrap_or("").to_string();
    let list = |k: &str| -> Option<Vec<String>> {
        input[k].as_array().map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
    };
    inner
        .ticket(&key, |t| {
            t.state = state;
            if let Some(cat) = input["category"].as_str() {
                t.category = match cat {
                    "bug" => Category::Bug,
                    "enhancement" => Category::Enhancement,
                    _ => Category::Task,
                };
            }
            if let Some(w) = input["what_to_build"].as_str() {
                t.what_to_build = w.to_string();
            }
            if let Some(a) = list("acceptance") {
                t.acceptance = a;
            }
            if let Some(o) = list("out_of_scope") {
                t.out_of_scope = o;
            }
            if let Some(ch) = input["check"].as_str().filter(|x| !x.trim().is_empty()) {
                t.check = Some(ch.to_string());
            }
            if !notes.is_empty() {
                t.notes.push(format!("triage: {notes}"));
            }
        })
        .context("ticket vanished")?;
    let d = Deliverable {
        summary: format!("{key} → {}", state.label()),
        files: vec![],
        diff_stat: None,
    };
    Ok(ToolOutcome::Delivered(d, "Triage recorded.".into()))
}

// ------------------------------------------------------------------ tools

fn skill_tool() -> ToolDef {
    ToolDef {
        name: "skill",
        description: "Load a skill (a markdown playbook) by name, or one of its extra files.",
        schema: json!({"type": "object", "properties": {
            "name": {"type": "string"},
            "file": {"type": "string", "description": "optional extra file, e.g. AGENT-BRIEF.md"}
        }, "required": ["name"]}),
    }
}

fn create_tickets_tool() -> ToolDef {
    ToolDef {
        name: "create_tickets",
        description: "Turn a plan into tickets: tracer-bullet vertical slices (see the to-tickets skill). Each becomes one agent's work in its own git worktree. Does not start work; use work_tickets.",
        schema: json!({"type": "object", "properties": {
            "tickets": {"type": "array", "items": {"type": "object", "properties": {
                "key": {"type": "string", "description": "short unique id, letters/digits/-, e.g. `todo-api`; names the git branch"},
                "title": {"type": "string"},
                "category": {"type": "string", "enum": ["task", "enhancement", "bug"]},
                "what_to_build": {"type": "string", "description": "end-to-end behaviour this ticket makes work, from the user's perspective"},
                "acceptance": {"type": "array", "items": {"type": "string"}, "description": "specific, independently testable criteria"},
                "check": {"type": "string", "description": "shell command that must exit 0 before review, e.g. `cargo test todo_api`. Strongly recommended."},
                "out_of_scope": {"type": "array", "items": {"type": "string"}},
                "blocked_by": {"type": "array", "items": {"type": "string"}, "description": "keys of your earlier tickets that must be done first"},
                "budget_usd": {"type": "number"}
            }, "required": ["key", "title", "what_to_build", "acceptance"]}}
        }, "required": ["tickets"]}),
    }
}

fn work_tickets_tool() -> ToolDef {
    ToolDef {
        name: "work_tickets",
        description: "Dispatch agents for your ready-for-agent tickets (all of them if `keys` is omitted). They run in parallel, respecting blocked_by, each routed to its own model and starting cheap. Blocks until every one is done or failed; accepted work is merged into your branch.",
        schema: json!({"type": "object", "properties": {
            "keys": {"type": "array", "items": {"type": "string"}}
        }}),
    }
}

fn revise_tool() -> ToolDef {
    ToolDef {
        name: "revise_ticket",
        description: "Send one of your finished tickets back with feedback. Reopens that agent's conversation one rung up the model/effort ladder and blocks until it resubmits.",
        schema: json!({"type": "object", "properties": {
            "key": {"type": "string"},
            "feedback": {"type": "string", "description": "what is wrong and what done looks like"}
        }, "required": ["key", "feedback"]}),
    }
}

fn file_ticket_tool() -> ToolDef {
    ToolDef {
        name: "file_ticket",
        description: "Report a bug or follow-up you found that is outside your ticket's scope, instead of fixing it. A triage agent classifies it; your manager decides whether to schedule it. Returns immediately.",
        schema: json!({"type": "object", "properties": {
            "title": {"type": "string"},
            "category": {"type": "string", "enum": ["bug", "enhancement", "task"]},
            "what_to_build": {"type": "string", "description": "what is wrong or missing, how you noticed, how to reproduce"}
        }, "required": ["title", "what_to_build"]}),
    }
}

fn triage_tool() -> ToolDef {
    ToolDef {
        name: "triage_ticket",
        description: "Record your triage decision for the ticket you were given. Ends your task.",
        schema: json!({"type": "object", "properties": {
            "state": {"type": "string", "enum": ["ready_for_agent", "ready_for_human", "needs_info", "wontfix"]},
            "category": {"type": "string", "enum": ["bug", "enhancement", "task"]},
            "notes": {"type": "string", "description": "what you verified and why this state"},
            "what_to_build": {"type": "string", "description": "rewritten as an agent brief, if moving to ready_for_agent"},
            "acceptance": {"type": "array", "items": {"type": "string"}},
            "check": {"type": "string"},
            "out_of_scope": {"type": "array", "items": {"type": "string"}}
        }, "required": ["state", "notes"]}),
    }
}

fn deliver_tool() -> ToolDef {
    ToolDef {
        name: "submit_deliverable",
        description: "Submit your finished work. Your ticket's check runs first; then it goes to review (the user, or your manager deep in the tree). Blocks until accepted or returned with feedback.",
        schema: json!({"type": "object", "properties": {
            "summary": {"type": "string", "description": "what was built, how it connects to the rest, how you verified it, what is left"},
            "files": {"type": "array", "items": {"type": "string"}}
        }, "required": ["summary"]}),
    }
}
