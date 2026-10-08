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
    memory: Option<String>,
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
    /// Read positions on the message board, per agent.
    cursors: Mutex<crate::board::Cursors>,
    /// The board's local bridge: (url, token), for CLI agents.
    bridge: Mutex<Option<(String, String)>>,
    changed: async_channel::Sender<()>,
    /// Bumped on every change; remote followers long-poll on it.
    version: watch::Sender<u64>,
    /// The main agent's checkout: the run worktree (`backspace/run`) when
    /// isolated, else the project folder.
    run_dir: PathBuf,
    /// Running workers and triage agents, so Stop can end them.
    tasks: Mutex<HashMap<AgentId, tokio::task::AbortHandle>>,
    /// Wakes the main loop to drop the turn it is in (Stop).
    stop_main: tokio::sync::Notify,
    /// The coding CLI the planner runs on ("claude", "codex"), when it is
    /// not on an API model.
    planner: Option<String>,
    /// What a CLI planner asked for through the bridge during its turn; run
    /// once the turn ends (approvals and dispatches outlast a tool call).
    planner_queue: Mutex<Vec<PlanAction>>,
    /// Per-machine choices: CLI permissions and effort, tracing.
    over: Overrides,
    /// One trace per agent run, a span per tool call.
    recs: Mutex<HashMap<AgentId, crate::trace::Recorder>>,
}

/// A CLI planner's request, run after its turn.
enum PlanAction {
    /// Wait for the user's verdict on this batch of proposed tickets.
    Approve(Vec<String>),
    /// work_tickets with these keys (None: every ready ticket).
    Dispatch(Option<Vec<String>>),
}

/// Per-machine choices that win over the project's config file.
#[derive(Default, Clone, Debug)]
pub struct Overrides {
    /// Model id every worker runs on (e.g. "claude-code" to hand each
    /// ticket to Claude Code). None: the router decides.
    pub worker: Option<String>,
    /// Memory notes (global and this project's) for every agent's brief.
    pub memory: Option<String>,
    /// Model id the planner runs on. A CLI one ("claude-code", "codex")
    /// runs the planner in that CLI; with no API key the worker's CLI (or
    /// the first signed-in one) is used, so a subscription is enough.
    pub planner: Option<String>,
    /// What CLI agents may do without asking: "edits" (default), "auto"
    /// (the CLI's own reviewer decides) or "full".
    pub permission: Option<String>,
    /// Effort for CLI agents: low, medium, high, xhigh, max.
    pub effort: Option<String>,
    /// Where traces go besides this machine.
    pub tracing: Option<crate::trace::Export>,
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
        traces: vec![],
        advisor_calls: 0,
    }
}

/// Which coding CLI the planner runs on, if not an API model. An explicit
/// pick wins; with no API model usable, the workers' CLI or the first
/// signed-in one of Claude Code and Codex stands in, so a subscription is
/// enough to start a project.
fn pick_planner(cfg: &mut Config, over: &Overrides) -> Option<String> {
    // A pinned API model with no key (the default pins Sonnet) would fail every
    // turn: drop it and let the router or a CLI take the lead.
    if cfg.router.pin_main.as_ref().is_some_and(|p| cfg.cli_of(p).is_none() && !cfg.usable_models().iter().any(|m| &m.id == p)) {
        cfg.router.pin_main = None;
    }
    let pin = over.planner.clone().filter(|p| !p.is_empty()).or_else(|| cfg.router.pin_main.clone());
    let cli = |cfg: &Config, m: &str| cfg.cli_usable(m).then(|| cfg.cli_of(m).and_then(|p| p.cli.clone())).flatten();
    match pin {
        Some(p) if cfg.cli_of(&p).is_some() => cli(cfg, &p),
        Some(p) => {
            if cfg.model(&p).is_some() {
                cfg.router.pin_main = Some(p);
            }
            None
        }
        None if cfg.usable_models().is_empty() => {
            let mut options: Vec<String> = cfg.router.pin_sub.clone().into_iter().collect();
            options.extend(["claude-code".to_string(), "codex".to_string()]);
            options.iter().find_map(|m| cli(cfg, m))
        }
        None => None,
    }
}

/// A saved project, reopened: what was mid-flight can't be picked up (its
/// process is gone), so it is marked interrupted and left ready to dispatch
/// again; its branch keeps the work so far. Plans awaiting approval stay
/// pending and can still be approved.
fn restore(mut old: ProjectState, fresh: ProjectState, config_source: Option<PathBuf>) -> ProjectState {
    let mut fresh_main = fresh.agents.into_iter().next().unwrap();
    for a in &mut old.agents {
        if matches!(a.status, AgentStatus::Running | AgentStatus::Queued | AgentStatus::AwaitingApproval) {
            if a.id == MAIN {
                a.status = AgentStatus::Idle;
            } else {
                a.status = AgentStatus::Failed;
                a.log.push(LogEntry { kind: LogKind::System, text: "interrupted: Backspace closed while this ran. Its branch keeps the work so far; ask the main agent to dispatch it again.".into() });
            }
        }
    }
    let mut pending_plan = Vec::new();
    for ap in &mut old.approvals {
        if ap.state == ApprovalState::Pending {
            if ap.kind == ApprovalKind::Plan {
                pending_plan.extend(ap.tickets.iter().cloned());
            } else {
                ap.state = ApprovalState::Rejected { feedback: "interrupted by a restart".into() };
            }
        }
    }
    for t in &mut old.tickets {
        if matches!(t.state, TicketState::Queued | TicketState::InProgress | TicketState::InReview) {
            t.state = TicketState::Failed;
            t.notes.push("interrupted by a restart; dispatch it again".into());
        }
    }
    // The main agent keeps its history; the new notes (git, planner) go after it.
    let main = &mut old.agents[MAIN];
    fresh_main.log.retain(|e| e.kind == LogKind::System);
    main.log.push(LogEntry { kind: LogKind::System, text: format!("reopened: {} tickets, {} waiting on your approval", old.tickets.len(), pending_plan.len()) });
    main.log.extend(fresh_main.log);
    main.worktree = fresh_main.worktree;
    main.branch = fresh_main.branch;
    old.workspace = fresh.workspace;
    old.config_source = config_source;
    old
}

impl Harness {
    /// Open (or create) a project rooted at `workspace`. Owns its own tokio
    /// runtime so any UI toolkit can drive it through plain sync calls.
    pub fn open(workspace: PathBuf) -> Result<Harness> {
        Self::open_with(workspace, Overrides::default())
    }

    /// Open with settings from the desktop app layered over the config file.
    pub fn open_with(workspace: PathBuf, over: Overrides) -> Result<Harness> {
        std::fs::create_dir_all(&workspace)?;
        let root = dunce::canonicalize(&workspace)?;
        let (mut cfg, config_source) = Config::load(&root)?;
        if let Some(w) = over.worker.clone().filter(|w| !w.is_empty()) {
            if cfg.model(&w).is_some() {
                cfg.router.pin_sub = Some(w);
            }
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        let planner = pick_planner(&mut cfg, &over);
        // Auto (the router picks per ticket): the effort slider caps where workers start.
        if over.worker.as_deref().is_none_or(|w| w.is_empty()) {
            if let Some(e) = over.effort.as_ref().and_then(|e| serde_json::from_value::<Effort>(json!(e)).ok()) {
                cfg.escalation.start_max_effort = e;
            }
        }

        let (git_notes, base_branch, run_dir) = if cfg.orchestrator.isolation == Isolation::Worktree {
            let (dir, notes) = rt
                .block_on(git::ensure_run(&root))
                .context("setting up git for worktree isolation (set isolation = \"shared\" to skip)")?;
            (notes, git::RUN_BRANCH.to_string(), dir)
        } else {
            (vec![], String::new(), root.clone())
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
        main.worktree = Some(run_dir.clone());
        if let Some(cli) = &planner {
            main.log.push(LogEntry { kind: LogKind::System, text: format!("the planner runs on {cli}, on your own plan") });
        }
        for n in git_notes {
            main.log.push(LogEntry { kind: LogKind::System, text: n });
        }
        let fresh = ProjectState {
            name,
            workspace: root.clone(),
            agents: vec![main],
            approvals: vec![],
            tickets: vec![],
            total_cost_usd: 0.0,
            router_cost_usd: 0.0,
            config_source: config_source.clone(),
            board: vec![],
            planner_session: None,
            canvas_ops: vec![],
        };
        // A project opened before carries on: tickets, approvals, spend and
        // the board come back; what was running is marked interrupted.
        let state = std::fs::read_to_string(root.join(".backspace/state.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<ProjectState>(&t).ok())
            .filter(|old| !old.agents.is_empty())
            .map(|old| restore(old, fresh.clone(), config_source))
            .unwrap_or(fresh);

        let (changed_tx, changed_rx) = async_channel::bounded(1);
        let inner = Arc::new(Inner {
            router: Router::new(http.clone(), cfg.clone()),
            agents_md: std::fs::read_to_string(root.join("AGENTS.md")).ok(),
            memory: over.memory.clone(),
            skills: Skills::load(&root),
            limiter: Semaphore::new(cfg.orchestrator.max_parallel_calls),
            state: Mutex::new(state),
            verdicts: Mutex::new(HashMap::new()),
            done: Mutex::new(HashMap::new()),
            parked: Mutex::new(HashMap::new()),
            git: tokio::sync::Mutex::new(()),
            cursors: Mutex::new(Default::default()),
            bridge: Mutex::new(None),
            changed: changed_tx,
            version: watch::channel(1).0,
            base_branch,
            root,
            cfg,
            http,
            run_dir,
            tasks: Mutex::new(HashMap::new()),
            stop_main: tokio::sync::Notify::new(),
            planner,
            planner_queue: Mutex::new(vec![]),
            over,
            recs: Mutex::new(HashMap::new()),
        });
        inner.save();

        // The board's bridge for agents in CLIs. Failing to bind only
        // costs them messaging.
        if let Ok(l) = std::net::TcpListener::bind("127.0.0.1:0") {
            let token = crate::remote::new_token();
            if let Ok(addr) = l.local_addr() {
                *inner.bridge.lock().unwrap() = Some((format!("http://{addr}"), token.clone()));
                let _ = l.set_nonblocking(true);
                let i = inner.clone();
                rt.spawn(async move {
                    if let Ok(l) = tokio::net::TcpListener::from_std(l) {
                        while let Ok((s, _)) = l.accept().await {
                            let (i, t) = (i.clone(), token.clone());
                            tokio::spawn(async move {
                                let _ = bridge_handle(s, i, &t).await;
                            });
                        }
                    }
                });
            }
        }

        let (to_main, inbox) = mpsc::unbounded_channel();
        rt.spawn(main_loop(inner.clone(), inbox));
        Ok(Harness {
            inner,
            to_main,
            changed: changed_rx,
            rt,
        })
    }

    /// The message board's local bridge (url, token), if it started.
    pub fn bridge(&self) -> Option<(String, String)> {
        self.inner.bridge.lock().unwrap().clone()
    }

    /// Post on the agents' message board as the human ("you").
    pub fn post_message(&self, to: &str, text: &str) -> Result<()> {
        self.inner.post_board("you", to, text)
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

    /// The project's root (the primary worktree).
    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    pub fn list_files(&self, agent: usize) -> Vec<crate::files::FileEntry> {
        let root = {
            let s = self.inner.state.lock().unwrap();
            s.agents
                .get(agent)
                .and_then(|a| a.worktree.clone())
                .unwrap_or_else(|| s.workspace.clone())
        };
        crate::files::scan(&root)
    }

    pub fn read_file(&self, path: &str) -> Result<String> {
        crate::files::read_within(&self.inner.root, path)
    }

    /// Share this harness over HTTP (see `remote`). Stops when the handle
    /// is aborted or the harness is dropped.
    pub fn serve(&self, addr: &str, token: &str) -> Result<tokio::task::AbortHandle> {
        let api: Arc<dyn crate::remote::Api> = Arc::new(Served {
            inner: self.inner.clone(),
            to_main: self.to_main.clone(),
        });
        // Bind first so a busy port is reported, not swallowed by the task.
        let probe = self.rt.block_on(tokio::net::TcpListener::bind(addr));
        drop(probe.with_context(|| format!("binding {addr}"))?);
        let task = self.rt.spawn(crate::remote::serve(
            api,
            addr.to_string(),
            token.to_string(),
        ));
        Ok(task.abort_handle())
    }

    fn resolve(&self, approval: usize, verdict: Verdict) {
        resolve(&self.inner, &self.to_main, approval, verdict);
    }

    /// Start another task in parallel; returns its ticket key.
    pub fn start_task(&self, text: &str) -> Result<String> {
        start_task(&self.inner, self.rt.handle(), text)
    }

    /// Stop an agent and everything under it (the main agent: the whole
    /// project). Its process is killed; its ticket is left failed, so it can
    /// be dispatched again, and its branch keeps what it did.
    pub fn stop(&self, agent: Option<AgentId>) {
        stop(&self.inner, agent);
    }
}

fn resolve(inner: &Arc<Inner>, to_main: &mpsc::UnboundedSender<String>, approval: usize, verdict: Verdict) {
    if let Some(tx) = inner.verdicts.lock().unwrap().remove(&approval) {
        let _ = tx.send(verdict);
        return;
    }
    // A plan proposed before a restart: nobody is waiting on it any
    // more, so settle it here and tell the main agent.
    let plan = inner.state.lock().unwrap().approvals.get(approval).cloned()
        .filter(|a| a.kind == ApprovalKind::Plan && a.state == ApprovalState::Pending);
    let Some(ap) = plan else { return };
    let keys = ap.tickets.join(", ");
    let msg = match &verdict {
        Verdict::Approved => {
            for k in &ap.tickets {
                inner.ticket(k, |t| t.state = TicketState::ReadyForAgent);
            }
            format!("The user approved the plan you proposed before Backspace restarted ({keys}). Dispatch it with work_tickets.")
        }
        Verdict::Rejected(f) => {
            inner.update(|s| s.tickets.retain(|t| !ap.tickets.contains(&t.key)));
            format!("The user rejected the plan you proposed before Backspace restarted ({keys}); those tickets were discarded. Feedback: {f}\nRevise it and call create_tickets again.")
        }
    };
    inner.update(|s| {
        s.approvals[approval].state = match verdict {
            Verdict::Approved => ApprovalState::Approved,
            Verdict::Rejected(feedback) => ApprovalState::Rejected { feedback },
        }
    });
    inner.save();
    let _ = to_main.send(msg);
}

fn stop(inner: &Arc<Inner>, agent: Option<AgentId>) {
    let ids: Vec<AgentId> = {
        let s = inner.state.lock().unwrap();
        s.agents
            .iter()
            .filter(|a| agent.is_none_or(|id| s.ancestry(a.id).contains(&id)))
            .filter(|a| !a.status.is_terminal() && a.status != AgentStatus::Idle)
            .map(|a| a.id)
            .collect()
    };
    // Only a turn in progress: a stored wake-up would cut the next one short.
    if agent.is_none_or(|id| id == MAIN) && ids.contains(&MAIN) {
        inner.stop_main.notify_one();
    }
    for id in ids.into_iter().filter(|&id| id != MAIN) {
        if let Some(h) = inner.tasks.lock().unwrap().remove(&id) {
            h.abort();
        }
        // Its approval, if it was waiting on one, goes too.
        let open: Vec<usize> = inner.state.lock().unwrap().approvals.iter()
            .filter(|a| a.agent == id && a.state == ApprovalState::Pending).map(|a| a.id).collect();
        for ap in open {
            inner.verdicts.lock().unwrap().remove(&ap);
            inner.update(|s| s.approvals[ap].state = ApprovalState::Rejected { feedback: "stopped".into() });
        }
        let key = inner.state.lock().unwrap().agents[id].ticket.clone();
        inner.log(id, LogKind::System, "stopped by you");
        inner.set_status(id, AgentStatus::Failed);
        if let Some(k) = key {
            inner.ticket(&k, |t| {
                t.state = TicketState::Failed;
                t.notes.push("stopped by the user".into());
            });
        }
        inner.trace_finish(id, Some("stopped".into()));
    }
    inner.save();
}

struct Served {
    inner: Arc<Inner>,
    to_main: mpsc::UnboundedSender<String>,
}

impl crate::remote::Api for Served {
    fn version(&self) -> watch::Receiver<u64> {
        self.inner.version.subscribe()
    }
    fn snapshot(&self) -> ProjectState {
        self.inner.state.lock().unwrap().clone()
    }
    fn send(&self, text: String) {
        let _ = self.to_main.send(text);
    }
    fn resolve(&self, id: usize, feedback: Option<String>) {
        let verdict = match feedback {
            None => Verdict::Approved,
            Some(f) => Verdict::Rejected(f),
        };
        resolve(&self.inner, &self.to_main, id, verdict);
    }
    fn stop(&self, agent: Option<usize>) {
        stop(&self.inner, agent);
    }
    fn start_task(&self, text: &str) -> Result<String> {
        // `serve` answers on the harness's runtime.
        start_task(&self.inner, &tokio::runtime::Handle::current(), text)
    }
    fn file_ticket(&self, title: &str, body: &str) -> Result<String> {
        file_ticket(
            &self.inner,
            MAIN,
            MAIN,
            &json!({"title": title, "what_to_build": body}),
        )
    }
    fn list_files(&self, agent: usize) -> Vec<crate::files::FileEntry> {
        let root = {
            let s = self.inner.state.lock().unwrap();
            s.agents
                .get(agent)
                .and_then(|a| a.worktree.clone())
                .unwrap_or_else(|| s.workspace.clone())
        };
        crate::files::scan(&root)
    }
    fn read_file(&self, path: &str) -> Result<String> {
        crate::files::read_within(&self.inner.root, path)
    }
    fn post_message(&self, to: &str, text: &str) -> Result<()> {
        self.inner.post_board("you", to, text)
    }
}

impl Inner {
    fn update<R>(&self, f: impl FnOnce(&mut ProjectState) -> R) -> R {
        let r = f(&mut self.state.lock().unwrap());
        self.touch();
        r
    }

    fn touch(&self) {
        let _ = self.changed.try_send(());
        self.version.send_modify(|v| *v += 1);
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

    /// An agent opens or closes a canvas; the shell applies it.
    fn canvas(&self, by: &str, op: &str, input: &Value) -> Result<String> {
        let kind = input["kind"].as_str().unwrap_or("");
        if !["browser", "files", "agent", "diagram", "docs", "board"].contains(&kind) {
            bail!("unknown canvas kind `{kind}`");
        }
        let url = input["url"].as_str().filter(|u| !u.is_empty()).map(String::from);
        if kind == "browser" && op == "open" && !url.as_deref().is_some_and(|u| u.starts_with("http://") || u.starts_with("https://")) {
            bail!("a browser canvas needs an http(s) url");
        }
        let agent = Some(input["agent"].as_str().filter(|a| !a.is_empty()).unwrap_or(by).to_string());
        self.update(|s| {
            let id = s.canvas_ops.len() as u64 + 1;
            s.canvas_ops.push(CanvasOp { id, op: op.into(), kind: kind.into(), agent, url, by: by.into() });
        });
        Ok(format!("{op}ed a {kind} canvas in the user's window"))
    }

    /// Start an agent's trace (one per worker run, one per message to main).
    fn trace_start(&self, id: AgentId, name: &str, input: &str) {
        let (project, key, model) = {
            let s = self.state.lock().unwrap();
            let a = &s.agents[id];
            (s.name.clone(), a.key.clone(), a.decision.as_ref().map(|d| d.model.clone()))
        };
        let mut attrs = std::collections::BTreeMap::new();
        attrs.insert("backspace.project".into(), json!(project));
        attrs.insert("backspace.agent".into(), json!(key));
        if let Some(m) = model {
            attrs.insert("gen_ai.request.model".into(), json!(m));
        }
        let rec = crate::trace::Recorder::start(&format!("project:{project}"), &key, name, attrs, input);
        self.recs.lock().unwrap().insert(id, rec);
    }

    fn trace_event(&self, id: AgentId, e: &backspace_runner::Event) {
        if let Some(r) = self.recs.lock().unwrap().get_mut(&id) {
            r.on(e);
        }
    }

    /// Close an agent's trace: kept in `<data>/traces`, sent on if Settings
    /// → Tracing says so, and listed on the agent.
    fn trace_finish(&self, id: AgentId, error: Option<String>) {
        let Some(rec) = self.recs.lock().unwrap().remove(&id) else { return };
        let t = rec.finish(error);
        let data = crate::prefs::Prefs::data_dir();
        if crate::trace::save(&data, &t).is_ok() {
            self.agent(id, |a| a.traces.push(t.trace_id.clone()));
        }
        if let Some(cfg) = self.over.tracing.clone().filter(|c| !c.endpoint.is_empty()) {
            let http = self.http.clone();
            tokio::spawn(async move {
                let _ = crate::trace::export(&http, &cfg, &t).await;
            });
        }
    }

    /// Post on the board and note it in both agents' logs.
    fn post_board(&self, from: &str, to: &str, text: &str) -> Result<()> {
        let (m, ids) = {
            let mut s = self.state.lock().unwrap();
            let m = crate::board::post(&mut s, from, to, text)?;
            let ids: Vec<(AgentId, bool)> = s
                .agents
                .iter()
                .filter(|a| a.key == from || a.key == to || (to == "all" && a.key != from))
                .map(|a| (a.id, a.key == from))
                .collect();
            (m, ids)
        };
        for (id, sender) in ids {
            let line = if sender {
                format!("→ {}: {}", m.to, m.text)
            } else {
                format!("← {}: {}", m.from, m.text)
            };
            self.log(id, LogKind::System, line);
        }
        self.touch();
        Ok(())
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
            AgentKind::Scout => SCOUT_PROMPT.to_string(),
        };
        if matches!(kind, AgentKind::Main | AgentKind::Worker) {
            s.push_str(&self.team_notes());
        }
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
            AgentKind::Scout => &wf.triage_skills,
        };
        for name in injected {
            if name == "grilling" && !wf.grill {
                continue;
            }
            if let Some(body) = self.skills.read(name, None) {
                s.push_str(&format!("\n\n# Skill: {name}\n\n{body}"));
            }
        }
        if let Some(m) = &self.memory {
            s.push_str("\n\n");
            s.push_str(m);
        }
        if let Some(md) = &self.agents_md {
            s.push_str("\n\n# Project instructions (AGENTS.md)\n\n");
            s.push_str(md);
        }
        s
    }

    /// How the lead and workers use the advisor and scouts, appended to
    /// their brief. The same text goes to agents in CLIs (cli_team_notes).
    fn team_notes(&self) -> String {
        let mut s = String::new();
        if let Some(a) = &self.cfg.advisor.model {
            s.push_str(&format!("\n\n# Your advisor\n\nA stronger model ({a}) is on call through the `advisor` tool; it reads this whole conversation. Consult it at three moments and stay on your own otherwise:\n- before a plan locks (before create_tickets, or before a large change): does it miss auth invariants, schema contracts or edge cases?\n- when the same check or error fails twice: is this the root cause, or a rabbit hole?\n- before you call it done (submit_deliverable): did the full diff break anything or skip a pre-flight rule?\nNever for routine steps."));
        }
        s.push_str(&format!("\n\n# Scouts\n\nFor looking things up (which files matter, how an API works, what the docs say), send scouts with the `scout` tool: up to {} small fast read-only agents in parallel, each returning a short summary. It is cheaper than reading a lot yourself.", self.cfg.advisor.scouts.max(1)));
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
    if kind == AgentKind::Scout {
        tools.retain(|t| t.name == "read" || t.name == "bash");
        return tools;
    }
    tools.push(skill_tool());
    if can_spawn {
        tools.extend([create_tickets_tool(), work_tickets_tool(), revise_tool()]);
    }
    tools.push(scout_tool());
    tools.extend(canvas_tools());
    tools.push(file_ticket_tool());
    tools.extend(board_tools());
    tools.push(deliver_tool());
    tools
}

pub(crate) fn scout_tool() -> ToolDef {
    ToolDef {
        name: "scout",
        description: "Send fast read-only scouts (a small model) to look things up in parallel: which files matter, how something works, what the docs or specs say. Each task gets its own scout and comes back as a short summary.",
        schema: json!({"type": "object", "properties": {"tasks": {"type": "array", "items": {"type": "string"}, "description": "one question per scout, specific enough to answer alone"}}, "required": ["tasks"]}),
    }
}

/// Opening and closing canvases in the user's window: a browser on the dev
/// server you started, your worktree's files, someone's session.
pub(crate) fn canvas_tools() -> Vec<ToolDef> {
    let props = json!({
        "kind": {"type": "string", "enum": ["browser", "files", "agent", "diagram", "docs", "board"], "description": "browser: a web page (give url); files: a worktree; agent: an agent's session; diagram: the review; docs: PLAN.md; board: team chat"},
        "url": {"type": "string", "description": "for browser, e.g. http://localhost:3000"},
        "agent": {"type": "string", "description": "for files and agent: whose (an agent key); default yourself"}
    });
    vec![
        ToolDef {
            name: "open_canvas",
            description: "Open a canvas in the user's window so they can watch something: your dev server in a browser, your files, a session. Use it when seeing it helps the user; not for routine steps.",
            schema: json!({"type": "object", "properties": props, "required": ["kind"]}),
        },
        ToolDef {
            name: "close_canvas",
            description: "Close a canvas you opened (same kind and url or agent) when it is no longer useful.",
            schema: json!({"type": "object", "properties": props, "required": ["kind"]}),
        },
    ]
}

fn board_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_agents",
            description: "The agents on this project: key, title, status, and the model or CLI running them.",
            schema: json!({"type": "object", "properties": {}}),
        },
        ToolDef {
            name: "send_message",
            description: "Message another agent by key, `main` (the lead) or `all`, whichever model or CLI runs it. Coordinate interfaces, shared files, or ask whoever owns something. Replies arrive in your next turn.",
            schema: json!({"type": "object", "properties": {"to": {"type": "string"}, "text": {"type": "string"}}, "required": ["to", "text"]}),
        },
        ToolDef {
            name: "read_messages",
            description: "New messages to you or to everyone. They also arrive on their own at the start of a turn.",
            schema: json!({"type": "object", "properties": {}}),
        },
    ]
}

async fn main_loop(inner: Arc<Inner>, mut inbox: mpsc::UnboundedReceiver<String>) {
    let mut convo: Option<Conversation> = load_main(&inner);
    while let Some(text) = inbox.recv().await {
        inner.log(MAIN, LogKind::User, text.clone());
        inner.set_status(MAIN, AgentStatus::Running);
        inner.trace_start(MAIN, "turn · main", &text);
        let turn = async {
            if let Some(cli) = inner.planner.clone() {
                return plan_on_cli(&inner, &cli, text).await.map(|_| TurnEnd::Replied);
            }
            if convo.is_none() {
                let d = inner.router.route(AgentRole::Main, &text, "").await?;
                announce_route(&inner, MAIN, &d);
                inner.agent(MAIN, |a| a.brief = text.clone());
                convo = Some(main_convo(&inner, d, vec![]));
            }
            let c = convo.as_mut().unwrap();
            match c.messages.last_mut() {
                // After a Stop the last message can be yours, unanswered.
                Some(m) if m.role == Role::User => m.content.push(Block::Text { text }),
                _ => c.messages.push(Message::user_text(text)),
            }
            drive(&inner, c).await
        };
        // Stop drops the turn where it is (and any agents it was waiting on
        // are stopped by `stop`).
        let res = tokio::select! {
            r = turn => Some(r),
            _ = inner.stop_main.notified() => None,
        };
        match res {
            Some(Ok(TurnEnd::Delivered(d))) => {
                inner.agent(MAIN, |a| a.deliverable = Some(d));
                inner.set_status(MAIN, AgentStatus::Approved);
                inner.trace_finish(MAIN, None);
            }
            Some(Ok(TurnEnd::Replied)) => {
                inner.set_status(MAIN, AgentStatus::Idle);
                inner.trace_finish(MAIN, None);
            }
            Some(Err(e)) => {
                inner.log(MAIN, LogKind::Error, format!("{e:#}"));
                inner.set_status(MAIN, AgentStatus::Idle);
                inner.trace_finish(MAIN, Some(format!("{e:#}")));
            }
            None => {
                if let Some(c) = convo.as_mut() {
                    close_open_calls(c);
                }
                inner.planner_queue.lock().unwrap().clear();
                inner.log(MAIN, LogKind::System, "stopped by you");
                inner.set_status(MAIN, AgentStatus::Idle);
                inner.trace_finish(MAIN, Some("stopped".into()));
            }
        }
        if let Some(c) = &convo {
            save_main(&inner, c);
        }
    }
}

/// The main agent's conversation, on an API model.
fn main_convo(inner: &Inner, d: Decision, messages: Vec<Message>) -> Conversation {
    Conversation {
        id: MAIN,
        kind: AgentKind::Main,
        ticket: None,
        can_spawn: true,
        ws: inner.workspace(&inner.run_dir),
        initial: d.clone(),
        system: inner.system_prompt(AgentKind::Main, 0, &d),
        decision: d,
        messages,
        tools: tools_for(AgentKind::Main, true),
        budget_warned_at: None,
        turns_total: 0,
        recent_calls: vec![],
    }
}

/// A turn cut off mid tool call leaves calls without results, which the
/// model's API rejects next time: answer them as stopped.
fn close_open_calls(c: &mut Conversation) {
    let Some(last) = c.messages.last() else { return };
    if last.role != Role::Assistant {
        return;
    }
    let blocks: Vec<Block> = last
        .content
        .iter()
        .filter_map(|b| match b {
            Block::ToolUse { id, .. } => Some(Block::ToolResult { tool_use_id: id.clone(), content: "stopped by the user".into(), is_error: true }),
            _ => None,
        })
        .collect();
    if !blocks.is_empty() {
        c.messages.push(Message { role: Role::User, content: blocks });
    }
}

/// The main agent's conversation survives a restart.
fn save_main(inner: &Inner, c: &Conversation) {
    let v = json!({"decision": c.decision, "messages": c.messages});
    let _ = std::fs::write(inner.root.join(".backspace/main.json"), v.to_string());
}

fn load_main(inner: &Inner) -> Option<Conversation> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(inner.root.join(".backspace/main.json")).ok()?).ok()?;
    let d: Decision = serde_json::from_value(v["decision"].clone()).ok()?;
    let messages: Vec<Message> = serde_json::from_value(v["messages"].clone()).ok()?;
    let mut c = main_convo(inner, d, messages);
    close_open_calls(&mut c);
    Some(c)
}

/// One message to a planner that lives in a coding CLI. It plans with
/// Backspace's tools over MCP; approvals and dispatches outlast a tool call,
/// so they are queued and run after its turn, and what came of them is its
/// next prompt (on the same session) until it answers without asking for
/// anything.
async fn plan_on_cli(inner: &Arc<Inner>, cli: &str, text: String) -> Result<()> {
    let bin = crate::harnesses::which(crate::harnesses::bin_for(cli))
        .ok_or_else(|| anyhow!("`{cli}` is not installed on this machine"))?;
    let mut prompt = text;
    loop {
        let session = inner.state.lock().unwrap().planner_session.clone();
        let system = session.is_none().then(|| planner_system(inner));
        let (_, sid) = run_cli_turn(inner, MAIN, cli, &bin, &inner.run_dir.clone(), &prompt, session.as_deref(), system.as_deref()).await?;
        if sid.is_some() {
            inner.update(|s| s.planner_session = sid);
            inner.save();
        }
        let actions = std::mem::take(&mut *inner.planner_queue.lock().unwrap());
        if actions.is_empty() {
            return Ok(());
        }
        let mut out = Vec::new();
        for a in actions {
            out.push(match a {
                PlanAction::Approve(keys) => approve_plan(inner, MAIN, keys).await,
                PlanAction::Dispatch(keys) => work_tickets(inner, MAIN, &json!({ "keys": keys }))
                    .await
                    .unwrap_or_else(|e| format!("work_tickets failed: {e:#}")),
            });
        }
        prompt = out.join("\n\n");
        inner.log(MAIN, LogKind::ToolResult, prompt.clone());
    }
}

/// The advisor and scouts as a CLI agent has them (Claude Code: `--advisor`
/// and the `scout` subagent).
fn cli_team_notes(inner: &Inner) -> String {
    let a = &inner.cfg.advisor;
    if a.cli.is_none() && a.cli_scouts.is_none() {
        return String::new();
    }
    "\n\n# Advisor and scouts\n\nIf you have an advisor tool, consult it at three moments only: before a plan locks, when the same check or error fails twice, and before you call the work done. For discovery (which files matter, what the docs say), send the `scout` subagent, several in parallel, rather than reading a lot yourself.".to_string()
}

/// The planner's brief in a CLI: the API planner's, plus how its tools work there.
fn planner_system(inner: &Inner) -> String {
    let d = Decision {
        model: "cli".into(),
        effort: inner.cfg.router.main_min_effort,
        source: "pinned".into(),
        confidence: 1.0,
        router_cost_usd: 0.0,
        note: None,
    };
    format!(
        "{}{}\n\n# Your tools here\n\nYou run inside a coding CLI. Backspace's planning tools are MCP tools named `create_tickets` and `work_tickets` (server `backspace`). Both return at once: after calling either, end your turn with a one-line note. The user's verdict on a plan, or each ticket's result, comes back as your next message. Do not build the tickets yourself; agents do that on their own branches, and their accepted work is merged into this checkout (branch `{}`), where you can read and verify it.",
        inner.system_prompt(AgentKind::Main, 0, &d),
        cli_team_notes(inner),
        inner.base_branch
    )
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

/// A worker on a coding CLI: run it headless in the ticket's worktree, then
/// submit what it changed through the same commit / check / review / merge
/// path as any worker. Feedback (a failed check, a rejection, a merge
/// conflict) goes back to the CLI as the next prompt, resuming its session
/// where the CLI can.
async fn finish_cli(
    inner: &Arc<Inner>,
    c: &mut Conversation,
    cli: &str,
    dir: &Path,
    brief: &str,
) -> Result<Deliverable> {
    let bin = crate::harnesses::which(crate::harnesses::bin_for(cli))
        .ok_or_else(|| anyhow!("`{cli}` is not installed on this machine"))?;
    inner.log(
        c.id,
        LogKind::System,
        format!(
            "handing this ticket to {cli} ({}) in {}",
            bin.display(),
            dir.display()
        ),
    );
    let me = inner.state.lock().unwrap().agents[c.id].key.clone();
    let exe = std::env::var("BACKSPACE_EXE")
        .ok()
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .map(|p| p.display().to_string())
        })
        .unwrap_or_else(|| "backspace".into());
    let mut prompt = format!(
        "{brief}{}\n\n# How to work\n\nYou are one worker in a team; this directory is your own git worktree for this ticket. Make the change, run the project's tests or checks if it has any, and fix what fails. Do not commit or push: Backspace commits your work and sends it for review. When you are done, reply with a short summary of what you changed; its first line is the one-line summary.\n\n# Talking to the team\n\nOther agents work on other tickets at the same time, some on other models or CLIs. Your key is `{me}`. From your shell:\n- `{exe} msg agents` lists them\n- `{exe} msg send <key|main|all> \"text\"` messages one (or everyone)\n- `{exe} msg read` shows messages for you\nCheck messages before you finish, and tell others about interfaces or shared files you change.",
        cli_team_notes(inner)
    );
    let mut session: Option<String> = None;
    for round in 0..4 {
        let (summary, sid) =
            run_cli_turn(inner, c.id, cli, &bin, dir, &prompt, session.as_deref(), None).await?;
        session = sid.or(session);
        let summary = if summary.trim().is_empty() {
            format!("{cli} finished without a summary")
        } else {
            summary
        };
        match submit(inner, c, &json!({ "summary": summary })).await? {
            ToolOutcome::Delivered(d, note) => {
                inner.log(c.id, LogKind::System, note);
                return Ok(d);
            }
            ToolOutcome::Failure(msg, _) | ToolOutcome::Text(msg) => {
                inner.log(
                    c.id,
                    LogKind::System,
                    format!("round {}: {}", round + 1, truncate(&msg, 400)),
                );
                prompt = msg;
                if session.is_none() {
                    prompt = format!("{brief}\n\n# Feedback on your last attempt\n\n{prompt}");
                }
            }
        }
    }
    bail!("{cli} did not get the ticket accepted in 4 rounds")
}

/// A CLI that prints nothing for this long is stuck: it is stopped.
// ponytail: one fixed limit; make it a setting if long silent builds need more.
const CLI_STALL: Duration = Duration::from_secs(10 * 60);

/// One headless run of a CLI through `backspace-runner`. Its text, tool
/// calls and results go to the agent's log and trace as they happen.
/// Returns its final message and the session to resume, if it has one.
#[allow(clippy::too_many_arguments)]
async fn run_cli_turn(
    inner: &Arc<Inner>,
    id: AgentId,
    cli: &str,
    bin: &Path,
    dir: &Path,
    prompt: &str,
    session: Option<&str>,
    system: Option<&str>,
) -> Result<(String, Option<String>)> {
    use backspace_runner::{Event, Request, Target, Turn};
    let me = inner.state.lock().unwrap().agents[id].key.clone();
    let exe = std::env::var("BACKSPACE_EXE")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    let bridge = inner.bridge.lock().unwrap().clone();
    let mut req = Request::new(
        Target::Cli { provider: cli.into(), bin: bin.to_path_buf() },
        vec![Turn::user(prompt)],
    );
    req.cwd = dir.to_path_buf();
    req.edit = true;
    req.session = session.map(String::from);
    req.system = system.map(String::from);
    req.permission = inner.over.permission.clone();
    req.effort = inner.over.effort.clone();
    if cli == "claude" {
        // Orchestrator, advisor, scouts, in Claude Code's own terms.
        req.advisor = inner.cfg.advisor.cli.clone();
        if let Some(m) = &inner.cfg.advisor.cli_scouts {
            req.agents = Some(json!({"scout": {
                "description": "Fast read-only scout: finds the files that matter, reads code, docs and specs, and returns a short summary. Use it for discovery instead of reading a lot yourself; send several in parallel.",
                "prompt": SCOUT_PROMPT,
                "model": m,
                "tools": ["Read", "Grep", "Glob", "WebFetch"],
            }}));
        }
    }
    req.env.push(("PATH".into(), crate::harnesses::path_env().to_string_lossy().into_owned()));
    if let Some((url, token)) = &bridge {
        // The board, for `backspace msg` and the MCP server.
        let vars = [(crate::board::ENV_URL, url.as_str()), (crate::board::ENV_TOKEN, token.as_str()), (crate::board::ENV_AGENT, me.as_str())];
        req.env.extend(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        if let Some(exe) = &exe {
            let env: serde_json::Map<String, Value> = vars.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
            req.mcp = Some(json!({"mcpServers": {"backspace": {"command": exe.display().to_string(), "args": ["mcp"], "env": env}}}));
            req.mcp_allow = vec!["mcp__backspace".into()];
        }
    }
    inner.set_status(id, AgentStatus::Running);

    let last = Arc::new(Mutex::new(std::time::Instant::now()));
    // What the turn said: the text since the last tool call, the last
    // paragraph logged, the CLI's own final answer, its session.
    #[derive(Default)]
    struct Said {
        buf: String,
        last: String,
        final_text: Option<String>,
        sid: Option<String>,
    }
    impl Said {
        fn flush(&mut self, inner: &Inner, id: AgentId) {
            let t = self.buf.trim().to_string();
            if !t.is_empty() {
                inner.log(id, LogKind::Assistant, t.clone());
                self.last = t;
            }
            self.buf.clear();
        }
    }
    let mut st = Said::default();
    let res = {
        let (inner2, last2) = (inner.clone(), last.clone());
        let st = &mut st;
        let mut sink = |e: Event| {
            *last2.lock().unwrap() = std::time::Instant::now();
            inner2.trace_event(id, &e);
            match &e {
                Event::Text { text } => st.buf.push_str(text),
                Event::Replace { text } => st.buf = text.clone(),
                Event::Final { text } => st.final_text = Some(text.clone()),
                Event::Session { id } => st.sid = Some(id.clone()),
                Event::Break => st.flush(&inner2, id),
                Event::ToolStart { name, input, .. } => {
                    st.flush(&inner2, id);
                    let v: Value = serde_json::from_str(input).unwrap_or(Value::Null);
                    let what = ["command", "file_path", "path", "pattern"]
                        .iter()
                        .find_map(|k| v[*k].as_str())
                        .map(String::from)
                        .unwrap_or_else(|| if v.is_null() { input.clone() } else { String::new() });
                    inner2.log(id, LogKind::ToolCall, format!("{name} {}", truncate(&what, 200)));
                }
                Event::ToolEnd { output, error, .. } => {
                    if !output.trim().is_empty() {
                        inner2.log(id, if *error { LogKind::Error } else { LogKind::ToolResult }, truncate(output, 800));
                    }
                }
                Event::Usage { input, output } => inner2.agent(id, |a| {
                    a.input_tokens = a.input_tokens.max(*input);
                    a.output_tokens = a.output_tokens.max(*output);
                }),
                Event::Cost { usd } => inner2.log(id, LogKind::System, format!("{cli} turn done (≈${usd:.3} at API prices; billed to your {cli} plan)")),
                Event::Model { .. } => {}
            }
        };
        let run = backspace_runner::run(&inner.http, &req, &mut sink);
        tokio::pin!(run);
        loop {
            tokio::select! {
                r = &mut run => break r,
                _ = tokio::time::sleep(Duration::from_secs(20)) => {
                    if last.lock().unwrap().elapsed() > CLI_STALL {
                        break Err(anyhow!("{cli} printed nothing for {} minutes; stopped it", CLI_STALL.as_secs() / 60));
                    }
                }
            }
        }
    };
    st.flush(inner, id);
    res.map_err(|e| anyhow!("{cli}: {}", truncate(&format!("{e:#}"), 600)))?;
    Ok((st.final_text.filter(|t| !t.trim().is_empty()).unwrap_or(st.last), st.sid))
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

        // Messages from other agents join this turn's input.
        let news = {
            let s = inner.state.lock().unwrap();
            let me = s.agents[c.id].key.clone();
            inner.cursors.lock().unwrap().drain(&s, &me)
        };
        if !news.is_empty() {
            let refs: Vec<&crate::board::BoardMsg> = news.iter().collect();
            let note = format!(
                "Messages from other agents:\n{}",
                crate::board::format(&refs)
            );
            match c.messages.last_mut() {
                Some(m) if m.role == Role::User => m.content.push(Block::Text { text: note }),
                _ => c.messages.push(Message::user_text(note)),
            }
        }

        let spec = inner
            .cfg
            .model(&c.decision.model)
            .ok_or_else(|| anyhow!("routed to unknown model {}", c.decision.model))?
            .clone();
        let provider_cfg = inner.cfg.providers[&spec.provider].clone();
        // The advisor: Anthropic only, and only for an executor it can rank
        // above (an Opus advisor can't advise Fable). Scouts work without one.
        let advisor = inner.cfg.advisor.model.clone().filter(|a| {
            provider_cfg.kind == crate::config::ProviderKind::Anthropic
                && c.kind != AgentKind::Scout
                && !spec.wire_model().contains("fable")
                && a != spec.wire_model()
        });
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
                    advisor: advisor.as_deref(),
                },
            )
            .await?
        };

        let mut cost = spec.cost(completion.input_tokens, completion.output_tokens);
        let (ai, ao) = completion.advisor_tokens;
        if ai + ao > 0 {
            if let Some(m) = advisor.as_deref().and_then(|a| inner.cfg.models.iter().find(|m| m.wire_model() == a)) {
                cost += m.cost(ai, ao);
            }
            inner.log(c.id, LogKind::System, format!("consulted the advisor ({}): {}k tokens read", advisor.as_deref().unwrap_or("advisor"), ai / 1000));
            inner.agent(c.id, |a| a.advisor_calls += 1);
        }
        inner.update(|s| {
            let a = &mut s.agents[c.id];
            a.cost_usd += cost;
            a.input_tokens += completion.input_tokens;
            a.output_tokens += completion.output_tokens;
            s.total_cost_usd += cost;
        });
        {
            let (i, o, usd) = {
                let s = inner.state.lock().unwrap();
                let a = &s.agents[c.id];
                (a.input_tokens, a.output_tokens, a.cost_usd)
            };
            inner.trace_event(c.id, &backspace_runner::Event::Model { model: spec.id.clone() });
            inner.trace_event(c.id, &backspace_runner::Event::Usage { input: i, output: o });
            inner.trace_event(c.id, &backspace_runner::Event::Cost { usd });
        }

        let text = completion.text();
        if !text.trim().is_empty() {
            inner.log(c.id, LogKind::Assistant, text.clone());
        }
        c.messages.push(Message {
            role: Role::Assistant,
            content: completion.content.clone(),
        });

        match completion.stop {
            // The advisor paused the turn; send it again to carry on.
            Stop::Pause => continue,
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
            join_all(calls.iter().map(|(call_id, name, input)| {
                inner.log(
                    cref.id,
                    LogKind::ToolCall,
                    format!("{name} {}", truncate(&input.to_string(), 400)),
                );
                inner.trace_event(cref.id, &backspace_runner::Event::ToolStart {
                    id: call_id.clone(),
                    name: name.clone(),
                    input: truncate(&input.to_string(), 4000).to_string(),
                });
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
            inner.trace_event(c.id, &backspace_runner::Event::ToolEnd {
                id: call_id.clone(),
                output: truncate(&content, 4000).to_string(),
                error: is_error,
            });
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
        "list_agents" => Ok(text(crate::board::agents_line(
            &inner.state.lock().unwrap(),
        ))),
        "send_message" => {
            let me = inner.state.lock().unwrap().agents[c.id].key.clone();
            inner.post_board(
                &me,
                input["to"].as_str().unwrap_or(""),
                input["text"].as_str().unwrap_or(""),
            )?;
            Ok(text("sent".into()))
        }
        "read_messages" => {
            let s = inner.state.lock().unwrap();
            let me = s.agents[c.id].key.clone();
            Ok(text(inner.cursors.lock().unwrap().take(&s, &me)))
        }
        "scout" if c.kind != AgentKind::Scout => scouts(inner, c, input).await.map(text),
        "open_canvas" | "close_canvas" => {
            let me = inner.state.lock().unwrap().agents[c.id].key.clone();
            inner.canvas(&me, if name == "open_canvas" { "open" } else { "close" }, input).map(text)
        }
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
        changes: vec![],
        check_passed: None,
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
        deliverable.changes = git::numstat(&dir, &parent_branch, &branch)
            .await
            .unwrap_or_default();
    }

    // 2. The ticket's check gates review. Failing it is escalation evidence.
    if let Some(check) = ticket.as_ref().and_then(|t| t.check.clone()) {
        let (code, out) = c.ws.sh(&check).await?;
        if code != 0 {
            let before = inner.state.lock().unwrap().agents[c.id].log.iter().filter(|e| e.text.contains("CHECK FAILED")).count();
            let advise = if before >= 1 && inner.cfg.advisor.model.is_some() {
                "\nThis check has now failed twice: consult the advisor before your next attempt. Are you fixing the root cause, or going round in circles?"
            } else {
                ""
            };
            return Ok(ToolOutcome::Failure(
                format!(
                    "CHECK FAILED: `{check}` exited {code}. Nothing was sent for review.\n{}\nFix it, then call submit_deliverable again.{advise}",
                    truncate(&out, 3000)
                ),
                "check failed".into(),
            ));
        }
        deliverable.check_passed = Some(check);
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
    let (keys, needs_approval) = create_batch(inner, caller, input)?;
    if !needs_approval {
        return Ok(format!("Created {}. Dispatch them with work_tickets.", keys.join(", ")));
    }
    Ok(approve_plan(inner, caller, keys).await)
}

/// Add a batch of tickets (proposed when the plan needs your approval).
/// Returns their keys and whether it does.
fn create_batch(inner: &Inner, caller: AgentId, input: &Value) -> Result<(Vec<String>, bool)> {
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
    Ok((batch.iter().map(|t| t.key.clone()).collect(), needs_approval))
}

/// Ask for your verdict on proposed tickets; on a no they are discarded.
async fn approve_plan(inner: &Arc<Inner>, caller: AgentId, keys: Vec<String>) -> String {
    let batch: Vec<Ticket> = {
        let s = inner.state.lock().unwrap();
        keys.iter().filter_map(|k| s.ticket(k).cloned()).collect()
    };
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
            changes: vec![],
            check_passed: None,
        },
        tickets: keys.clone(),
        state: ApprovalState::Pending,
    };
    match await_verdict(inner, approval).await {
        Verdict::Approved => {
            for k in &keys {
                inner.ticket(k, |t| t.state = TicketState::ReadyForAgent);
            }
            format!(
                "Plan APPROVED by the user: {}. Dispatch with work_tickets.",
                keys.join(", ")
            )
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
            format!(
                "Plan REJECTED by the user; those tickets were discarded. Feedback: {feedback}\nRevise the plan and call create_tickets again."
            )
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
        let h = tokio::spawn(async move {
            let res = run_worker(inner2.clone(), id).await;
            settle(&inner2, id, &key, &res);
            inner2.tasks.lock().unwrap().remove(&id);
            let _ = tx.send(true);
            res
        });
        inner.tasks.lock().unwrap().insert(id, h.abort_handle());
        handles.push(h);
    }

    let mut report = Vec::new();
    for ((id, _), h) in ids.iter().zip(handles) {
        let res = h
            .await
            .map_err(|e| if e.is_cancelled() { anyhow!("stopped by the user") } else { anyhow!("crashed: {e}") })
            .and_then(|r| r);
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
    inner.trace_finish(id, res.as_ref().err().map(|e| format!("{e:#}")));
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
        inner.trace_start(id, &format!("worker · {key}"), &brief);

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
        let cli = inner
            .cfg
            .cli_of(&decision.model)
            .and_then(|p| p.cli.clone());
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
            messages: vec![Message::user_text(first.clone())],
            tools: tools_for(AgentKind::Worker, can_spawn),
            budget_warned_at: None,
            turns_total: 0,
            recent_calls: vec![],
        };
        let res = match &cli {
            Some(cli) => finish_cli(&inner, &mut c, cli, &dir, &first).await,
            None => finish(&inner, &mut c).await,
        };
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

const SCOUT_PROMPT: &str = "You are a scout: a fast, read-only helper. Answer the one question you are given by reading files and running read-only shell commands (ls, grep, cat, git log). Never change anything. Reply with a short, concrete summary: file paths with line numbers, names, the facts asked for. Under 200 words.";

/// Scouts: one small, fast, read-only agent per task, in parallel, each in
/// the asker's checkout. Returns their summaries.
async fn scouts(inner: &Arc<Inner>, c: &Conversation, input: &Value) -> Result<String> {
    let tasks: Vec<String> = input["tasks"]
        .as_array()
        .context("`tasks` must be a list")?
        .iter()
        .filter_map(|t| t.as_str().map(String::from))
        .take(inner.cfg.advisor.scouts.max(1))
        .collect();
    if tasks.is_empty() {
        bail!("give at least one task");
    }
    let model = inner
        .cfg
        .advisor
        .scout_model
        .clone()
        .filter(|m| inner.cfg.usable_models().iter().any(|u| &u.id == m))
        .or_else(|| inner.cfg.usable_models().into_iter().min_by_key(|m| m.tier).map(|m| m.id.clone()))
        .ok_or_else(|| anyhow!("no API model for scouts; read the files yourself"))?;
    let d = Decision { model, effort: Effort::Low, source: "scout".into(), confidence: 1.0, router_cost_usd: 0.0, note: None };
    let (depth, dir) = {
        let s = inner.state.lock().unwrap();
        (s.agents[c.id].depth + 1, c.ws.root.clone())
    };
    let runs = tasks.iter().enumerate().map(|(i, task)| {
        let (inner, d, dir, task) = (inner.clone(), d.clone(), dir.clone(), task.clone());
        let parent = c.id;
        async move {
            let id = inner.update(|s| {
                let id = s.agents.len();
                let mut rec = new_record(id, AgentKind::Scout, Some(parent), depth, format!("scout-{id}"), truncate(&task, 60).to_string());
                rec.brief = task.clone();
                rec.decision = Some(d.clone());
                s.agents.push(rec);
                id
            });
            inner.log(id, LogKind::User, task.clone());
            inner.trace_start(id, &format!("scout · {}", truncate(&task, 40)), &task);
            let mut sc = Conversation {
                id,
                kind: AgentKind::Scout,
                ticket: None,
                can_spawn: false,
                ws: inner.workspace(&dir),
                initial: d.clone(),
                system: inner.system_prompt(AgentKind::Scout, depth, &d),
                decision: d,
                messages: vec![Message::user_text(task.clone())],
                tools: tools_for(AgentKind::Scout, false),
                budget_warned_at: None,
                turns_total: 0,
                recent_calls: vec![],
            };
            inner.set_status(id, AgentStatus::Running);
            let res = drive(&inner, &mut sc).await;
            let answer = sc.messages.iter().rev().find(|m| m.role == Role::Assistant).map(|m| {
                m.content.iter().filter_map(|b| if let Block::Text { text } = b { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join("\n")
            }).unwrap_or_default();
            inner.set_status(id, if res.is_ok() { AgentStatus::Approved } else { AgentStatus::Failed });
            inner.trace_finish(id, res.as_ref().err().map(|e| format!("{e:#}")));
            let body = match res {
                Ok(_) if !answer.trim().is_empty() => answer,
                Ok(_) => "(no answer)".into(),
                Err(e) => format!("(failed: {e:#})"),
            };
            format!("## Scout {}: {task}\n{body}", i + 1)
        }
    });
    Ok(join_all(runs).await.join("\n\n"))
}

/// Another task, in parallel with whatever runs: a ticket of its own, on its
/// own branch and worktree, built by an agent that may plan sub-tickets;
/// reviewed and merged like any ticket. The main agent hears how it went.
fn start_task(inner: &Arc<Inner>, rt: &tokio::runtime::Handle, text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        bail!("say what the task is");
    }
    let slug: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|w| !w.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join("-");
    let key = {
        let s = inner.state.lock().unwrap();
        let base = if slug.is_empty() { "task".to_string() } else { slug };
        (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") })
            .find(|k| s.ticket(k).is_none() && !s.agents.iter().any(|a| &a.key == k))
            .unwrap()
    };
    let title: String = text.lines().next().unwrap_or(text).chars().take(70).collect();
    let input = json!({"tickets": [{"key": key, "title": title, "what_to_build": text,
        "acceptance": ["It does what the task asks", "Existing tests and checks still pass"]}]});
    create_batch(inner, MAIN, &input)?;
    // You asked for it: no plan approval; the work itself still comes to review.
    inner.ticket(&key, |t| t.state = TicketState::ReadyForAgent);
    inner.log(MAIN, LogKind::System, format!("started `{key}` in parallel: {title}"));
    let (i2, k2) = (inner.clone(), key.clone());
    rt.spawn(async move {
        let report = work_tickets(&i2, MAIN, &json!({ "keys": [k2] }))
            .await
            .unwrap_or_else(|e| format!("`{k2}` could not start: {e:#}"));
        i2.log(MAIN, LogKind::System, report);
    });
    Ok(key)
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
        inner.touch();

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
        changes: vec![],
        check_passed: None,
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

pub(crate) fn create_tickets_tool() -> ToolDef {
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

pub(crate) fn work_tickets_tool() -> ToolDef {
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

// ---------------------------------------------------------------- board bridge

async fn bridge_handle(
    mut stream: tokio::net::TcpStream,
    inner: Arc<Inner>,
    token: &str,
) -> Result<()> {
    use crate::remote::{read_request, respond, same};
    let req = read_request(&mut stream).await?;
    let ok = req
        .auth
        .as_deref()
        .and_then(|a| a.strip_prefix("Bearer "))
        .is_some_and(|t| same(t, token));
    if !ok {
        return respond(&mut stream, 401, &json!({"error": "bad token"})).await;
    }
    let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
    let res: Result<Value> = match req.path.as_str() {
        "/v1/board/post" => inner
            .post_board(
                body["from"].as_str().unwrap_or("?"),
                body["to"].as_str().unwrap_or(""),
                body["text"].as_str().unwrap_or(""),
            )
            .map(|_| json!({"ok": true})),
        "/v1/board/read" => {
            let s = inner.state.lock().unwrap();
            let me = body["me"].as_str().unwrap_or("");
            Ok(json!({"text": inner.cursors.lock().unwrap().take(&s, me)}))
        }
        "/v1/board/agents" => {
            Ok(json!({"text": crate::board::agents_line(&inner.state.lock().unwrap())}))
        }
        "/v1/canvas" => inner.canvas(
            body["me"].as_str().unwrap_or("?"),
            if body["op"] == "close" { "close" } else { "open" },
            &body,
        ).map(|t| json!({"text": t})),
        // A planner in a CLI: tickets are made now (so mistakes come back at
        // once); approval and dispatch run after its turn (plan_on_cli).
        p if p.starts_with("/v1/plan/") && (body["me"] != "main" || inner.planner.is_none()) => {
            Err(anyhow!("only the main agent plans"))
        }
        "/v1/plan/create" => create_batch(&inner, MAIN, &body).map(|(keys, needs)| {
            let keys_s = keys.join(", ");
            if needs {
                inner.planner_queue.lock().unwrap().push(PlanAction::Approve(keys));
                json!({"text": format!("Proposed {keys_s} for the user's approval. End your turn now; their verdict comes back as your next message.")})
            } else {
                json!({"text": format!("Created {keys_s}. Call work_tickets to dispatch them.")})
            }
        }),
        "/v1/plan/dispatch" => {
            let keys = body["keys"].as_array().map(|a| a.iter().filter_map(|k| k.as_str().map(String::from)).collect());
            inner.planner_queue.lock().unwrap().push(PlanAction::Dispatch(keys));
            Ok(json!({"text": "Queued. End your turn now: the agents start then, and each ticket's result comes back as your next message."}))
        }
        _ => return respond(&mut stream, 404, &json!({"error": "not found"})).await,
    };
    match res {
        Ok(v) => respond(&mut stream, 200, &v).await,
        Err(e) => respond(&mut stream, 400, &json!({"error": e.to_string()})).await,
    }
}
