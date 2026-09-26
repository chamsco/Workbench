//! One project = one main agent you talk to, plus a tree of agents under it.
//! Any agent above `max_depth` can hire reports. Deliverables from the top
//! `human_review_depth` levels go to the human; deeper ones go to the agent
//! that spawned them, which can send them back with `revise_agent`. Either
//! way, feedback lands in the same conversation, so agents revise instead of
//! restarting.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures::future::{join_all, BoxFuture, FutureExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, watch, Semaphore};

use crate::config::Config;
use crate::project::*;
use crate::provider::{self, Block, Message, Role, Stop, ToolDef};
use crate::router::{truncate, AgentRole, Decision, Router};
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
    ws: Workspace,
    agents_md: Option<String>,
    state: Mutex<ProjectState>,
    verdicts: Mutex<HashMap<usize, oneshot::Sender<Verdict>>>,
    done: Mutex<HashMap<AgentId, watch::Receiver<bool>>>,
    /// Finished sub-agent conversations, kept so a manager can reopen them.
    parked: Mutex<HashMap<AgentId, Conversation>>,
    limiter: Semaphore,
    changed: async_channel::Sender<()>,
}

pub struct Harness {
    inner: Arc<Inner>,
    to_main: mpsc::UnboundedSender<String>,
    changed: async_channel::Receiver<()>,
    rt: tokio::runtime::Runtime,
}

impl Harness {
    /// Open (or create) a project rooted at `workspace`. Owns its own tokio
    /// runtime so any UI toolkit can drive it through plain sync calls.
    pub fn open(workspace: PathBuf) -> Result<Harness> {
        std::fs::create_dir_all(&workspace)?;
        let workspace = workspace.canonicalize()?;
        let (cfg, config_source) = Config::load(&workspace)?;
        let http = reqwest::Client::builder().build()?;
        let name = workspace
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into());

        let main = AgentRecord {
            id: MAIN,
            role: AgentRole::Main,
            parent: None,
            depth: 0,
            key: "main".into(),
            title: "Main agent".into(),
            brief: String::new(),
            depends_on: vec![],
            status: AgentStatus::Idle,
            decision: None,
            log: vec![],
            cost_usd: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            deliverable: None,
            budget_usd: None,
        };
        let (changed_tx, changed_rx) = async_channel::bounded(1);
        let inner = Arc::new(Inner {
            router: Router::new(http.clone(), cfg.clone()),
            ws: Workspace {
                root: workspace.clone(),
                bash_timeout: Duration::from_secs(cfg.orchestrator.bash_timeout_secs),
            },
            agents_md: std::fs::read_to_string(workspace.join("AGENTS.md")).ok(),
            limiter: Semaphore::new(cfg.orchestrator.max_parallel_calls),
            state: Mutex::new(ProjectState {
                name,
                workspace,
                agents: vec![main],
                approvals: vec![],
                total_cost_usd: 0.0,
                router_cost_usd: 0.0,
                config_source,
            }),
            verdicts: Mutex::new(HashMap::new()),
            done: Mutex::new(HashMap::new()),
            parked: Mutex::new(HashMap::new()),
            changed: changed_tx,
            cfg,
            http,
        });

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
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

    fn can_spawn(&self, depth: usize) -> bool {
        depth < self.cfg.orchestrator.max_depth
    }

    fn system_prompt(&self, role: AgentRole, depth: usize, decision: &Decision) -> String {
        let remaining = self
            .cfg
            .orchestrator
            .max_subagents
            .saturating_sub(self.state.lock().unwrap().subagent_count());
        let mut s = match role {
            AgentRole::Main => format!(include_str!("prompts/main.md"), remaining = remaining),
            AgentRole::Sub => include_str!("prompts/sub.md").to_string(),
        };
        if role == AgentRole::Sub && self.can_spawn(depth) {
            s.push_str("\n\n");
            s.push_str(&format!(
                include_str!("prompts/manager.md"),
                remaining = remaining,
                depth = depth,
                max_depth = self.cfg.orchestrator.max_depth,
            ));
        }
        s.push_str(&format!(
            "\n\nYou are running on {} at {} effort. Your working directory is the project workspace.",
            decision.model, decision.effort
        ));
        if let Some(md) = &self.agents_md {
            s.push_str("\n\n# Project instructions (AGENTS.md)\n\n");
            s.push_str(md);
        }
        s
    }
}

// ------------------------------------------------------------------ agents

enum TurnEnd {
    Replied,
    Delivered(Deliverable),
}

struct Conversation {
    id: AgentId,
    can_spawn: bool,
    budget_warned_at: Option<usize>,
    decision: Decision,
    system: String,
    messages: Vec<Message>,
    tools: Vec<ToolDef>,
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
                        can_spawn: true,
                        budget_warned_at: None,
                        tools: agent_tools(true),
                        system: inner.system_prompt(AgentRole::Main, 0, &d),
                        decision: d,
                        messages: vec![],
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

/// Explicit BoxFuture return type breaks the drive → spawn → run_sub → drive
/// async type cycle.
fn run_sub(inner: Arc<Inner>, id: AgentId) -> BoxFuture<'static, Result<Deliverable>> {
    async move {
        let deps = inner.state.lock().unwrap().agents[id].depends_on.clone();
        for dep in &deps {
            let rx = inner.done.lock().unwrap().get(dep).cloned();
            if let Some(mut rx) = rx {
                let _ = rx.wait_for(|done| *done).await;
            }
        }

        let (brief, context, depth) = {
            let s = inner.state.lock().unwrap();
            let me = &s.agents[id];
            let deps: Vec<String> = me
                .depends_on
                .iter()
                .map(|d| {
                    let a = &s.agents[*d];
                    match &a.deliverable {
                        Some(del) if a.status == AgentStatus::Approved => format!(
                            "## Dependency `{}` ({}), accepted\n{}\nFiles: {}",
                            a.key,
                            a.title,
                            del.summary,
                            del.files.join(", ")
                        ),
                        _ => format!(
                            "## Dependency `{}` ({}) FAILED; work around it or report the gap.",
                            a.key, a.title
                        ),
                    }
                })
                .collect();
            // Goal ancestry: every agent knows why its task exists, all the way
            // up to the project goal.
            let chain: Vec<String> = s
                .ancestry(id)
                .into_iter()
                .skip(1)
                .rev()
                .map(|a| {
                    let a = &s.agents[a];
                    format!("- {}: {}", a.title, truncate(&a.brief, 600))
                })
                .collect();
            let mut ctx = format!(
                "# Why this task exists (project goal first)\n\n{}",
                chain.join("\n")
            );
            if !deps.is_empty() {
                ctx.push_str("\n\n# What your dependencies delivered\n\n");
                ctx.push_str(&deps.join("\n\n"));
            }
            if let Some(b) = me.budget_usd {
                ctx.push_str(&format!(
                    "\n\nYour budget, including anything you delegate: ${b:.2}."
                ));
            }
            (format!("# {}\n\n{}", me.title, me.brief), ctx, me.depth)
        };

        inner.set_status(id, AgentStatus::Running);
        let decision = inner.router.route(AgentRole::Sub, &brief, &context).await?;
        announce_route(&inner, id, &decision);

        let first = format!("{brief}\n\n{context}");
        inner.log(id, LogKind::User, first.clone());
        let can_spawn = inner.can_spawn(depth);
        let mut c = Conversation {
            id,
            can_spawn,
            budget_warned_at: None,
            system: inner.system_prompt(AgentRole::Sub, depth, &decision),
            decision,
            messages: vec![Message::user_text(first)],
            tools: agent_tools(can_spawn),
        };
        let res = finish(&inner, &mut c).await;
        inner.parked.lock().unwrap().insert(id, c);
        res
    }
    .boxed()
}

/// Drive a sub-agent until it delivers, nudging it if it stops early.
async fn finish(inner: &Arc<Inner>, c: &mut Conversation) -> Result<Deliverable> {
    for _nudge in 0..2 {
        match drive(inner, c).await? {
            TurnEnd::Delivered(d) => return Ok(d),
            TurnEnd::Replied => {
                let nudge = "You ended your turn without calling submit_deliverable. Finish the task, then call submit_deliverable.";
                inner.log(c.id, LogKind::System, nudge);
                c.messages.push(Message::user_text(nudge));
            }
        }
    }
    bail!("agent stopped without submitting a deliverable")
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
    let spec = inner
        .cfg
        .model(&c.decision.model)
        .ok_or_else(|| anyhow!("routed to unknown model {}", c.decision.model))?
        .clone();
    let provider_cfg = inner.cfg.providers[&spec.provider].clone();
    let budget = c.decision.effort.max_turns();

    for turn in 0.. {
        if turn == budget {
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
                    c.budget_warned_at = Some(turn);
                }
                Some(t) if turn > t + 2 => bail!("{reason}"),
                Some(_) => {}
            }
        }

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

        let results = join_all(calls.iter().map(|(_, name, input)| {
            inner.log(
                c.id,
                LogKind::ToolCall,
                format!("{name} {}", truncate(&input.to_string(), 400)),
            );
            run_tool(inner, c.id, c.can_spawn, name, input)
        }))
        .await;

        let mut blocks = Vec::new();
        let mut delivered = None;
        for ((call_id, _, _), res) in calls.iter().zip(results) {
            let (content, is_error) = match res {
                Ok(ToolOutcome::Text(t)) => (t, false),
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
    }
    unreachable!()
}

fn has_tool_use(content: &[Block]) -> bool {
    content.iter().any(|b| matches!(b, Block::ToolUse { .. }))
}

enum ToolOutcome {
    Text(String),
    Delivered(Deliverable, String),
}

async fn run_tool(
    inner: &Arc<Inner>,
    id: AgentId,
    can_spawn: bool,
    name: &str,
    input: &Value,
) -> Result<ToolOutcome> {
    match name {
        "spawn_agents" if can_spawn => spawn_batch(inner, id, input).await.map(ToolOutcome::Text),
        "revise_agent" if can_spawn => revise(inner, id, input).await.map(ToolOutcome::Text),
        "submit_deliverable" => submit(inner, id, input).await,
        _ => inner.ws.run(name, input).await.map(ToolOutcome::Text),
    }
}

async fn submit(inner: &Arc<Inner>, id: AgentId, input: &Value) -> Result<ToolOutcome> {
    let deliverable = Deliverable {
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
    };

    let depth = inner.state.lock().unwrap().agents[id].depth;
    if depth > inner.cfg.orchestrator.human_review_depth {
        inner.agent(id, |a| a.deliverable = Some(deliverable.clone()));
        return Ok(ToolOutcome::Delivered(
            deliverable,
            "Sent to your manager for review. If they request changes, you will be reopened."
                .into(),
        ));
    }
    if inner.cfg.orchestrator.auto_approve {
        inner.agent(id, |a| a.deliverable = Some(deliverable.clone()));
        return Ok(ToolOutcome::Delivered(deliverable, "Auto-approved.".into()));
    }

    let (tx, rx) = oneshot::channel();
    let approval_id = inner.update(|s| {
        let aid = s.approvals.len();
        s.approvals.push(Approval {
            id: aid,
            agent: id,
            deliverable: deliverable.clone(),
            state: ApprovalState::Pending,
        });
        s.agents[id].deliverable = Some(deliverable.clone());
        aid
    });
    inner.verdicts.lock().unwrap().insert(approval_id, tx);
    inner.set_status(id, AgentStatus::AwaitingApproval);

    let verdict = rx
        .await
        .unwrap_or(Verdict::Rejected("approval channel closed".into()));
    match verdict {
        Verdict::Approved => {
            inner.update(|s| s.approvals[approval_id].state = ApprovalState::Approved);
            inner.set_status(id, AgentStatus::Running);
            Ok(ToolOutcome::Delivered(
                deliverable,
                "Approved by the user.".into(),
            ))
        }
        Verdict::Rejected(feedback) => {
            inner.update(|s| {
                s.approvals[approval_id].state = ApprovalState::Rejected {
                    feedback: feedback.clone(),
                };
            });
            inner.set_status(id, AgentStatus::Running);
            Ok(ToolOutcome::Text(format!(
                "REJECTED by the user. Feedback: {feedback}\nAddress it, then call submit_deliverable again."
            )))
        }
    }
}

async fn spawn_batch(inner: &Arc<Inner>, parent: AgentId, input: &Value) -> Result<String> {
    let specs = input["agents"]
        .as_array()
        .context("`agents` must be an array")?;
    if specs.is_empty() {
        bail!("no agents given");
    }

    let ids: Vec<AgentId> = inner.update(|s| -> Result<Vec<AgentId>> {
        let remaining = inner.cfg.orchestrator.max_subagents.saturating_sub(s.subagent_count());
        if specs.len() > remaining {
            bail!("sub-agent cap reached: {} requested, {remaining} left. Merge tasks or do small ones yourself.", specs.len());
        }
        let first_new = s.agents.len();
        let depth = s.agents[parent].depth + 1;
        // Keys are project-wide; dependencies are sibling-only, which keeps
        // the wait graph acyclic (a parent never waits on its own waiter).
        let mut siblings: HashMap<String, AgentId> = s
            .agents
            .iter()
            .filter(|a| a.parent == Some(parent))
            .map(|a| (a.key.clone(), a.id))
            .collect();
        for (i, spec) in specs.iter().enumerate() {
            let key = spec["key"].as_str().context("each agent needs a `key`")?.to_string();
            if s.agents.iter().any(|a| a.key == key) || siblings.insert(key.clone(), first_new + i).is_some() {
                bail!("duplicate agent key `{key}`");
            }
        }
        let mut ids = Vec::new();
        for (i, spec) in specs.iter().enumerate() {
            let id = first_new + i;
            let depends_on = spec["depends_on"]
                .as_array()
                .map(|d| d.iter().filter_map(|k| k.as_str()).collect::<Vec<_>>())
                .unwrap_or_default()
                .into_iter()
                .map(|k| siblings.get(k).copied().ok_or_else(|| anyhow!("unknown dependency `{k}` (only siblings can be dependencies)")))
                .collect::<Result<Vec<_>>>()?;
            if depends_on.iter().any(|d| *d >= id) {
                bail!("agent `{}` may only depend on agents listed before it", spec["key"]);
            }
            s.agents.push(AgentRecord {
                id,
                role: AgentRole::Sub,
                parent: Some(parent),
                depth,
                key: spec["key"].as_str().unwrap().to_string(),
                title: spec["title"].as_str().unwrap_or("untitled").to_string(),
                brief: spec["brief"].as_str().context("each agent needs a `brief`")?.to_string(),
                depends_on,
                status: AgentStatus::Queued,
                decision: None,
                log: vec![],
                cost_usd: 0.0,
                input_tokens: 0,
                output_tokens: 0,
                deliverable: None,
                budget_usd: spec["budget_usd"].as_f64(),
            });
            ids.push(id);
        }
        Ok(ids)
    })?;
    inner.save();

    let mut handles = Vec::new();
    for &id in &ids {
        let (tx, rx) = watch::channel(false);
        inner.done.lock().unwrap().insert(id, rx);
        let inner2 = inner.clone();
        handles.push(tokio::spawn(async move {
            let res = run_sub(inner2.clone(), id).await;
            match &res {
                Ok(_) => inner2.set_status(id, AgentStatus::Approved),
                Err(e) => {
                    inner2.log(id, LogKind::Error, format!("{e:#}"));
                    inner2.set_status(id, AgentStatus::Failed);
                }
            }
            let _ = tx.send(true);
            res
        }));
    }

    let mut report = Vec::new();
    for (id, h) in ids.iter().zip(handles) {
        let res = h.await.map_err(|e| anyhow!("crashed: {e}")).and_then(|r| r);
        report.push(report_line(inner, *id, res));
    }
    Ok(report.join("\n\n"))
}

fn report_line(inner: &Inner, id: AgentId, res: Result<Deliverable>) -> String {
    let (key, title, depth, cost) = inner.agent(id, |a| {
        (a.key.clone(), a.title.clone(), a.depth, a.cost_usd)
    });
    let s = inner.state.lock().unwrap();
    let spent = s.subtree_cost(id).max(cost);
    drop(s);
    match res {
        Ok(d) if depth > inner.cfg.orchestrator.human_review_depth => format!(
            "## `{key}` {title}: SUBMITTED FOR YOUR REVIEW (${spent:.3})\n{}\nFiles: {}\nVerify it (read the files, run the checks). If it falls short, call revise_agent with specific feedback.",
            d.summary,
            d.files.join(", ")
        ),
        Ok(d) => format!("## `{key}` {title}: APPROVED by the user (${spent:.3})\n{}\nFiles: {}", d.summary, d.files.join(", ")),
        Err(e) => format!("## `{key}` {title}: FAILED (${spent:.3})\n{e:#}"),
    }
}

/// Reopen one of the caller's own reports with feedback and wait for its
/// revised deliverable.
async fn revise(inner: &Arc<Inner>, caller: AgentId, input: &Value) -> Result<String> {
    let key = input["key"].as_str().context("missing `key`")?;
    let feedback = input["feedback"].as_str().context("missing `feedback`")?;
    let id = {
        let s = inner.state.lock().unwrap();
        s.agents
            .iter()
            .find(|a| a.key == key && a.parent == Some(caller))
            .map(|a| a.id)
            .ok_or_else(|| anyhow!("`{key}` is not one of your reports"))?
    };
    let mut c = inner
        .parked
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or_else(|| anyhow!("`{key}` is still running or cannot be reopened"))?;

    let msg = format!("Your manager requested changes: {feedback}\nAddress this, then call submit_deliverable again.");
    inner.log(id, LogKind::User, msg.clone());
    c.messages.push(Message::user_text(msg));
    c.budget_warned_at = None;
    inner.set_status(id, AgentStatus::Running);

    let res = finish(inner, &mut c).await;
    inner.parked.lock().unwrap().insert(id, c);
    match &res {
        Ok(_) => inner.set_status(id, AgentStatus::Approved),
        Err(e) => {
            inner.log(id, LogKind::Error, format!("{e:#}"));
            inner.set_status(id, AgentStatus::Failed);
        }
    }
    Ok(report_line(inner, id, res))
}

fn agent_tools(can_spawn: bool) -> Vec<ToolDef> {
    let mut tools = file_tools();
    if can_spawn {
        tools.push(spawn_tool());
        tools.push(revise_tool());
    }
    tools.push(deliver_tool());
    tools
}

fn revise_tool() -> ToolDef {
    ToolDef {
        name: "revise_agent",
        description: "Send one of your own reports back with feedback. Reopens its conversation, blocks until it resubmits, and returns the revised deliverable.",
        schema: json!({"type": "object", "properties": {
            "key": {"type": "string"},
            "feedback": {"type": "string", "description": "what is wrong and what done looks like"}
        }, "required": ["key", "feedback"]}),
    }
}

fn spawn_tool() -> ToolDef {
    ToolDef {
        name: "spawn_agents",
        description: "Spawn reports that run in parallel (respecting depends_on) and block until each has delivered or failed. Returns each one's deliverable summary. Each agent is routed to its own model and effort level based on its brief.",
        schema: json!({"type": "object", "properties": {
            "agents": {"type": "array", "items": {"type": "object", "properties": {
                "key": {"type": "string", "description": "short unique id, e.g. `api`"},
                "title": {"type": "string"},
                "brief": {"type": "string", "description": "self-contained task: goal, files owned, interfaces to honor, how to verify"},
                "depends_on": {"type": "array", "items": {"type": "string"}, "description": "keys of sibling agents that must finish first"},
                "budget_usd": {"type": "number", "description": "optional spend cap for this agent and anything it delegates"}
            }, "required": ["key", "title", "brief"]}}
        }, "required": ["agents"]}),
    }
}

fn deliver_tool() -> ToolDef {
    ToolDef {
        name: "submit_deliverable",
        description: "Submit your finished work for review (by the user, or by your manager if you are deep in the tree). Blocks until accepted or returned with feedback.",
        schema: json!({"type": "object", "properties": {
            "summary": {"type": "string", "description": "what was built, how it connects to the rest, how to run/verify it"},
            "files": {"type": "array", "items": {"type": "string"}}
        }, "required": ["summary"]}),
    }
}
