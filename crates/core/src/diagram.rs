//! Diagrams for human review, built by the harness from what it already
//! knows (tickets, edges, branches, checks, escalations, line counts). The
//! agent never draws these, so a review diagram cannot be wrong in the same
//! way the work under review might be.

use std::collections::HashMap;

use serde::Serialize;

use crate::project::{AgentRecord, Approval, ApprovalKind, FileChange, ProjectState, MAIN};
use crate::ticket::{Ticket, TicketState};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Neutral,
    /// Finished and merged.
    Done,
    /// Running, or the item under review.
    Active,
    /// Needs a second look (e.g. a ticket with no check).
    Warn,
    Fail,
}

#[derive(Clone, Debug, Serialize)]
pub struct Node {
    pub label: String,
    pub sub: String,
    pub tone: Tone,
    /// Column: 0 = no dependencies.
    pub layer: usize,
    /// Position within its column.
    pub row: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagram {
    pub title: String,
    /// One-line facts shown above the drawing, e.g. ("waves", "2").
    pub facts: Vec<(String, String)>,
    pub nodes: Vec<Node>,
    /// (from, to) node indices; arrows point from dependency to dependent.
    pub edges: Vec<(usize, usize)>,
    pub files: Vec<FileChange>,
    /// Things the reviewer should look at before approving.
    pub warnings: Vec<String>,
}

impl Diagram {
    pub fn layers(&self) -> usize {
        self.nodes.iter().map(|n| n.layer + 1).max().unwrap_or(0)
    }

    pub fn rows(&self) -> usize {
        self.nodes.iter().map(|n| n.row + 1).max().unwrap_or(0)
    }
}

/// The diagram shown first on an approval card.
pub fn for_approval(s: &ProjectState, ap: &Approval) -> Diagram {
    match ap.kind {
        ApprovalKind::Plan => plan(s, &ap.tickets),
        ApprovalKind::Deliverable if ap.agent == MAIN => project(s),
        ApprovalKind::Deliverable => deliverable(s, ap),
    }
}

/// Place nodes in columns by longest dependency chain, then number rows.
fn layer(nodes: &mut [Node], edges: &[(usize, usize)]) {
    let mut depth = vec![0usize; nodes.len()];
    // Edges only point forward in creation order (blocked_by must be listed
    // earlier), so one pass in index order settles every depth.
    let mut by_target: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
    for &(a, b) in edges {
        by_target[b].push(a);
    }
    for i in 0..nodes.len() {
        depth[i] = by_target[i]
            .iter()
            .map(|&a| depth[a] + 1)
            .max()
            .unwrap_or(0);
    }
    let mut rows: HashMap<usize, usize> = HashMap::new();
    for (i, n) in nodes.iter_mut().enumerate() {
        n.layer = depth[i];
        let r = rows.entry(depth[i]).or_insert(0);
        n.row = *r;
        *r += 1;
    }
}

fn ticket_sub(t: &Ticket) -> String {
    let check = if t.check.is_some() {
        "check"
    } else {
        "no check"
    };
    format!("{} criteria · {check}", t.acceptance.len())
}

/// A proposed batch: what runs in parallel, what waits, what has no check.
fn plan(s: &ProjectState, keys: &[String]) -> Diagram {
    let batch: Vec<&Ticket> = keys.iter().filter_map(|k| s.ticket(k)).collect();
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut warnings = Vec::new();

    // Blockers outside the batch (already done) appear as settled nodes first.
    for t in &batch {
        for b in &t.blocked_by {
            if !keys.contains(b) && !index.contains_key(b.as_str()) {
                if let Some(bt) = s.ticket(b) {
                    index.insert(&bt.key, nodes.len());
                    nodes.push(Node {
                        label: bt.key.clone(),
                        sub: bt.state.label().into(),
                        tone: Tone::Done,
                        layer: 0,
                        row: 0,
                    });
                }
            }
        }
    }
    for t in &batch {
        index.insert(&t.key, nodes.len());
        let tone = if t.check.is_none() {
            Tone::Warn
        } else {
            Tone::Neutral
        };
        nodes.push(Node {
            label: t.key.clone(),
            sub: ticket_sub(t),
            tone,
            layer: 0,
            row: 0,
        });
        if t.check.is_none() {
            warnings.push(format!(
                "`{}` has no check, so its work reaches you unverified",
                t.key
            ));
        }
        if t.acceptance.is_empty() {
            warnings.push(format!("`{}` has no acceptance criteria", t.key));
        }
    }
    for t in &batch {
        for b in &t.blocked_by {
            if let (Some(&a), Some(&z)) = (index.get(b.as_str()), index.get(t.key.as_str())) {
                edges.push((a, z));
            }
        }
    }
    layer(&mut nodes, &edges);
    let waves = nodes
        .iter()
        .filter(|n| n.tone != Tone::Done)
        .map(|n| n.layer)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let widest = (0..=nodes.iter().map(|n| n.layer).max().unwrap_or(0))
        .map(|l| {
            nodes
                .iter()
                .filter(|n| n.layer == l && n.tone != Tone::Done)
                .count()
        })
        .max()
        .unwrap_or(0);
    let budget: f64 = batch.iter().filter_map(|t| t.budget_usd).sum();
    let mut facts = vec![
        ("tickets".into(), batch.len().to_string()),
        ("waves".into(), waves.to_string()),
        ("most in parallel".into(), widest.to_string()),
        (
            "with checks".into(),
            format!(
                "{}/{}",
                batch.iter().filter(|t| t.check.is_some()).count(),
                batch.len()
            ),
        ),
    ];
    if budget > 0.0 {
        facts.push(("budgets".into(), format!("${budget:.2}")));
    }
    Diagram {
        title: "What will run, and in what order".into(),
        facts,
        nodes,
        edges,
        files: vec![],
        warnings,
    }
}

/// One ticket: where its work came from, how it got here, where it lands.
fn deliverable(s: &ProjectState, ap: &Approval) -> Diagram {
    let agent = &s.agents[ap.agent];
    let ticket = ap.tickets.first().and_then(|k| s.ticket(k));
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut warnings = Vec::new();

    let mut blockers = Vec::new();
    if let Some(t) = ticket {
        for b in &t.blocked_by {
            if let Some(bt) = s.ticket(b) {
                blockers.push(nodes.len());
                nodes.push(Node {
                    label: bt.key.clone(),
                    sub: "merged into the base".into(),
                    tone: Tone::Done,
                    layer: 0,
                    row: 0,
                });
            }
        }
    }
    let route = route_line(agent);
    let me = nodes.len();
    nodes.push(Node {
        label: ticket
            .map(|t| t.key.clone())
            .unwrap_or_else(|| agent.key.clone()),
        sub: route,
        tone: if agent.escalations.is_empty() {
            Tone::Active
        } else {
            Tone::Warn
        },
        layer: 0,
        row: 0,
    });
    for b in &blockers {
        edges.push((*b, me));
    }
    let check = nodes.len();
    let d = ap.deliverable.clone();
    match &d.check_passed {
        Some(c) => nodes.push(Node {
            label: "check passed".into(),
            sub: truncate(c, 34),
            tone: Tone::Done,
            layer: 0,
            row: 0,
        }),
        None => {
            nodes.push(Node {
                label: "no check".into(),
                sub: "unverified".into(),
                tone: Tone::Warn,
                layer: 0,
                row: 0,
            });
            warnings.push("No check ran: nothing automatic verified this work".into());
        }
    }
    edges.push((me, check));
    let target = nodes.len();
    let parent_branch = agent
        .parent
        .and_then(|p| s.agents[p].branch.clone())
        .unwrap_or_else(|| "your branch".into());
    nodes.push(Node {
        label: "merge on approve".into(),
        sub: format!("into {parent_branch}"),
        tone: Tone::Neutral,
        layer: 0,
        row: 0,
    });
    edges.push((check, target));
    layer(&mut nodes, &edges);

    if !agent.escalations.is_empty() {
        warnings.push(format!(
            "Escalated {}x before submitting: {}",
            agent.escalations.len(),
            agent.escalations.join("; ")
        ));
    }
    let added: u32 = d.changes.iter().map(|c| c.added).sum();
    let removed: u32 = d.changes.iter().map(|c| c.removed).sum();
    let facts = vec![
        ("files".into(), d.changes.len().to_string()),
        ("lines".into(), format!("+{added} −{removed}")),
        ("cost".into(), format!("${:.3}", s.subtree_cost(ap.agent))),
    ];
    Diagram {
        title: "Where this work came from and where it lands".into(),
        facts,
        nodes,
        edges,
        files: d.changes,
        warnings,
    }
}

/// The whole run, for the final deliverable.
fn project(s: &ProjectState) -> Diagram {
    let tickets: Vec<&Ticket> = s
        .tickets
        .iter()
        .filter(|t| t.state != TicketState::Proposed)
        .collect();
    let index: HashMap<&str, usize> = tickets
        .iter()
        .enumerate()
        .map(|(i, t)| (t.key.as_str(), i))
        .collect();
    let mut warnings = Vec::new();
    let mut nodes: Vec<Node> = tickets
        .iter()
        .map(|t| {
            let tone = match t.state {
                TicketState::Done => Tone::Done,
                TicketState::Failed => Tone::Fail,
                TicketState::Wontfix => Tone::Neutral,
                _ => Tone::Warn,
            };
            if !matches!(t.state, TicketState::Done | TicketState::Wontfix) {
                warnings.push(format!("`{}` is {}", t.key, t.state.label()));
            }
            let sub = t
                .assignee
                .map(|a| route_line(&s.agents[a]))
                .unwrap_or_else(|| t.state.label().into());
            Node {
                label: t.key.clone(),
                sub,
                tone,
                layer: 0,
                row: 0,
            }
        })
        .collect();
    let edges: Vec<(usize, usize)> = tickets
        .iter()
        .enumerate()
        .flat_map(|(i, t)| {
            t.blocked_by
                .iter()
                .filter_map(|b| index.get(b.as_str()))
                .map(move |&a| (a, i))
                .collect::<Vec<_>>()
        })
        .collect();
    layer(&mut nodes, &edges);
    let escalated = s
        .agents
        .iter()
        .filter(|a| !a.escalations.is_empty())
        .count();
    let facts = vec![
        (
            "tickets done".into(),
            format!(
                "{}/{}",
                tickets
                    .iter()
                    .filter(|t| t.state == TicketState::Done)
                    .count(),
                tickets.len()
            ),
        ),
        ("escalated".into(), escalated.to_string()),
        ("total".into(), format!("${:.3}", s.total_cost_usd)),
    ];
    Diagram {
        title: "Everything this goal produced".into(),
        facts,
        nodes,
        edges,
        files: vec![],
        warnings,
    }
}

/// "model @ effort", or "start → end ↑n" when it escalated.
pub fn route_line(a: &AgentRecord) -> String {
    let now = a
        .decision
        .as_ref()
        .map(|d| format!("{} @ {}", d.model, d.effort))
        .unwrap_or_else(|| "not routed".into());
    match a.escalations.first().and_then(|e| e.split(" → ").next()) {
        Some(start) => format!("{start} → {now} ↑{}", a.escalations.len()),
        None => now,
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ApprovalState;
    use serde_json::json;

    fn state_with(tickets: Vec<Ticket>) -> ProjectState {
        ProjectState {
            name: "t".into(),
            workspace: "/tmp".into(),
            agents: vec![],
            approvals: vec![],
            tickets,
            total_cost_usd: 0.0,
            router_cost_usd: 0.0,
            config_source: None,
            board: vec![],
        }
    }

    fn t(key: &str, blocked: &[&str], check: bool) -> Ticket {
        let mut v =
            json!({"key": key, "what_to_build": "x", "acceptance": ["a"], "blocked_by": blocked});
        if check {
            v["check"] = json!("true");
        }
        Ticket::from_json(&v, 1, MAIN, TicketState::Proposed).unwrap()
    }

    #[test]
    fn plan_layers_by_blockers_and_flags_missing_checks() {
        let s = state_with(vec![
            t("schema", &[], true),
            t("api", &["schema"], true),
            t("ui", &["schema"], false),
            t("e2e", &["api", "ui"], true),
        ]);
        let ap = Approval {
            id: 0,
            kind: ApprovalKind::Plan,
            agent: MAIN,
            deliverable: Default::default(),
            tickets: vec!["schema".into(), "api".into(), "ui".into(), "e2e".into()],
            state: ApprovalState::Pending,
        };
        let d = for_approval(&s, &ap);
        let layer = |k: &str| d.nodes.iter().find(|n| n.label == k).unwrap().layer;
        assert_eq!(
            (layer("schema"), layer("api"), layer("ui"), layer("e2e")),
            (0, 1, 1, 2)
        );
        assert_eq!(d.edges.len(), 4);
        assert!(d.facts.contains(&("waves".into(), "3".into())));
        assert!(d.facts.contains(&("most in parallel".into(), "2".into())));
        assert_eq!(d.warnings.len(), 1);
        assert!(d.warnings[0].contains("`ui` has no check"));
    }
}
