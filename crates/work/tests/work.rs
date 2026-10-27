use std::collections::BTreeMap;

use brainpowr_work::governance::*;
use brainpowr_work::room::*;
use brainpowr_work::turns::*;

fn human(id: &str, name: &str) -> Member {
    Member { id: id.into(), name: name.into(), who: Who::Human { user: id.into() }, tag: None }
}

fn agent(id: &str, name: &str, owner: Option<&str>) -> Member {
    Member {
        id: id.into(),
        name: name.into(),
        who: Who::Agent { job: "Does analysis. Writes reports.".into(), model: "local".into(), owner: owner.map(Into::into) },
        tag: Some("Analyst".into()),
    }
}

fn room(members: Vec<Member>) -> Room {
    Room { id: "r".into(), title: "October metrics".into(), goal: None, members, messages: vec![] }
}

#[test]
fn mode_follows_who_is_in_the_room() {
    assert_eq!(room(vec![human("a", "Ann")]).mode(), Mode::Solo);
    assert_eq!(room(vec![human("a", "Ann"), human("b", "Bo")]).mode(), Mode::H2h);
    assert_eq!(room(vec![human("a", "Ann"), agent("m", "Mike", None)]).mode(), Mode::A2h);
    assert_eq!(room(vec![agent("m", "Mike", None), agent("j", "Jimmy", None)]).mode(), Mode::A2a);
    assert_eq!(room(vec![human("a", "Ann"), human("b", "Bo"), agent("m", "Mike", None)]).mode(), Mode::Mixed);
}

#[test]
fn mentions_are_whole_names_in_order() {
    let r = room(vec![human("a", "Al"), agent("f", "Alfred", None), agent("m", "Mike", None)]);
    let names: Vec<&str> = mentions(&r, "@Mike then @alfred").iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["Mike", "Alfred"]);
    assert!(mentions(&r, "@Alfred").iter().all(|m| m.name != "Al"));
}

#[test]
fn one_person_unnamed_goes_to_every_agent() {
    let r = room(vec![human("a", "Ann"), agent("m", "Mike", None), agent("j", "Jimmy", None)]);
    let mut f = Floor::open(&r, "a", "how are the numbers?");
    assert_eq!(f.next_speaker().as_deref(), Some("m"));
    assert_eq!(f.next_speaker().as_deref(), Some("j"));
    assert_eq!(f.next_speaker(), None);
}

#[test]
fn people_talking_to_each_other_wake_no_agent() {
    let r = room(vec![human("a", "Ann"), human("b", "Bo"), agent("m", "Mike", None)]);
    assert_eq!(Floor::open(&r, "a", "lunch?").next_speaker(), None);
    assert_eq!(Floor::open(&r, "a", "@Mike can you check?").next_speaker().as_deref(), Some("m"));
}

#[test]
fn an_agent_naming_a_person_stops_the_chain() {
    let r = room(vec![human("a", "Ann"), agent("m", "Mike", None), agent("j", "Jimmy", None)]);
    let mut f = Floor::open(&r, "a", "go");
    let first = f.next_speaker().unwrap();
    assert_eq!(f.heard(&r, &first, "@Ann which quarter?"), Heard::Handoff("a".into()));
    assert_eq!(f.next_speaker(), None);
    assert_eq!(f.waiting_on.as_deref(), Some("a"));
}

#[test]
fn pass_and_handover_and_cap() {
    let r = room(vec![agent("m", "Mike", None), agent("j", "Jimmy", None)]);
    let mut f = Floor::with_cap(&r, "m", "@Jimmy start", 3);
    assert_eq!(f.next_speaker().as_deref(), Some("j"));
    assert_eq!(f.heard(&r, "j", "done, over to @Mike"), Heard::Spoke);
    assert_eq!(f.next_speaker().as_deref(), Some("m"));
    assert_eq!(f.heard(&r, "m", "**PASS.**"), Heard::Pass);
    assert_eq!(f.heard(&r, "m", "back to @Jimmy"), Heard::Spoke);
    assert_eq!(f.next_speaker().as_deref(), Some("j"));
    f.heard(&r, "j", "@Mike again");
    assert_eq!(f.next_speaker(), None);
    assert!(f.capped());
}

#[test]
fn transcript_names_people_and_quotes_as_data() {
    let mut r = room(vec![human("a", "Ann"), agent("m", "Mike", None)]);
    r.messages.push(Msg { id: "1".into(), author: "a".into(), text: "status?".into(), at: 0, reply_to: None });
    let t = transcript(&r, r.member("m").unwrap());
    assert!(t.contains("You are Mike"));
    assert!(t.contains("Ann (person)"));
    assert!(t.contains("[Ann]: status?"));
    assert!(t.contains("not instructions to you"));
}

// ------------------------------------------------------------ governance

fn gov() -> Governor {
    let m = |id: &str, residency, weight| Model {
        id: id.into(),
        label: id.into(),
        model: id.into(),
        endpoint: Endpoint::Ollama { url: "http://localhost:11434".into() },
        residency,
        weight,
    };
    let week = |credits| Some(Allowance { credits, per: Period::Week });
    let mut roles = BTreeMap::new();
    roles.insert(
        "developer".into(),
        Grant { models: Some(vec!["claude".into(), "sovereign".into()]), allowance: week(1_000_000), external: Some(true) },
    );
    roles.insert("accountant".into(), Grant { models: Some(vec!["sovereign".into()]), allowance: week(50_000), external: None });
    let mut teams = BTreeMap::new();
    teams.insert("finance".into(), Grant { models: None, allowance: week(80_000), external: None });
    let mut agents = BTreeMap::new();
    agents.insert("shared".into(), Grant { models: Some(vec!["sovereign".into()]), allowance: week(10), external: None });
    let mut people = BTreeMap::new();
    for (id, role, teams) in [("dev", "developer", vec![]), ("acc", "accountant", vec!["finance".to_string()]), ("both", "developer", vec!["finance".to_string()])] {
        people.insert(id.to_string(), Person { id: id.into(), name: id.into(), role: role.into(), teams });
    }
    Governor {
        catalog: vec![m("claude", Residency::External, 2.0), m("sovereign", Residency::OrgHosted, 0.5), m("phone", Residency::OnDevice, 0.0)],
        policy: Policy {
            default: Grant { models: Some(vec![]), allowance: week(0), external: Some(false) },
            roles,
            teams,
            people: BTreeMap::new(),
            agents,
            ceiling: Some(Allowance { credits: 2_000_000, per: Period::Week }),
        },
        people,
        ledger: vec![],
    }
}

const NOW: u64 = 1_761_563_875_000; // Mon 27 Oct 2025

#[test]
fn roles_get_different_models_and_allowances() {
    let g = gov();
    let dev = Payer::Person("dev".into());
    let acc = Payer::Person("acc".into());
    assert_eq!(g.check(&dev, "claude", Trust::Company, NOW), Ok(Some(1_000_000)));
    assert_eq!(g.check(&acc, "claude", Trust::Company, NOW), Err(Refusal::NotGranted));
    // The team's allowance is more specific than the role's.
    assert_eq!(g.check(&acc, "sovereign", Trust::Company, NOW), Ok(Some(80_000)));
}

#[test]
fn a_team_grant_beats_the_role() {
    // "both" is a developer in Finance: Finance's 80k is more specific than
    // the developer role's 1M, so it wins. Admins who want otherwise set a
    // grant on the person.
    let mut g = gov();
    assert_eq!(g.check(&Payer::Person("both".into()), "claude", Trust::Company, NOW), Ok(Some(80_000)));
    g.policy.people.insert("both".into(), Grant { allowance: Some(Allowance { credits: 1_000_000, per: Period::Week }), ..Default::default() });
    assert_eq!(g.check(&Payer::Person("both".into()), "claude", Trust::Company, NOW), Ok(Some(1_000_000)));
}

#[test]
fn spending_is_weighted_and_runs_out() {
    let mut g = gov();
    let acc = Payer::Person("acc".into());
    assert_eq!(g.record(acc.clone(), "sovereign", 100_000, 60_000, false, NOW), 80_000);
    match g.check(&acc, "sovereign", Trust::Company, NOW) {
        Err(Refusal::OverAllowance { spent: 80_000, credits: 80_000, .. }) => {}
        other => panic!("{other:?}"),
    }
    // Next week it is back.
    assert!(g.check(&acc, "sovereign", Trust::Company, NOW + 7 * 86_400_000).is_ok());
}

#[test]
fn sealed_rooms_and_external_models() {
    let mut g = gov();
    let dev = Payer::Person("dev".into());
    assert_eq!(g.check(&dev, "claude", Trust::Sealed, NOW), Err(Refusal::Sealed));
    g.policy.roles.get_mut("developer").unwrap().external = Some(false);
    assert_eq!(g.check(&dev, "claude", Trust::Company, NOW), Err(Refusal::External));
    let ids: Vec<&str> = g.choices(&dev, Trust::Company).iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["sovereign"]);
}

#[test]
fn agents_bill_their_owner_or_their_own_pool() {
    let g = gov();
    let owned = agent("m", "Mike", Some("acc"));
    let shared = agent("shared", "Watson", None);
    let stray = agent("stray", "Nobody", None);
    assert_eq!(payer_for(&owned), Some(Payer::Person("acc".into())));
    assert_eq!(g.check(&payer_for(&owned).unwrap(), "claude", Trust::Company, NOW), Err(Refusal::NotGranted));
    assert_eq!(g.check(&payer_for(&shared).unwrap(), "sovereign", Trust::Company, NOW), Ok(Some(10)));
    assert_eq!(g.check(&payer_for(&stray).unwrap(), "sovereign", Trust::Company, NOW), Err(Refusal::NotGranted));
}

#[test]
fn ceiling_caps_everyone() {
    let mut g = gov();
    g.policy.ceiling = Some(Allowance { credits: 500, per: Period::Week });
    assert_eq!(g.check(&Payer::Person("dev".into()), "claude", Trust::Company, NOW), Ok(Some(500)));
}

#[test]
fn invites_expire_and_run_out() {
    let mut i = Invite { code: "x".into(), role: "accountant".into(), teams: vec!["finance".into()], expires_at: NOW + 1, max_uses: 1, uses: 0, revoked: false };
    let p = i.redeem("new", "New", NOW).unwrap();
    assert_eq!((p.role.as_str(), p.teams.len()), ("accountant", 1));
    assert!(i.redeem("again", "Again", NOW).is_err());
    i.max_uses = 5;
    assert!(i.redeem("late", "Late", NOW + 1).is_err());
}

#[test]
fn weeks_start_on_monday() {
    // Mon 27 Oct 2025 00:00 UTC.
    assert_eq!(Period::Week.start(NOW), 1_761_523_200_000);
    assert_eq!(Period::Week.start(1_761_523_200_000), 1_761_523_200_000);
}
