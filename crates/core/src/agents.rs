//! Agents with a job (after Noodle): a name, a backstory that sets the
//! role, an avatar, the model or CLI that runs it, its own folder, and
//! optionally its own computer. You talk to one in its thread, or put
//! several in a group with a goal, where they read each other's messages and
//! take turns (see chat.rs).
//!
//! Each agent works in its own folder (`<data>/agents/<id>/workspace`) until
//! you share more folders with it. Its computer is where it runs things that
//! should not touch your machine:
//!
//! - `folder`: just its folder on this machine (the default);
//! - `docker`: a container of its own on this machine, kept between uses
//!   (`/home/agent` is a folder under the agent's directory), optionally a
//!   full desktop you can open (a webtop image on a local port);
//! - `ssh`: a VPS or any machine you can ssh to with a key.
//!
//! CLIs reach the computer through `backspace mcp` (computer_run,
//! computer_read, computer_write), which reads BACKSPACE_COMPUTER.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::chat::{new_id, now_ms, Route};

pub const ENV_COMPUTER: &str = "BACKSPACE_COMPUTER";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Agent {
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// The backstory: who it is and what it does. Its system prompt.
    #[serde(default)]
    pub job: String,
    #[serde(default)]
    pub avatar: Avatar,
    /// The style text appended to its system prompt (a personality chip).
    #[serde(default)]
    pub personality: String,
    pub route: Route,
    /// Folders it may use besides its own.
    #[serde(default)]
    pub shared: Vec<String>,
    #[serde(default)]
    pub computer: Computer,
    /// Give it the memory notes (global and its own).
    #[serde(default = "yes")]
    pub memory: bool,
    /// Its rules: what it may not do ("edit", "run", "web", "apps").
    #[serde(default)]
    pub off: Vec<String>,
    /// Apps and connections it may not use, by id; the rest it may.
    #[serde(default)]
    pub apps_off: Vec<String>,
    /// Its webhook's secret (Reach this bot); empty while the webhook is off.
    #[serde(default)]
    pub hook: String,
    /// Its job on a clock, owned by the bot (see [`Routine`]).
    #[serde(default)]
    pub routines: Vec<Routine>,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
}

fn yes() -> bool {
    true
}

impl Agent {
    pub fn allows(&self, rule: &str) -> bool {
        !self.off.iter().any(|o| o == rule)
    }

    /// What its rules deny, as `backspace mcp` reads it (BACKSPACE_DENY): the
    /// rules turned off, and "app:<id>" for each app or connection it may not use.
    pub fn deny(&self) -> Vec<String> {
        self.off.iter().cloned().chain(self.apps_off.iter().map(|a| format!("app:{a}"))).collect()
    }
}

/// A bot's job on a clock: it needs the bot's model, rules, memory, folder
/// and computer, so it lives on the bot and not in a store of its own.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Routine {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// What to do when it fires.
    #[serde(default)]
    pub instructions: String,
    /// "daily" | "weekdays" | "weekly" | "every"
    #[serde(default)]
    pub kind: String,
    /// "HH:MM" local, for daily/weekdays/weekly
    #[serde(default)]
    pub time: String,
    /// ISO weekdays, 1 = Mon .. 7 = Sun, for weekly
    #[serde(default)]
    pub days: Vec<u8>,
    /// every: minutes between runs
    #[serde(default)]
    pub every_minutes: u32,
    /// Project id this run belongs to; "" = the bot's own folder
    #[serde(default)]
    pub project: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub last_run: u64,
    /// Absolute ms; computed here, never by the UI.
    #[serde(default)]
    pub next_run: u64,
    #[serde(default)]
    pub runs: u32,
    /// Minutes east of UTC, sent by the UI (JS offset negated).
    #[serde(default)]
    pub utc_offset_minutes: i32,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
}

const WEEKDAYS: [&str; 8] = ["", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

impl Routine {
    /// The first occurrence strictly after `from`, or 0 when disabled.
    pub fn next_after(&self, from: u64) -> u64 {
        if !self.enabled {
            return 0;
        }
        let from = from.min(i64::MAX as u64) as i64;
        if self.kind == "every" {
            let every = self.every_minutes.max(5) as i64 * 60_000;
            return u64::try_from(from + every).unwrap_or(0);
        }
        let Some(minute) = parse_hhmm(&self.time) else { return 0 };
        let offset = self.utc_offset_minutes as i64 * 60_000;
        // Local time is the clock the routine's owner set, so all the date
        // maths happens in it and only the answer goes back to UTC.
        let local = from + offset;
        let mut day = local.div_euclid(civil::DAY_MS);
        let at = minute as i64 * 60_000;
        if local.rem_euclid(civil::DAY_MS) >= at {
            day += 1;
        }
        match self.kind.as_str() {
            "daily" => {}
            "weekdays" => while !(1..=5).contains(&civil::weekday(day)) {
                day += 1;
            },
            "weekly" => {
                let days = self.valid_days();
                if days.is_empty() {
                    return 0;
                }
                let mut steps = 0;
                while !days.contains(&civil::weekday(day)) {
                    day += 1;
                    steps += 1;
                    if steps > 7 {
                        return 0;
                    }
                }
            }
            _ => return 0,
        }
        u64::try_from(day * civil::DAY_MS + at - offset).unwrap_or(0)
    }

    /// "Every day at 08:00", "Wakes Mon, Wed at 07:30", "Every 6 hours".
    pub fn summary(&self) -> String {
        let at = norm_hhmm(&self.time).unwrap_or_else(|_| "08:00".into());
        let when = match self.kind.as_str() {
            "daily" => format!("Every day at {at}"),
            "weekdays" => format!("Weekdays at {at}"),
            "weekly" => {
                let days = self.valid_days();
                let names: Vec<&str> = if days.is_empty() {
                    vec!["Mon"]
                } else {
                    days.iter().map(|d| WEEKDAYS[*d as usize]).collect()
                };
                format!("Wakes {} at {at}", names.join(", "))
            }
            "every" => {
                let m = self.every_minutes;
                if m >= 60 && m % 60 == 0 {
                    format!("Every {} hour{}", m / 60, if m == 60 { "" } else { "s" })
                } else {
                    format!("Every {m} min")
                }
            }
            _ => "Not scheduled".into(),
        };
        if self.enabled {
            when
        } else {
            format!("{when} (paused)")
        }
    }

    fn valid_days(&self) -> Vec<u8> {
        let mut days: Vec<u8> = self.days.iter().copied().filter(|d| (1..=7).contains(d)).collect();
        days.sort_unstable();
        days.dedup();
        days
    }

    /// Validation for something stored by a UI: errors when this cannot be a
    /// routine, and normalises the fields the scheduler reads.
    fn sanitise(&mut self) -> Result<()> {
        self.name = self.name.trim().chars().take(60).collect();
        if self.name.is_empty() {
            bail!("a routine needs a name");
        }
        match self.kind.as_str() {
            "daily" | "weekdays" => {
                self.time = norm_hhmm(&self.time)?;
                self.days.clear();
                self.every_minutes = 0;
            }
            "weekly" => {
                self.time = norm_hhmm(&self.time)?;
                self.days = self.valid_days();
                if self.days.is_empty() {
                    bail!("a weekly routine needs at least one day");
                }
                self.every_minutes = 0;
            }
            "every" => {
                if self.every_minutes < 5 {
                    bail!("a routine runs every 5 minutes at least");
                }
                self.time.clear();
                self.days.clear();
            }
            k => bail!("unknown routine kind {k:?}"),
        }
        self.instructions = self.instructions.trim().chars().take(4000).collect();
        self.project = self.project.trim().to_string();
        Ok(())
    }
}

/// "HH:MM" to minutes after midnight, or None when it is not a time.
fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u32 = h.trim().parse().ok()?;
    let m: u32 = m.trim().parse().ok()?;
    (h <= 23 && m <= 59).then_some(h * 60 + m)
}

/// The one "HH:MM" spelling the core stores, so times compare as strings.
fn norm_hhmm(s: &str) -> Result<String> {
    let minute = parse_hhmm(s).ok_or_else(|| anyhow!("a routine time is HH:MM"))?;
    Ok(format!("{:02}:{:02}", minute / 60, minute % 60))
}

/// Local date maths without a date crate: day numbers since 1970-01-01, with
/// the ISO weekday. The Hinnant conversions (as in git.rs) are the way in and
/// out, so the tests can read a wake back as a civil date.
mod civil {
    pub const DAY_MS: i64 = 86_400_000;

    #[cfg(test)]
    pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// ISO weekday of a day number: 1 = Monday .. 7 = Sunday (1970-01-01 was
    /// a Thursday).
    pub fn weekday(days: i64) -> u8 {
        (days.rem_euclid(7) as u8 + 3) % 7 + 1
    }

    #[cfg(test)]
    pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m, d)
    }
}

/// The CLI tools (Claude Code's names) that a bot's rules turn off. Any rule
/// also takes the tools that hand work to something the rules don't cover
/// (a cloud agent, another session, a later run).
pub fn denied_tools(deny: &[String]) -> Vec<String> {
    let handoff = if deny.is_empty() { &[][..] } else { &["RemoteTrigger", "SendMessage", "CronCreate", "ScheduleWakeup"][..] };
    deny.iter()
        .flat_map(|d| match d.as_str() {
            "edit" => &["Edit", "Write", "NotebookEdit", "EnterWorktree"][..],
            "run" => &["Bash"][..],
            "web" => &["WebFetch", "WebSearch"][..],
            _ => &[][..],
        })
        .chain(handoff)
        .map(|t| t.to_string())
        .collect()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Avatar {
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Computer {
    /// "folder", "docker" or "ssh".
    pub kind: String,
    /// docker: the image (default ubuntu:24.04; a webtop image gives a desktop).
    #[serde(default)]
    pub image: String,
    /// docker: publish the image's web desktop (port 3000) on localhost.
    #[serde(default)]
    pub desktop: bool,
    /// ssh: host, user, port and private key file.
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub key: String,
    /// ssh: a web desktop on the VPS (noVNC...), if it has one.
    #[serde(default)]
    pub desktop_url: String,
}

impl Default for Computer {
    fn default() -> Self {
        Self {
            kind: "folder".into(),
            image: String::new(),
            desktop: false,
            host: String::new(),
            user: String::new(),
            port: 0,
            key: String::new(),
            desktop_url: String::new(),
        }
    }
}

pub const DEFAULT_IMAGE: &str = "ubuntu:24.04";
pub const DESKTOP_IMAGE: &str = "lscr.io/linuxserver/webtop:ubuntu-xfce";

/// What the computer is doing, for the agent's card.
#[derive(Serialize, Clone, Debug, Default)]
pub struct ComputerStatus {
    pub kind: String,
    /// "running", "stopped", "missing", "unreachable", "ready" (folder)
    pub state: String,
    pub detail: String,
    /// A web desktop to open, when there is one.
    pub desktop: Option<String>,
}

/// Everything `backspace mcp` needs to drive an agent's computer, passed in
/// BACKSPACE_COMPUTER as JSON.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ComputerEnv {
    pub agent: String,
    pub name: String,
    pub computer: Computer,
    /// docker: the host folder mounted at /home/agent.
    pub home: String,
}

pub struct Agents {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Agents {
    pub fn open(dir: PathBuf) -> Self {
        Self {
            dir,
            lock: Mutex::new(()),
        }
    }

    fn agent_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    /// The agent's own folder (created on first use).
    pub fn workspace(&self, id: &str) -> PathBuf {
        let p = self.agent_dir(id).join("workspace");
        let _ = std::fs::create_dir_all(&p);
        p
    }

    fn home(&self, id: &str) -> PathBuf {
        let p = self.agent_dir(id).join("computer");
        let _ = std::fs::create_dir_all(&p);
        p
    }

    pub fn list(&self) -> Vec<Agent> {
        let mut out: Vec<Agent> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|id| self.get(id)))
            .collect();
        out.sort_by(|a, b| a.created.cmp(&b.created));
        out
    }

    pub fn get(&self, id: &str) -> Option<Agent> {
        if id.is_empty() || id.contains(['/', '\\', '.']) {
            return None;
        }
        let b = std::fs::read(self.agent_dir(id).join("agent.json")).ok()?;
        serde_json::from_slice(&b).ok()
    }

    /// Create (empty id) or update an agent.
    pub fn save(&self, mut a: Agent) -> Result<Agent> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        a.name = a.name.trim().chars().take(40).collect();
        if a.name.is_empty() {
            bail!("an agent needs a name");
        }
        match a.computer.kind.as_str() {
            "folder" | "" => a.computer.kind = "folder".into(),
            "docker" => {}
            "ssh" => {
                if a.computer.host.trim().is_empty() {
                    bail!("an SSH computer needs a host");
                }
            }
            k => bail!("unknown computer kind {k}"),
        }
        let now = now_ms();
        if a.id.is_empty() {
            a.id = new_id();
            a.created = now;
        } else if let Some(old) = self.get(&a.id) {
            a.created = old.created;
        } else {
            bail!("no such agent");
        }
        a.updated = now;
        a.personality = a.personality.trim().chars().take(600).collect();
        // An old editor round-trip can carry routine fields the scheduler
        // cannot read; keep the usable ones and refresh a wake that has passed.
        clean_routines(&mut a, now);
        self.write(&a)?;
        Ok(a)
    }

    fn write(&self, a: &Agent) -> Result<()> {
        let d = self.agent_dir(&a.id);
        std::fs::create_dir_all(&d)?;
        std::fs::write(d.join("agent.json"), serde_json::to_vec_pretty(a)?)?;
        let _ = self.workspace(&a.id);
        Ok(())
    }

    /// Remove the agent. Its folder and computer files go too; a container
    /// is removed.
    pub fn delete(&self, id: &str) -> Result<()> {
        let a = self.get(id).ok_or_else(|| anyhow!("no such agent"))?;
        if a.computer.kind == "docker" {
            let _ = docker(&["rm", "-f", &container(id)]);
        }
        std::fs::remove_dir_all(self.agent_dir(id))?;
        Ok(())
    }

    /// The brief a reply runs with: who it is, where it works, its computer.
    pub fn brief(&self, a: &Agent) -> String {
        self.brief_in(a, None)
    }

    /// The brief for a bot working on a project: the project folder is its
    /// working directory, and a due routine is carried out in that thread.
    pub fn brief_in(&self, a: &Agent, project: Option<&Path>) -> String {
        let mut s = format!(
            "You are {}, one of the user's agents in Backspace.\n\n{}\n\n",
            a.name,
            if a.job.trim().is_empty() { "Help the user with whatever they ask." } else { a.job.trim() }
        );
        if !a.personality.trim().is_empty() {
            s.push_str(&format!("The user asked you to write like this: {}\n\n", a.personality.trim()));
        }
        match project {
            Some(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("project");
                s.push_str(&format!(
                    "You are working on the {name} project, in {} (your working directory). Keep your files there.\n",
                    p.display()
                ));
            }
            None => s.push_str(&format!(
                "Your own folder is {} (your working directory). Keep your files there.",
                self.workspace(&a.id).display()
            )),
        }
        if !a.shared.is_empty() {
            s.push_str(&format!(" The user also shared: {}.", a.shared.join(", ")));
        }
        match a.computer.kind.as_str() {
            "docker" => s.push_str(" You have your own Linux computer (a container); run commands, install software and keep files there with the computer_run, computer_read and computer_write tools. Prefer it over this machine for anything that installs or runs code."),
            "ssh" => s.push_str(&format!(" You have your own computer, a server at {}; use the computer_run, computer_read and computer_write tools to work on it.", a.computer.host)),
            _ => {}
        }
        if !a.routines.is_empty() {
            s.push_str("\n\nYour routines:");
            for r in &a.routines {
                let what = if r.instructions.trim().is_empty() { r.name.trim() } else { r.instructions.trim() };
                s.push_str(&format!("\n- {}: {} — {}", r.name.trim(), r.summary(), what));
            }
            s.push_str(
                "\nA message starting `Routine · <name>` is a routine that is due now: carry it out, then say in one line what you did.",
            );
        }
        s
    }

    /// Validate and upsert a routine on a bot; the next wake is computed here
    /// with the offset the UI sent, never by the UI.
    pub fn routine_save(&self, agent: &str, mut r: Routine) -> Result<Agent> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = self.get(agent).ok_or_else(|| anyhow!("no such agent"))?;
        let now = now_ms();
        r.sanitise()?;
        r.updated = now;
        r.next_run = r.next_after(now);
        let existing = a.routines.iter().position(|x| !r.id.is_empty() && x.id == r.id);
        match existing {
            Some(i) => {
                r.created = a.routines[i].created;
                a.routines[i] = r;
            }
            None => {
                if r.id.is_empty() || a.routines.iter().any(|x| x.id == r.id) {
                    r.id = new_id();
                }
                if r.created == 0 {
                    r.created = now;
                }
                a.routines.push(r);
            }
        }
        a.updated = now;
        self.write(&a)?;
        Ok(a)
    }

    pub fn routine_delete(&self, agent: &str, routine: &str) -> Result<Agent> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = self.get(agent).ok_or_else(|| anyhow!("no such agent"))?;
        let before = a.routines.len();
        a.routines.retain(|r| r.id != routine);
        if a.routines.len() == before {
            bail!("no such routine");
        }
        a.updated = now_ms();
        self.write(&a)?;
        Ok(a)
    }

    /// The scheduler's bookkeeping: it fired at `at`, so the next wake moves on
    /// from there.
    pub fn routine_ran(&self, agent: &str, routine: &str, at: u64) -> Result<Agent> {
        self.book(agent, routine, at, true)
    }

    /// A manual "Run now": the job was done early, so the next wake is left
    /// where it was.
    pub fn routine_posted(&self, agent: &str, routine: &str) -> Result<Agent> {
        self.book(agent, routine, now_ms(), false)
    }

    fn book(&self, agent: &str, routine: &str, at: u64, advance: bool) -> Result<Agent> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = self.get(agent).ok_or_else(|| anyhow!("no such agent"))?;
        {
            let r = a.routines.iter_mut().find(|r| r.id == routine).ok_or_else(|| anyhow!("no such routine"))?;
            r.last_run = at;
            r.runs = r.runs.saturating_add(1);
            r.updated = at;
            if advance {
                let next = r.next_after(at);
                r.next_run = next;
            }
        }
        self.write(&a)?;
        Ok(a)
    }

    pub fn computer_env(&self, a: &Agent) -> Option<ComputerEnv> {
        (a.computer.kind == "docker" || a.computer.kind == "ssh").then(|| ComputerEnv {
            agent: a.id.clone(),
            name: a.name.clone(),
            computer: a.computer.clone(),
            home: self.home(&a.id).display().to_string(),
        })
    }

    pub fn status(&self, id: &str) -> Result<ComputerStatus> {
        let a = self.get(id).ok_or_else(|| anyhow!("no such agent"))?;
        let c = &a.computer;
        Ok(match c.kind.as_str() {
            "docker" => {
                let name = container(id);
                match docker(&["inspect", "-f", "{{.State.Running}}", &name]) {
                    Ok(o) if o.trim() == "true" => ComputerStatus {
                        kind: "docker".into(),
                        state: "running".into(),
                        detail: image_of(c).into(),
                        desktop: desktop_port(&name).map(|p| format!("http://127.0.0.1:{p}")),
                    },
                    Ok(_) => ComputerStatus { kind: "docker".into(), state: "stopped".into(), detail: image_of(c).into(), desktop: None },
                    Err(e) if e.to_string().to_lowercase().contains("no such") => {
                        ComputerStatus { kind: "docker".into(), state: "missing".into(), detail: image_of(c).into(), desktop: None }
                    }
                    Err(e) => ComputerStatus { kind: "docker".into(), state: "unreachable".into(), detail: e.to_string(), desktop: None },
                }
            }
            "ssh" => ComputerStatus {
                kind: "ssh".into(),
                state: "ready".into(),
                detail: format!("{}@{}", if c.user.is_empty() { "root" } else { &c.user }, c.host),
                desktop: (!c.desktop_url.is_empty()).then(|| c.desktop_url.clone()),
            },
            _ => ComputerStatus {
                kind: "folder".into(),
                state: "ready".into(),
                detail: self.workspace(id).display().to_string(),
                desktop: None,
            },
        })
    }

    /// Start (creating if needed) a docker computer.
    pub fn start(&self, id: &str) -> Result<ComputerStatus> {
        let a = self.get(id).ok_or_else(|| anyhow!("no such agent"))?;
        if a.computer.kind == "docker" {
            let env = self.computer_env(&a).unwrap();
            ensure_container(&env)?;
        }
        self.status(id)
    }

    pub fn stop(&self, id: &str) -> Result<ComputerStatus> {
        let a = self.get(id).ok_or_else(|| anyhow!("no such agent"))?;
        if a.computer.kind == "docker" {
            docker(&["stop", "-t", "2", &container(id)])?;
        }
        self.status(id)
    }
}

/// Keep the routines a stale editor can still run: drop the ones that cannot
/// be, give ids where they are missing, and move a wake that already passed.
fn clean_routines(a: &mut Agent, now: u64) {
    let mut seen = std::collections::HashSet::new();
    a.routines.retain_mut(|r| {
        if r.sanitise().is_err() {
            return false;
        }
        if r.id.is_empty() {
            r.id = new_id();
        }
        if !seen.insert(r.id.clone()) {
            r.id = new_id();
            seen.insert(r.id.clone());
        }
        if r.created == 0 {
            r.created = now;
        }
        if r.enabled {
            if r.next_run == 0 || r.next_run <= now {
                r.next_run = r.next_after(now);
            }
        } else {
            r.next_run = 0;
        }
        true
    });
}

fn image_of(c: &Computer) -> &str {
    if !c.image.trim().is_empty() {
        c.image.trim()
    } else if c.desktop {
        DESKTOP_IMAGE
    } else {
        DEFAULT_IMAGE
    }
}

pub fn container(id: &str) -> String {
    format!("backspace-agent-{id}")
}

fn docker(args: &[&str]) -> Result<String> {
    let o = Command::new("docker")
        .args(args)
        .env("PATH", crate::harnesses::path_env())
        .output()
        .context("Docker is not installed (or not on PATH)")?;
    if !o.status.success() {
        bail!("{}", String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).to_string())
}

fn desktop_port(name: &str) -> Option<u16> {
    let o = docker(&["port", name, "3000/tcp"]).ok()?;
    o.lines().next()?.rsplit(':').next()?.trim().parse().ok()
}

/// The agent's container, running. Created on first use with its home
/// folder mounted, so files survive a removed container.
pub fn ensure_container(env: &ComputerEnv) -> Result<()> {
    let name = container(&env.agent);
    match docker(&["inspect", "-f", "{{.State.Running}}", &name]) {
        Ok(o) if o.trim() == "true" => return Ok(()),
        Ok(_) => {
            docker(&["start", &name])?;
            return Ok(());
        }
        Err(_) => {}
    }
    let image = image_of(&env.computer).to_string();
    let mount = format!("{}:/home/agent", env.home);
    let host: String = env.name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).take(30).collect();
    let host = if host.is_empty() { "agent".to_string() } else { host };
    let mut args: Vec<String> = vec![
        "run".into(), "-d".into(), "--name".into(), name.clone(),
        "--label".into(), "backspace.agent=1".into(),
        "-v".into(), mount, "-w".into(), "/home/agent".into(),
        "--hostname".into(), host,
    ];
    if env.computer.desktop {
        args.extend(["-p".into(), "127.0.0.1::3000".into(), "--shm-size".into(), "1g".into()]);
        args.push(image);
    } else {
        args.push(image);
        args.extend(["sleep".into(), "infinity".into()]);
    }
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    docker(&a).map(|_| ())
}

const OUT_LIMIT: usize = 30_000;

fn clip(s: String) -> String {
    if s.len() <= OUT_LIMIT {
        return s;
    }
    let mut end = OUT_LIMIT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… ({} more bytes)", &s[..end], s.len() - end)
}

fn ssh_cmd(c: &Computer) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=10"]);
    if c.port != 0 {
        cmd.args(["-p", &c.port.to_string()]);
    }
    if !c.key.trim().is_empty() {
        cmd.args(["-i", c.key.trim()]);
    }
    let user = if c.user.trim().is_empty() { "root" } else { c.user.trim() };
    cmd.arg(format!("{user}@{}", c.host.trim()));
    cmd
}

/// Run a shell command on the agent's computer, with a time limit.
pub fn run(env: &ComputerEnv, command: &str, timeout: Duration) -> Result<String> {
    let mut cmd = match env.computer.kind.as_str() {
        "docker" => {
            ensure_container(env)?;
            let mut c = Command::new("docker");
            c.args(["exec", "-w", "/home/agent", &container(&env.agent), "sh", "-lc", command]);
            c
        }
        "ssh" => {
            let mut c = ssh_cmd(&env.computer);
            c.arg(command);
            c
        }
        k => bail!("{k} computers run commands through the CLI itself"),
    };
    output_with_timeout(&mut cmd, None, timeout)
}

/// Write a file on the agent's computer (stdin piped to `cat >`).
pub fn write(env: &ComputerEnv, path: &str, content: &str) -> Result<String> {
    let q = shell_quote(path);
    let script = format!("mkdir -p \"$(dirname {q})\" && cat > {q}");
    let mut cmd = match env.computer.kind.as_str() {
        "docker" => {
            ensure_container(env)?;
            let mut c = Command::new("docker");
            c.args(["exec", "-i", "-w", "/home/agent", &container(&env.agent), "sh", "-c", &script]);
            c
        }
        "ssh" => {
            let mut c = ssh_cmd(&env.computer);
            c.arg(script);
            c
        }
        k => bail!("{k} computers write files through the CLI itself"),
    };
    output_with_timeout(&mut cmd, Some(content.as_bytes()), Duration::from_secs(60))?;
    Ok(format!("wrote {} bytes to {path}", content.len()))
}

pub fn read(env: &ComputerEnv, path: &str) -> Result<String> {
    run(env, &format!("cat {}", shell_quote(path)), Duration::from_secs(60))
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn output_with_timeout(cmd: &mut Command, input: Option<&[u8]>, timeout: Duration) -> Result<String> {
    use std::io::{Read, Write};
    use std::process::Stdio;
    let mut child = cmd
        .env("PATH", crate::harnesses::path_env())
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting the computer's command")?;
    if let Some(i) = input {
        child.stdin.take().unwrap().write_all(i)?;
    }
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let ro = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let re = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err.read_to_end(&mut b);
        b
    });
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            bail!("timed out after {}s", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let o = String::from_utf8_lossy(&ro.join().unwrap_or_default()).to_string();
    let e = String::from_utf8_lossy(&re.join().unwrap_or_default()).to_string();
    let text = if e.trim().is_empty() { o } else { format!("{o}{}{e}", if o.is_empty() { "" } else { "\n[stderr]\n" }) };
    if !status.success() {
        bail!("exit {}: {}", status.code().unwrap_or(-1), clip(text));
    }
    Ok(clip(text))
}

/// Folder paths are kept as the user typed them; resolve `~`.
pub fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(h) = std::env::var_os("HOME") {
            return Path::new(&h).join(rest);
        }
    }
    PathBuf::from(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(name: &str) -> Agent {
        Agent {
            id: String::new(),
            name: name.into(),
            job: "Writes release notes.".into(),
            avatar: Avatar::default(),
            personality: String::new(),
            route: Route { kind: crate::chat::RouteKind::Cli, provider: "claude".into(), model: None },
            shared: vec![],
            computer: Computer::default(),
            memory: true,
            off: vec![],
            apps_off: vec![],
            hook: String::new(),
            routines: vec![],
            created: 0,
            updated: 0,
        }
    }

    #[test]
    fn rules_become_denied_tools() {
        let mut a = agent("Kira");
        assert!(a.deny().is_empty() && a.allows("run"));
        a.off = vec!["run".into(), "web".into()];
        a.apps_off = vec!["github".into()];
        assert!(!a.allows("run") && a.allows("edit"));
        assert_eq!(a.deny(), ["run", "web", "app:github"]);
        assert_eq!(denied_tools(&a.deny()), ["Bash", "WebFetch", "WebSearch", "RemoteTrigger", "SendMessage", "CronCreate", "ScheduleWakeup"]);
        assert!(denied_tools(&[]).is_empty());
    }

    #[test]
    fn save_list_brief_delete() {
        let dir = std::env::temp_dir().join(format!("bs-agents-{}", now_ms()));
        let ag = Agents::open(dir.clone());
        assert!(ag.save(agent("  ")).is_err());
        let a = ag.save(agent("Kira")).unwrap();
        assert!(!a.id.is_empty());
        std::thread::sleep(std::time::Duration::from_millis(3));
        let mut b = agent("Andre");
        b.computer = Computer { kind: "ssh".into(), ..Computer::default() };
        assert!(ag.save(b.clone()).is_err(), "ssh needs a host");
        b.computer.host = "10.0.0.5".into();
        let b = ag.save(b).unwrap();
        assert_eq!(ag.list().iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Kira", "Andre"]);
        let brief = ag.brief(&b);
        assert!(brief.contains("You are Andre") && brief.contains("10.0.0.5") && brief.contains("computer_run"));
        assert!(ag.computer_env(&a).is_none());
        assert_eq!(ag.status(&a.id).unwrap().state, "ready");
        assert!(ag.get("../x").is_none());
        ag.delete(&a.id).unwrap();
        assert_eq!(ag.list().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Runs only where Docker answers (skipped elsewhere).
    #[test]
    fn docker_computer() {
        if docker(&["info", "--format", "{{.ServerVersion}}"]).is_err() {
            eprintln!("no docker; skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("bs-agents-dk-{}", now_ms()));
        let ag = Agents::open(dir.clone());
        let mut a = agent("Box");
        a.computer = Computer { kind: "docker".into(), image: "alpine:3.20".into(), ..Computer::default() };
        let a = ag.save(a).unwrap();
        let env = ag.computer_env(&a).unwrap();
        assert_eq!(ag.start(&a.id).unwrap().state, "running");
        assert!(write(&env, "notes/hello.txt", "hi there").unwrap().contains("8 bytes"));
        assert_eq!(read(&env, "notes/hello.txt").unwrap(), "hi there");
        assert!(run(&env, "uname -s", Duration::from_secs(30)).unwrap().contains("Linux"));
        assert!(run(&env, "exit 3", Duration::from_secs(30)).unwrap_err().to_string().contains("exit 3"));
        // Files live in the agent's folder on this machine.
        assert!(dir.join(&a.id).join("computer/notes/hello.txt").exists());
        assert_eq!(ag.stop(&a.id).unwrap().state, "stopped");
        ag.delete(&a.id).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("a'b c"), "'a'\\''b c'");
    }

    fn utc_ms(y: i64, m: i64, d: i64, h: i64, min: i64) -> u64 {
        let day = civil::days_from_civil(y, m, d);
        ((day * 86_400 + h * 3_600 + min * 60) * 1_000) as u64
    }

    fn timed(kind: &str, time: &str) -> Routine {
        Routine { name: "Job".into(), kind: kind.into(), time: time.into(), enabled: true, ..Routine::default() }
    }

    #[test]
    fn next_run_maths() {
        let daily = timed("daily", "08:00");
        assert_eq!(daily.next_after(utc_ms(2026, 10, 17, 6, 0)), utc_ms(2026, 10, 17, 8, 0));
        assert_eq!(daily.next_after(utc_ms(2026, 10, 17, 20, 0)), utc_ms(2026, 10, 18, 8, 0));
        assert_eq!(daily.next_after(utc_ms(2026, 10, 17, 8, 0)), utc_ms(2026, 10, 18, 8, 0), "strictly after");
        // Weekdays skip the weekend: Friday evening and Saturday both wake Monday.
        let weekdays = timed("weekdays", "08:00");
        assert_eq!(weekdays.next_after(utc_ms(2026, 10, 16, 9, 0)), utc_ms(2026, 10, 19, 8, 0));
        assert_eq!(weekdays.next_after(utc_ms(2026, 10, 17, 10, 0)), utc_ms(2026, 10, 19, 8, 0));
        assert_eq!(weekdays.next_after(utc_ms(2026, 10, 19, 6, 0)), utc_ms(2026, 10, 19, 8, 0));
        // Weekly picks the next chosen weekday.
        let weekly = Routine { kind: "weekly".into(), time: "07:30".into(), days: vec![3, 1], enabled: true, ..Routine::default() };
        assert_eq!(weekly.next_after(utc_ms(2026, 10, 17, 10, 0)), utc_ms(2026, 10, 19, 7, 30));
        assert_eq!(weekly.next_after(utc_ms(2026, 10, 19, 6, 0)), utc_ms(2026, 10, 19, 7, 30));
        assert_eq!(weekly.next_after(utc_ms(2026, 10, 19, 7, 30)), utc_ms(2026, 10, 21, 7, 30));
        // Every N minutes counts from now; a paused routine never wakes.
        let every = Routine { kind: "every".into(), every_minutes: 90, enabled: true, ..Routine::default() };
        assert_eq!(every.next_after(utc_ms(2026, 10, 17, 10, 0)), utc_ms(2026, 10, 17, 11, 30));
        let paused = Routine { enabled: false, ..daily.clone() };
        assert_eq!(paused.next_after(utc_ms(2026, 10, 17, 6, 0)), 0);
    }

    #[test]
    fn next_run_round_trips_a_negative_offset() {
        // UTC-5: 12:00Z is 07:00 local, so 08:00 local is 13:00Z the same day.
        let r = Routine { kind: "daily".into(), time: "08:00".into(), utc_offset_minutes: -300, enabled: true, ..Routine::default() };
        let next = r.next_after(utc_ms(2026, 10, 17, 12, 0));
        assert_eq!(next, utc_ms(2026, 10, 17, 13, 0));
        let local = next as i64 - 300 * 60_000;
        assert_eq!(civil::civil_from_days(local.div_euclid(civil::DAY_MS)), (2026, 10, 17));
        assert_eq!(local.rem_euclid(civil::DAY_MS), 8 * 3_600_000);
        assert_eq!(civil::weekday(local.div_euclid(civil::DAY_MS)), 6, "a Saturday");
    }

    #[test]
    fn routine_summaries() {
        assert_eq!(timed("daily", "08:00").summary(), "Every day at 08:00");
        assert_eq!(timed("weekdays", "08:00").summary(), "Weekdays at 08:00");
        let weekly = Routine { kind: "weekly".into(), time: "07:30".into(), days: vec![3, 1], enabled: true, ..Routine::default() };
        assert_eq!(weekly.summary(), "Wakes Mon, Wed at 07:30");
        let every = Routine { kind: "every".into(), every_minutes: 360, enabled: true, ..Routine::default() };
        assert_eq!(every.summary(), "Every 6 hours");
    }

    #[test]
    fn routine_save_validates_and_persists() {
        let dir = std::env::temp_dir().join(format!("bs-agents-routines-{}", now_ms()));
        let ag = Agents::open(dir.clone());
        let a = ag.save(agent("Kira")).unwrap();
        let daily = timed("daily", "08:00");
        for bad in [
            Routine { name: "  ".into(), ..daily.clone() },
            Routine { kind: "hourly".into(), ..daily.clone() },
            Routine { time: "8".into(), ..daily.clone() },
            Routine { time: "25:00".into(), ..daily.clone() },
            Routine { kind: "weekly".into(), days: vec![], ..daily.clone() },
            Routine { kind: "weekly".into(), days: vec![0, 8], ..daily.clone() },
            Routine { kind: "every".into(), every_minutes: 4, ..daily.clone() },
        ] {
            assert!(ag.routine_save(&a.id, bad).is_err());
        }
        let saved = ag.routine_save(&a.id, daily).unwrap();
        assert_eq!(saved.routines.len(), 1);
        let r = &saved.routines[0];
        assert!(!r.id.is_empty() && r.name == "Job");
        assert!(r.next_run > now_ms());
        // It lives on the bot's own agent.json, not in a store of its own.
        let again = Agents::open(dir.clone()).get(&a.id).unwrap();
        assert_eq!(again.routines.len(), 1);
        assert_eq!(again.routines[0].id, r.id);
        // Upsert by id edits in place.
        let mut edit = again.routines[0].clone();
        edit.name = "Evening job".into();
        edit.time = "18:00".into();
        let saved = ag.routine_save(&a.id, edit).unwrap();
        assert_eq!(saved.routines.len(), 1);
        assert_eq!(saved.routines[0].name, "Evening job");
        // The scheduler's bookkeeping reschedules from when it fired; "run
        // now" is a do-it-now, not a reschedule.
        let at = now_ms() + 60_000;
        let saved = ag.routine_ran(&a.id, &saved.routines[0].id, at).unwrap();
        assert_eq!((saved.routines[0].last_run, saved.routines[0].runs), (at, 1));
        assert_eq!(saved.routines[0].next_run, saved.routines[0].next_after(at));
        let next = saved.routines[0].next_run;
        let saved = ag.routine_posted(&a.id, &saved.routines[0].id).unwrap();
        assert_eq!(saved.routines[0].next_run, next, "run now is not a reschedule");
        assert!(saved.routines[0].last_run > 0);
        assert_eq!(saved.routines[0].runs, 2);
        let saved = ag.routine_delete(&a.id, &saved.routines[0].id).unwrap();
        assert!(saved.routines.is_empty());
        assert!(ag.routine_delete(&a.id, "nope").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_cleans_routines_and_refreshes_a_passed_wake() {
        let dir = std::env::temp_dir().join(format!("bs-agents-clean-{}", now_ms()));
        let ag = Agents::open(dir.clone());
        let past = now_ms().saturating_sub(86_400_000);
        let mut a = agent("Kira");
        a.routines = vec![
            Routine { name: "Digest".into(), kind: "daily".into(), time: "8:00".into(), enabled: true, next_run: past, ..Routine::default() },
            Routine { name: "  ".into(), kind: "daily".into(), time: "08:00".into(), enabled: true, ..Routine::default() },
            Routine { name: "Broken".into(), kind: "every".into(), every_minutes: 0, enabled: true, ..Routine::default() },
        ];
        let saved = ag.save(a).unwrap();
        assert_eq!(saved.routines.len(), 1);
        assert_eq!(saved.routines[0].time, "08:00");
        assert!(saved.routines[0].next_run > now_ms());
        assert!(!saved.routines[0].id.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn brief_in_names_the_project_personality_and_routines() {
        let dir = std::env::temp_dir().join(format!("bs-agents-brief-{}", now_ms()));
        let ag = Agents::open(dir.clone());
        let project = dir.join("acme-site");
        std::fs::create_dir_all(&project).unwrap();
        let mut a = agent("Kira");
        a.personality = "Be brief.".into();
        a.routines = vec![timed("daily", "08:00")];
        let a = ag.save(a).unwrap();
        let brief = ag.brief_in(&a, Some(&project));
        assert!(brief.contains("Be brief."));
        assert!(brief.contains("acme-site project"));
        assert!(brief.contains("Every day at 08:00"));
        assert!(brief.contains("Routine · <name>"));
        assert!(!ag.brief(&a).contains("acme-site project"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
