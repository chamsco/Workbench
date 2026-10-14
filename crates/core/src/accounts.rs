//! Several accounts per coding CLI, kept apart in their own profile
//! directories, with health tracking and failover — the csm principle, native.
//!
//! Each account is one profile directory, pointed at with the CLI's own
//! variable (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GEMINI_CLI_HOME`,
//! `COPILOT_HOME`, `CURSOR_CONFIG_DIR`, `XDG_DATA_HOME`), so two logins of the
//! same CLI never see each other's session or quota. Health is per profile;
//! on a *confirmed* usage-limit report the harness can checkpoint and resume on
//! the next account. Two rules are the whole point of the port: usage is never
//! estimated, and a plain failure never switches accounts.
//!
//! `accounts.json` lives under [`Prefs::data_dir`]; profiles live in
//! `<data>/accounts/<provider>-<id>/`.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A day in ms: the window `fails_24h` counts over.
pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;
/// How long an account cools down when the CLI reported a limit but no usable
/// reset time, so we can not put it straight back at the top of the order.
pub const DEFAULT_COOLDOWN_MS: u64 = 30 * 60 * 1000;
/// ponytail: a ring, so an account that fails forever can not grow the file.
const MAX_FAIL_TIMES: usize = 32;

/// Who may switch accounts, and when.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FailoverPolicy {
    /// Switch on a confirmed limit report and resume the same conversation.
    Automatic,
    /// Never switch by itself; the user's `next` drives it.
    #[default]
    Manual,
    /// Leave accounts alone entirely.
    Off,
}

/// How much we trust a profile right now.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    /// No turn has run on this profile yet.
    #[default]
    Unknown,
    Healthy,
    /// Recent failures, but no confirmed limit.
    Warning,
    /// A confirmed usage limit; skipped until it cools down.
    Exhausted,
}

impl HealthState {
    /// The ranking order (Healthy > Unknown > Warning > Exhausted), which is
    /// deliberately not the declaration order.
    fn rank(self) -> u8 {
        match self {
            HealthState::Healthy => 0,
            HealthState::Unknown => 1,
            HealthState::Warning => 2,
            HealthState::Exhausted => 3,
        }
    }
}

/// What we know about one profile. Times are ms since the epoch.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Health {
    pub last_used: u64,
    pub last_ok: u64,
    pub last_fail: u64,
    /// Failures in the last 24h, refreshed by `note_*`/`load`; the ordering
    /// itself counts `fail_times`, so a stale count can not outrank reality.
    pub fails_24h: usize,
    /// Set only by a confirmed limit report; 0 means usable now.
    pub cooldown_until: u64,
    pub state: HealthState,
    /// The failure times `fails_24h` is counted from, oldest first.
    pub fail_times: Vec<u64>,
}

impl Health {
    /// Failures inside the 24h window ending at `now`, counted from the ring:
    /// a count saved by a process that has been down must decay, not be trusted.
    fn recent_fails(&self, now: u64) -> usize {
        let since = now.saturating_sub(DAY_MS);
        self.fail_times.iter().filter(|t| **t >= since).count()
    }

    /// Drop times older than the window and refresh the cached count.
    fn prune(&mut self, now: u64) {
        let since = now.saturating_sub(DAY_MS);
        self.fail_times.retain(|t| *t >= since);
        self.fails_24h = self.fail_times.len();
    }

    fn record_fail(&mut self, now: u64) {
        self.prune(now);
        self.fail_times.push(now);
        if self.fail_times.len() > MAX_FAIL_TIMES {
            let over = self.fail_times.len() - MAX_FAIL_TIMES;
            self.fail_times.drain(..over);
        }
        self.fails_24h = self.fail_times.len();
    }
}

/// What the CLI itself last reported about a quota window. `used`/`limit` are
/// only ever copied from that report — never estimated from timings or tokens.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Usage {
    pub used: Option<u64>,
    pub limit: Option<u64>,
    /// When the window resets (0 when the CLI did not say).
    pub resets_at: u64,
    /// The CLI's own words, shown as-is in Settings.
    pub report: String,
}

/// One login of one CLI, living in its own profile directory.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Account {
    pub id: String,
    pub name: String,
    /// The CLI this profile belongs to ("claude", "codex", ...): picks the env var.
    pub provider: String,
    /// The profile directory the CLI is pointed at; None means the CLI's own.
    pub profile_dir: Option<String>,
    pub enabled: bool,
    /// Lower wins among otherwise equal accounts.
    pub priority: u8,
    pub health: Health,
    pub usage: Option<Usage>,
    /// Who the profile is signed in as, verified when it is first used.
    pub identity: Option<String>,
}

/// The whole registry, as stored in `accounts.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Accounts {
    pub policy: FailoverPolicy,
    pub list: Vec<Account>,
}

impl Accounts {
    /// Where the registry lives: one file for the whole machine, so every
    /// project and chat sees the same logins and the same health.
    pub fn path() -> std::path::PathBuf {
        crate::prefs::Prefs::data_dir().join("accounts.json")
    }

    /// Add a login (or update one with the same id). A new one gets its own
    /// profile directory under the data dir.
    pub fn add(&mut self, provider: &str, name: &str) -> Account {
        let base: String = name
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let base = base.trim_matches('-');
        let base = if base.is_empty() { "account" } else { base };
        let mut id = base.to_string();
        let mut n = 1;
        while self.list.iter().any(|a| a.id == id) {
            n += 1;
            id = format!("{base}-{n}");
        }
        let a = Account {
            profile_dir: Some(default_profile_dir(provider, &id)),
            id,
            name: if name.trim().is_empty() { provider.to_string() } else { name.trim().to_string() },
            provider: provider.to_string(),
            enabled: true,
            priority: self.list.len() as u8,
            ..Account::default()
        };
        self.list.push(a.clone());
        a
    }

    /// Forget a login. The profile directory is left on disk: it holds the
    /// CLI's own credentials, and deleting a login is the user's call.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|a| a.id != id);
        self.list.len() != before
    }

    /// Accounts for one CLI, in the order they would be tried.
    pub fn for_provider(&self, provider: &str) -> Vec<&Account> {
        let mut v: Vec<&Account> = self
            .list
            .iter()
            .filter(|a| a.provider.eq_ignore_ascii_case(provider))
            .collect();
        v.sort_by_key(|a| (a.priority, a.id.clone()));
        v
    }

    /// The best account to run one CLI right now (health and cooldown decide;
    /// policy decides whether we may switch at all).
    pub fn best_for(&self, provider: &str, now: u64) -> Option<&Account> {
        if self.policy == FailoverPolicy::Off {
            return None;
        }
        self.for_provider(provider)
            .into_iter()
            .filter(|a| a.enabled && a.health.cooldown_until <= now)
            .min_by_key(|a| {
                (
                    a.health.state.rank(),
                    a.health.fails_24h,
                    a.priority,
                    std::cmp::Reverse(a.health.last_ok),
                )
            })
    }
}

/// Is this failure the CLI telling us the account has hit its limit (rather
/// than a plain rate limit, a crash or a network blip)? Only a confirmed limit
/// may move a session to another account.
pub fn looks_like_usage_limit(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    const HINTS: [&str; 9] = [
        "usage limit",
        "limit reached",
        "reached your limit",
        "quota exceeded",
        "out of credits",
        "insufficient_quota",
        "you've hit your",
        "weekly limit",
        "resets at",
    ];
    HINTS.iter().any(|h| t.contains(h))
}

/// The variable a CLI reads its own config directory from, so one account can
/// live apart from another without disturbing the machine's login.
pub fn profile_env(provider: &str) -> Option<&'static str> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "claude" => Some("CLAUDE_CONFIG_DIR"),
        "codex" => Some("CODEX_HOME"),
        "gemini" => Some("GEMINI_CLI_HOME"),
        "copilot" => Some("COPILOT_HOME"),
        "cursor" => Some("CURSOR_CONFIG_DIR"),
        "opencode" => Some("XDG_DATA_HOME"),
        _ => None,
    }
}

/// Where a new account's profile goes. One directory per provider+id under the
/// data dir, so deleting an account can delete its login with it.
pub fn default_profile_dir(provider: &str, id: &str) -> String {
    crate::prefs::Prefs::data_dir()
        .join("accounts")
        .join(format!("{provider}-{id}"))
        .to_string_lossy()
        .into_owned()
}

impl Accounts {
    /// Read `accounts.json`. Anything unreadable means "no accounts yet", as in
    /// `Prefs::load`: a damaged registry must not stop a chat.
    pub fn load(path: &Path) -> Accounts {
        let mut a: Accounts = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Counts carried over from a process that has been down must decay.
        let now = crate::chat::now_ms();
        for acc in &mut a.list {
            acc.health.prune(now);
        }
        a
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut Account> {
        self.list.iter_mut().find(|a| a.id == id)
    }

    /// Accounts the user has switched on; everything else is theirs to ignore.
    pub fn enabled(&self) -> impl Iterator<Item = &Account> {
        self.list.iter().filter(|a| a.enabled)
    }

    /// The failover order at `now`: enabled, not cooling down, best first.
    fn ranked(&self, now: u64) -> Vec<&Account> {
        let mut v: Vec<&Account> = self
            .enabled()
            .filter(|a| a.health.cooldown_until <= now)
            .collect();
        v.sort_by_key(|a| {
            (
                a.health.state.rank(),
                a.health.recent_fails(now),
                a.priority,
                std::cmp::Reverse(a.health.last_ok),
            )
        });
        v
    }

    /// Every usable account's id, best first.
    pub fn order(&self) -> Vec<String> {
        self.ranked(crate::chat::now_ms())
            .into_iter()
            .map(|a| a.id.clone())
            .collect()
    }

    /// The account a turn should run on now. The policy decides whether the
    /// caller may *switch* to it; the lookup itself is policy-free.
    pub fn best(&self, now: u64) -> Option<&Account> {
        self.ranked(now).first().copied()
    }

    /// A turn finished. Clears a cooldown: it just proved the account works.
    pub fn note_ok(&mut self, id: &str, now: u64) {
        if let Some(a) = self.find_mut(id) {
            a.health.state = HealthState::Healthy;
            a.health.last_used = now;
            a.health.last_ok = now;
            a.health.cooldown_until = 0;
        }
    }

    /// A turn failed for an ordinary reason (network, CLI, model).
    /// Deliberately no cooldown: only the CLI's own limit report may take an
    /// account out of the rotation — a failed turn is not a reason to switch.
    pub fn note_fail(&mut self, id: &str, now: u64) {
        if let Some(a) = self.find_mut(id) {
            a.health.record_fail(now);
            a.health.last_used = now;
            a.health.last_fail = now;
            a.health.state = HealthState::Warning;
        }
    }

    /// The CLI reported a confirmed usage limit. This is the only thing that
    /// cools an account down and the only thing that exhausts it.
    pub fn note_limit(&mut self, id: &str, now: u64, resets_at: u64, report: String) {
        // No usable reset time (0, or already past): cool down for a while anyway.
        let until = if resets_at > now {
            resets_at
        } else {
            now.saturating_add(DEFAULT_COOLDOWN_MS)
        };
        if let Some(a) = self.find_mut(id) {
            a.health.record_fail(now);
            a.health.last_used = now;
            a.health.last_fail = now;
            a.health.state = HealthState::Exhausted;
            a.health.cooldown_until = until;
            let u = a.usage.get_or_insert_with(Usage::default);
            u.resets_at = resets_at;
            u.report = report;
        }
    }

    /// Copy the CLI's own usage report. Nothing is inferred here: state and
    /// cooldown move only through `note_*`, and `used`/`limit` stay unknown
    /// until the CLI itself states them.
    pub fn set_usage(&mut self, id: &str, u: Usage) {
        if let Some(a) = self.find_mut(id) {
            a.usage = Some(u);
        }
    }

    /// The account a `next-account` command switches to: the best usable one
    /// that is not the one just used. It is marked used, so a second press
    /// walks further down the list instead of repeating itself.
    /// `Off` means the user asked for no switching at all, even by hand.
    pub fn manual_next(&mut self, now: u64) -> Option<String> {
        if self.policy == FailoverPolicy::Off {
            return None;
        }
        // The most recently used account is the one being left behind.
        let current = self
            .list
            .iter()
            .filter(|a| a.health.last_used > 0)
            .max_by_key(|a| a.health.last_used)
            .map(|a| a.id.clone());
        let next = self
            .ranked(now)
            .into_iter()
            .find(|a| Some(&a.id) != current.as_ref())
            .map(|a| a.id.clone())?;
        if let Some(a) = self.find_mut(&next) {
            a.health.last_used = now;
        }
        Some(next)
    }

    /// The environment to spawn this account's CLI with: its own profile
    /// variable. Empty when there is no profile dir (the CLI's default login)
    /// or when the provider has no such variable.
    pub fn env_for(&self, id: &str) -> Vec<(String, String)> {
        let Some(a) = self.list.iter().find(|a| a.id == id) else {
            return Vec::new();
        };
        let Some(dir) = a.profile_dir.as_deref().filter(|d| !d.trim().is_empty()) else {
            return Vec::new();
        };
        match profile_env(&a.provider) {
            Some(var) => vec![(var.to_string(), dir.to_string())],
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 60 * 60 * 1000;

    /// A time relative to the real clock: `order()` takes no `now`, so its
    /// tests have to live on today's timeline (offsets are hours wide).
    fn at(hours: i64) -> u64 {
        crate::chat::now_ms().saturating_add_signed(hours * HOUR as i64)
    }

    fn acc(id: &str, state: HealthState, priority: u8) -> Account {
        Account {
            id: id.into(),
            name: id.into(),
            provider: "claude".into(),
            profile_dir: Some(format!("C:/profiles/{id}")),
            enabled: true,
            priority,
            health: Health {
                state,
                ..Health::default()
            },
            ..Account::default()
        }
    }

    fn ids(a: &Accounts) -> Vec<String> {
        a.order()
    }

    #[test]
    fn order_drops_disabled_and_cooled_down_accounts() {
        let mut a = Accounts::default();
        a.list.push(acc("a", HealthState::Healthy, 0));
        let mut off = acc("b", HealthState::Healthy, 0);
        off.enabled = false;
        a.list.push(off);
        let mut cold = acc("c", HealthState::Healthy, 0);
        cold.health.cooldown_until = u64::MAX;
        a.list.push(cold);
        assert_eq!(ids(&a), ["a"]);
        assert_eq!(a.enabled().count(), 2);
    }

    #[test]
    fn order_ranks_state_then_failures_then_priority_then_last_ok() {
        let mut a = Accounts::default();
        let mut healthy = acc("healthy", HealthState::Healthy, 9);
        // A bad state outranks a clean failure count...
        healthy.health.fail_times = vec![at(-1); 4];
        a.list.push(healthy);
        a.list.push(acc("unknown", HealthState::Unknown, 0));
        let mut warn = acc("warn", HealthState::Warning, 0);
        warn.health.fail_times = vec![at(-1)];
        a.list.push(warn);
        a.list.push(acc("done", HealthState::Exhausted, 0));
        assert_eq!(ids(&a), ["healthy", "unknown", "warn", "done"]);

        // ...then fewer failures, then priority, then the freshest success.
        let mut b = Accounts::default();
        let mut few = acc("few", HealthState::Healthy, 9);
        few.health.fail_times = vec![at(-1)];
        let mut many = acc("many", HealthState::Healthy, 0);
        many.health.fail_times = vec![at(-1); 3];
        b.list.push(many);
        b.list.push(few);
        assert_eq!(ids(&b), ["few", "many"]);

        let mut c = Accounts::default();
        c.list.push(acc("late", HealthState::Healthy, 5));
        let mut early = acc("early", HealthState::Healthy, 5);
        early.health.last_ok = at(-1);
        c.list.push(early);
        assert_eq!(ids(&c), ["early", "late"]);
    }

    #[test]
    fn best_prefers_healthy_over_warning_at_equal_failures() {
        let now = at(0);
        let mut a = Accounts::default();
        let mut warn = acc("warn", HealthState::Warning, 0);
        warn.health.fail_times = vec![now - HOUR, now - 2 * HOUR];
        warn.health.fails_24h = 2;
        a.list.push(warn);
        let mut ok = acc("ok", HealthState::Healthy, 0);
        ok.health.fail_times = vec![now - HOUR, now - 2 * HOUR];
        ok.health.fails_24h = 2;
        a.list.push(ok);
        assert_eq!(a.best(now).unwrap().id, "ok");
    }

    #[test]
    fn note_limit_cools_down_and_note_fail_does_not() {
        let now = at(0);
        let mut a = Accounts::default();
        a.list.push(acc("a", HealthState::Unknown, 0));
        a.list.push(acc("b", HealthState::Unknown, 1));

        a.note_fail("a", now);
        let h = &a.list[0].health;
        assert_eq!(h.state, HealthState::Warning);
        assert_eq!(h.cooldown_until, 0, "a plain failure must not cool down");
        assert_eq!(h.last_fail, now);

        // The same account hits its quota: now it is cooled down and skipped.
        a.note_limit("a", now, now + HOUR, "5h limit reached".into());
        let h = &a.list[0].health;
        assert_eq!(h.state, HealthState::Exhausted);
        assert_eq!(h.cooldown_until, now + HOUR);
        let u = a.list[0].usage.as_ref().unwrap();
        assert_eq!(u.resets_at, now + HOUR);
        assert_eq!(u.report, "5h limit reached");
        assert_eq!(u.used, None, "used/limit are never invented");
        assert_eq!(u.limit, None);
        assert_eq!(a.best(now).unwrap().id, "b");

        // A reset time in the past (or none) still cools down for a while.
        a.note_limit("b", now, 0, "no reset time".into());
        assert_eq!(a.list[1].health.cooldown_until, now + DEFAULT_COOLDOWN_MS);
        assert!(a.order().is_empty());

        // A success brings an exhausted account straight back.
        a.note_ok("a", now + HOUR + 1);
        assert_eq!(a.best(now + HOUR + 1).unwrap().id, "a");
    }

    #[test]
    fn fails_decay_after_a_day_and_the_ring_stays_small() {
        let now = at(0);
        let mut a = Accounts::default();
        // Stale count says 5, but every failure is older than a day.
        let mut old = acc("old", HealthState::Warning, 5);
        old.health.fail_times = vec![at(-25); 5];
        old.health.fails_24h = 5;
        let mut fresh = acc("fresh", HealthState::Warning, 0);
        fresh.health.fail_times = vec![at(-1)];
        a.list.push(old);
        a.list.push(fresh);
        assert_eq!(ids(&a), ["old", "fresh"]);
        assert_eq!(a.list[0].health.recent_fails(now), 0);

        // `note_fail` prunes as it goes, so the cached count decays too.
        let mut b = Accounts::default();
        b.list.push(acc("a", HealthState::Unknown, 0));
        b.note_fail("a", at(-25));
        assert_eq!(b.list[0].health.fails_24h, 1);
        b.note_fail("a", now);
        assert_eq!(b.list[0].health.fails_24h, 1);
        for _ in 0..40 {
            b.note_fail("a", now);
        }
        assert_eq!(b.list[0].health.fails_24h, MAX_FAIL_TIMES);
        assert!(b.list[0].health.fail_times.len() <= MAX_FAIL_TIMES);
    }

    #[test]
    fn env_for_uses_the_providers_own_variable() {
        let mut a = Accounts::default();
        a.list.push(acc("claude-work", HealthState::Unknown, 0));
        let mut codex = acc("codex-home", HealthState::Unknown, 0);
        codex.provider = "codex".into();
        codex.profile_dir = Some("C:/profiles/codex-home".into());
        a.list.push(codex);
        let mut grok = acc("grok", HealthState::Unknown, 0);
        grok.provider = "grok".into();
        a.list.push(grok);
        let mut bare = acc("bare", HealthState::Unknown, 0);
        bare.profile_dir = None;
        a.list.push(bare);

        assert_eq!(
            a.env_for("claude-work"),
            [("CLAUDE_CONFIG_DIR".to_string(), "C:/profiles/claude-work".to_string())]
        );
        assert_eq!(
            a.env_for("codex-home"),
            [("CODEX_HOME".to_string(), "C:/profiles/codex-home".to_string())]
        );
        assert!(a.env_for("grok").is_empty(), "unknown provider: no variable");
        assert!(a.env_for("bare").is_empty(), "no profile dir: the CLI's own login");
        assert!(a.env_for("nope").is_empty());
        assert_eq!(profile_env("  Claude "), Some("CLAUDE_CONFIG_DIR"));
        assert_eq!(profile_env("opencode"), Some("XDG_DATA_HOME"));
        assert_eq!(profile_env("antigravity"), None);
    }

    #[test]
    fn policies_decide_who_switches() {
        let now = at(0);
        let mut a = Accounts::default();
        a.list.push(acc("a", HealthState::Healthy, 0));
        a.list.push(acc("b", HealthState::Healthy, 1));

        // Off: no switching, not even by hand, but the lookup still works.
        a.policy = FailoverPolicy::Off;
        assert!(a.manual_next(now).is_none());
        assert_eq!(a.best(now).unwrap().id, "a");

        // Manual: `next` moves off the account in use (a turn ran on "a") and
        // then back, because the account it hands out is marked used.
        a.policy = FailoverPolicy::Manual;
        a.note_ok("a", now - 1);
        assert_eq!(a.manual_next(now).as_deref(), Some("b"));
        assert_eq!(a.manual_next(now).as_deref(), Some("a"));

        // A registry nobody has used yet can only hand out the best account.
        let mut fresh = Accounts::default();
        fresh.list.push(acc("a", HealthState::Healthy, 0));
        fresh.list.push(acc("b", HealthState::Healthy, 1));
        assert_eq!(fresh.manual_next(now).as_deref(), Some("a"));

        // Automatic: a confirmed limit is enough to move off the best account.
        a.policy = FailoverPolicy::Automatic;
        a.note_limit("a", now, 0, "limit".into());
        assert_eq!(a.best(now).unwrap().id, "b");
    }

    #[test]
    fn default_profile_dir_sits_under_the_data_accounts_folder() {
        let tail: std::path::PathBuf = ["accounts", "claude-work"].iter().collect();
        let p = default_profile_dir("claude", "work");
        assert!(Path::new(&p).ends_with(&tail), "{p}");
        assert_eq!(FailoverPolicy::default(), FailoverPolicy::Manual);
    }

    #[test]
    fn round_trips_through_json_and_decays_on_load() {
        let now = crate::chat::now_ms();
        let path = std::env::temp_dir().join(format!("backspace-accounts-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert!(Accounts::load(&path).list.is_empty(), "missing file: no accounts");

        let mut a = Accounts::default();
        a.policy = FailoverPolicy::Automatic;
        let mut stale = acc("a", HealthState::Warning, 0);
        stale.health.fail_times = vec![now - 25 * HOUR, now - HOUR];
        stale.health.fails_24h = 2;
        a.list.push(stale);
        a.save(&path).unwrap();

        // The file keeps the report verbatim; the count decays on the way in.
        let back = Accounts::load(&path);
        assert_eq!(back, {
            let mut want = a.clone();
            want.list[0].health.fail_times = vec![now - HOUR];
            want.list[0].health.fails_24h = 1;
            want
        });
        assert_eq!(serde_json::to_string(&back.policy).unwrap(), "\"automatic\"");
        assert_eq!(serde_json::to_string(&HealthState::Exhausted).unwrap(), "\"exhausted\"");
        let _ = std::fs::remove_file(&path);
    }
}
