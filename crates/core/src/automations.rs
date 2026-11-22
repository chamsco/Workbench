//! Automations (Charles, 22 Nov: "automation and the agent side"): a familiar's routines, run
//! by whichever window is open. Each one posts into a side thread of its own under the
//! familiar's stream, so its runs keep their history and the familiar's chat stays clean.

use std::sync::Arc;

use anyhow::{bail, Result};

use crate::agents::{Agent, Agents, Routine};
use crate::chat::{now_ms, Chats, Ctx};

/// Every routine of every agent, with the agent it belongs to, soonest wake first (paused last).
pub fn all(agents: &Agents) -> Vec<(Agent, Routine)> {
    let mut v: Vec<(Agent, Routine)> = agents.list().into_iter().flat_map(|a| a.routines.clone().into_iter().map(move |r| (a.clone(), r))).collect();
    v.sort_by_key(|(_, r)| (!r.enabled, if r.next_run == 0 { u64::MAX } else { r.next_run }));
    v
}

/// What is due at `now`: at most one routine per agent (the oldest wake), and none for an agent
/// that is answering something else.
pub fn due(chats: &Chats, agents: &Agents, now: u64) -> Vec<(Agent, Routine)> {
    let busy: std::collections::HashSet<String> = chats.list().into_iter().filter(|t| t.busy).filter_map(|t| t.agent).collect();
    agents
        .list()
        .into_iter()
        .filter(|a| !busy.contains(&a.id))
        .filter_map(|a| {
            let r = a.routines.iter().filter(|r| r.enabled && r.next_run > 0 && r.next_run <= now).min_by_key(|r| r.next_run).cloned()?;
            Some((a, r))
        })
        .collect()
}

/// The words an automation run posts.
pub fn prompt(r: &Routine) -> String {
    let what = if r.instructions.trim().is_empty() { r.name.trim() } else { r.instructions.trim() };
    format!("Automation \u{201c}{}\u{201d} is due. Do it now: {what}\n\nWhen you are done, say in one line what you did.", r.name.trim())
}

/// Runs automation `r` of familiar `a` now: posts it into its thread and books the run.
/// `scheduled` is the clock firing (the next wake moves on); otherwise it is Run now (the next
/// wake stays). Returns the thread it went to.
pub fn run(chats: &Arc<Chats>, agents: &Agents, a: &Agent, r: &Routine, scheduled: bool, ctx: Ctx) -> Result<String> {
    if a.lead.is_none() {
        bail!("{} is not a familiar", a.name);
    }
    let id = chats.automation_thread(a, r);
    if chats.is_busy(&id) {
        bail!("its last run is still going");
    }
    chats.event(&id, &format!("Automation \u{b7} {}", r.name), &prompt(r), ctx)?;
    // The run is in; bookkeeping that fails (it was deleted meanwhile) must not fire it again.
    if r.thread != id {
        let _ = agents.routine_thread(&a.id, &r.id, &id);
    }
    let _ = if scheduled { agents.routine_ran(&a.id, &r.id, now_ms()) } else { agents.routine_posted(&a.id, &r.id) };
    Ok(id)
}

/// One scheduler tick: runs what is due among the familiars. Errors leave a routine due, and the
/// next tick tries again.
pub fn tick(chats: &Arc<Chats>, agents: &Agents, ctx: impl Fn() -> Ctx) -> usize {
    due(chats, agents, now_ms()).into_iter().filter(|(a, _)| a.lead.is_some()).filter(|(a, r)| run(chats, agents, a, r, true, ctx()).is_ok()).count()
}
