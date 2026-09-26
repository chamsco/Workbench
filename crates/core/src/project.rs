use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::router::{AgentRole, Decision};

pub type AgentId = usize;
pub const MAIN: AgentId = 0;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Queued,
    Running,
    AwaitingApproval,
    Approved,
    Failed,
}

impl AgentStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, AgentStatus::Approved | AgentStatus::Failed)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    User,
    Assistant,
    ToolCall,
    ToolResult,
    System,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogEntry {
    pub kind: LogKind,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Deliverable {
    pub summary: String,
    pub files: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentRecord {
    pub id: AgentId,
    pub role: AgentRole,
    pub parent: Option<AgentId>,
    pub depth: usize,
    pub key: String,
    pub title: String,
    pub brief: String,
    pub depends_on: Vec<AgentId>,
    pub status: AgentStatus,
    pub decision: Option<Decision>,
    pub log: Vec<LogEntry>,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub deliverable: Option<Deliverable>,
    /// Spend cap for this agent and everything it spawns.
    pub budget_usd: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected { feedback: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Approval {
    pub id: usize,
    pub agent: AgentId,
    pub deliverable: Deliverable,
    pub state: ApprovalState,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectState {
    pub name: String,
    pub workspace: PathBuf,
    pub agents: Vec<AgentRecord>,
    pub approvals: Vec<Approval>,
    pub total_cost_usd: f64,
    pub router_cost_usd: f64,
    pub config_source: Option<PathBuf>,
}

impl ProjectState {
    pub fn pending_approvals(&self) -> impl Iterator<Item = &Approval> {
        self.approvals
            .iter()
            .filter(|a| a.state == ApprovalState::Pending)
    }

    /// This agent's spend plus every descendant's.
    pub fn subtree_cost(&self, id: AgentId) -> f64 {
        self.agents
            .iter()
            .filter(|a| self.ancestry(a.id).contains(&id))
            .map(|a| a.cost_usd)
            .sum()
    }

    /// `id` followed by its parent, grandparent, ... up to main.
    pub fn ancestry(&self, id: AgentId) -> Vec<AgentId> {
        let mut chain = vec![id];
        let mut cur = id;
        while let Some(p) = self.agents[cur].parent {
            chain.push(p);
            cur = p;
        }
        chain
    }

    pub fn subagent_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| a.role == AgentRole::Sub)
            .count()
    }
}
