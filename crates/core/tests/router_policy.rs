//! M2 acceptance, through the router: the planner starts on the cheapest
//! class that can do the job and uses the frontier only for a long task. This
//! replaces the old rule that forced the planner to Xhigh on every task.

use std::net::TcpListener;
use std::sync::Arc;

use anyhow::Result;
use backspace_core::config::Config;
use backspace_core::decision::{Answer, DecisionModel, Question, TaskType, Triage};
use backspace_core::effort::Effort;
use backspace_core::ladder::{class_of, ModelClass};
use backspace_core::router::{AgentRole, Decision, Router};

const ONE_SHOT: &str = "fix a typo in the readme";
const RESEARCH: &str =
    "research the options for rate limiting libraries and write up a comparison report, no code changes";
const LONG: &str =
    "design and build a multi-tenant billing system across several services with database migrations, auth and an admin dashboard";

/// A config whose models are all on a live local port (the router skips a
/// loopback provider nothing is listening on). `extra_router` lands in `[router]`.
fn cfg(port: u16, models: &str, extra_router: &str) -> Config {
    Config::parse(&format!(
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"
{extra_router}

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

{models}
"#
    ))
    .unwrap()
}

const MODELS: &str = r#"
[[models]]
id = "mid-cheap"
provider = "mock"
tier = 1
description = "cheap"
input_usd_per_mtok = 0.4
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mid-strong"
provider = "mock"
tier = 2
description = "mid"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 5.0
max_output_tokens = 1000

[[models]]
id = "front"
provider = "mock"
tier = 4
description = "frontier"
input_usd_per_mtok = 4.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000
"#;

fn route(cfg: &Config, role: AgentRole, task: &str) -> Decision {
    router(cfg).route_blocking(role, task)
}

fn router(cfg: &Config) -> TestRouter {
    TestRouter(Router::new(reqwest::Client::new(), cfg.clone()))
}

struct TestRouter(Router);

impl TestRouter {
    fn route_blocking(&self, role: AgentRole, task: &str) -> Decision {
        tokio::runtime::Runtime::new().unwrap().block_on(self.0.route(role, task, "")).unwrap()
    }
}

fn class_of_decision(cfg: &Config, d: &Decision) -> ModelClass {
    class_of(cfg.model(&d.model).unwrap())
}

fn live() -> (TcpListener, u16) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    (l, p)
}

#[test]
fn a_one_shot_task_is_planned_on_a_mid_model_not_a_frontier_one() {
    let (_l, port) = live();
    let c = cfg(port, MODELS, "");
    let d = route(&c, AgentRole::Main, ONE_SHOT);
    assert_eq!(class_of_decision(&c, &d), ModelClass::Mid, "{d:?}");
    assert!(matches!(d.triage, Some(Triage::Confident { task: TaskType::OneShot, .. })), "{d:?}");
    // The configured planner effort floor is the user's, and still applies.
    assert_eq!(d.effort, Effort::High);
}

#[test]
fn a_research_task_is_planned_on_a_mid_model() {
    let (_l, port) = live();
    let c = cfg(port, MODELS, "");
    let d = route(&c, AgentRole::Main, RESEARCH);
    assert_eq!(class_of_decision(&c, &d), ModelClass::Mid, "{d:?}");
    assert!(matches!(d.triage, Some(Triage::Confident { task: TaskType::Research, .. })), "{d:?}");
}

#[test]
fn a_long_task_is_planned_on_a_frontier_model_at_high_effort() {
    let (_l, port) = live();
    let c = cfg(port, MODELS, "");
    let d = route(&c, AgentRole::Main, LONG);
    assert_eq!(class_of_decision(&c, &d), ModelClass::Frontier, "{d:?}");
    assert!(d.effort >= Effort::Xhigh, "{d:?}");
    assert!(matches!(d.triage, Some(Triage::Confident { task: TaskType::Long, .. })), "{d:?}");
}

#[test]
fn an_unclear_task_starts_mid_and_is_marked_for_asking() {
    let (_l, port) = live();
    let c = cfg(port, MODELS, "");
    let d = route(&c, AgentRole::Main, "ok");
    assert_eq!(class_of_decision(&c, &d), ModelClass::Mid, "{d:?}");
    assert!(matches!(d.triage, Some(Triage::Ask { .. })), "{d:?}");
}

#[test]
fn the_threshold_in_the_config_decides_what_counts_as_unsure() {
    let (_l, port) = live();
    // Nothing reaches 1.0, so every task is unsure and none starts on the frontier.
    let c = cfg(port, MODELS, "triage_threshold = 1.0");
    let d = route(&c, AgentRole::Main, LONG);
    assert!(matches!(d.triage, Some(Triage::Ask { .. })), "{d:?}");
    assert_eq!(class_of_decision(&c, &d), ModelClass::Mid);
}

#[test]
fn a_pinned_planner_model_still_wins() {
    let (_l, port) = live();
    let c = cfg(port, MODELS, "pin_main = \"front\"");
    let d = route(&c, AgentRole::Main, ONE_SHOT);
    assert_eq!(d.model, "front");
}

#[test]
fn a_project_with_only_frontier_models_still_routes() {
    let (_l, port) = live();
    let only = MODELS.split("[[models]]").filter(|b| b.contains("id = \"front\"")).collect::<Vec<_>>().join("");
    let c = cfg(port, &format!("[[models]]{only}"), "");
    let d = route(&c, AgentRole::Main, ONE_SHOT);
    assert_eq!(d.model, "front");
}

#[test]
fn workers_are_not_classified_by_the_planner_policy() {
    let (_l, port) = live();
    let c = cfg(port, MODELS, "");
    let d = route(&c, AgentRole::Sub, LONG);
    assert!(d.triage.is_none(), "{d:?}");
}

/// A decider that always says "long", to show the router takes whatever it is given.
struct AlwaysLong;

impl DecisionModel for AlwaysLong {
    fn name(&self) -> &str {
        "always-long"
    }
    fn decide(&self, _state: &str, questions: &[Question]) -> Result<Vec<Answer>> {
        Ok(questions
            .iter()
            .map(|q| Answer {
                question: q.id.clone(),
                probs: vec![("one_shot".into(), 0.02), ("long".into(), 0.96), ("research".into(), 0.02)],
            })
            .collect())
    }
}

#[test]
fn swapping_the_decider_changes_where_the_planner_starts() {
    let (_l, port) = live();
    let c = cfg(port, MODELS, "");
    let before = route(&c, AgentRole::Main, ONE_SHOT);
    assert_eq!(class_of_decision(&c, &before), ModelClass::Mid);
    let swapped = TestRouter(Router::new(reqwest::Client::new(), c.clone()).with_decider(Arc::new(AlwaysLong)));
    let after = swapped.route_blocking(AgentRole::Main, ONE_SHOT);
    assert_eq!(class_of_decision(&c, &after), ModelClass::Frontier, "{after:?}");
}
