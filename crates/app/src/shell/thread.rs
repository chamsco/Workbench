//! The thread view (UI4): the open thread's transcript, the composer that sends to core, and the
//! stage bar's usage. Rows open here from the sidebar; core's change channel keeps it current.
//!
//! Left out until core has them per thread (docs/cauldron/UI3-CORE-MAP.md): ask cards, the stage
//! stepper, teammates' messages and avatars.

use super::*;
use backspace_core::chat::{Ctx, Msg, Role, Status, Thread};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui::{linear_color_stop, linear_gradient};
use gpui_kit::prelude::FluentBuilder;

/// One transcript entry as drawn.
#[derive(Debug, PartialEq)]
pub(super) struct Line {
    pub id: String,
    pub mine: bool,
    pub text: String,
    pub status: Status,
    pub error: Option<String>,
    /// Who answered and at what cost, under a reply.
    pub meta: Option<String>,
    /// The model that wrote a reply, drawn above it: as the backend reported it, else the route's.
    pub model: Option<String>,
    /// The reply as text and steps, when it recorded any steps (board A); else empty.
    pub parts: Vec<super::ledger::Part>,
    /// When it was sent (ms), for the wait timer.
    pub at: u64,
    /// Why nothing is arriving, when the target says (OpenCode retrying a throttled provider).
    pub wait: Option<String>,
}

/// A failed reply, read for people: `reason` in words, the exit code, a hint for causes we know,
/// and the CLI's own lines for Show output.
pub(super) struct Failure {
    pub reason: String,
    pub code: Option<String>,
    pub hint: Option<String>,
    pub output: Vec<String>,
}

/// Core writes a CLI failure as "<cli> exited with code N:" and then the CLI's own output.
pub(super) fn failure(e: &str) -> Failure {
    let mut ls: Vec<&str> = e.lines().map(str::trim_end).collect();
    let mut code = None;
    if let Some(first) = ls.first().copied() {
        if let Some(rest) = first.split("exited with code ").nth(1) {
            let n: String = rest.chars().take_while(|ch| ch.is_ascii_digit() || *ch == '-').collect();
            code = (!n.is_empty()).then(|| format!("exit {n}"));
            // Core may carry the CLI's first line on the same line, after the colon.
            let tail = rest[n.len()..].trim_start_matches(':').trim();
            if tail.is_empty() {
                ls.remove(0);
            } else {
                ls[0] = tail;
            }
        }
    }
    let output: Vec<String> = ls.iter().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect();
    // The last line that reads as an error, without its "Error:" prefix.
    let said = output.iter().rev().find(|l| !l.trim_start().starts_with('>')).map(|l| l.trim().trim_start_matches("Error:").trim().to_string());
    let low = e.to_lowercase();
    let (reason, hint) = if low.contains("route not found") || (low.contains("not found") && low.contains("404")) {
        ("The model's provider answered 404, route not found".to_string(), Some("This CLI's model points at a provider with no chat route. Pick another model, or fix the provider in the CLI's own config.".to_string()))
    } else if low.contains("not installed") || low.contains("no such file") || low.contains("cannot find the file") {
        (said.clone().unwrap_or_else(|| "The CLI could not start".into()), Some("Install it, or pick another CLI with Change model.".to_string()))
    } else if low.contains("unauthorized") || low.contains("401") || low.contains("not logged in") || low.contains("sign in") {
        (said.clone().unwrap_or_else(|| "Not signed in".into()), Some("Sign in to this CLI in Settings, then Retry.".to_string()))
    } else if low.contains("rate limit") || low.contains("429") || low.contains("quota") {
        (said.clone().unwrap_or_else(|| "Rate limited".into()), Some("The subscription hit its limit. Retry later, or pick another CLI or account.".to_string()))
    } else {
        (said.clone().unwrap_or_else(|| e.lines().next().unwrap_or("The reply failed").to_string()), None)
    };
    // When the output is only the reason itself, there is nothing more to show.
    let output = if output.len() == 1 && said.as_deref() == Some(output[0].trim().trim_start_matches("Error:").trim()) && code.is_none() { Vec::new() } else { output };
    Failure { reason, code, hint, output }
}

pub(super) fn lines(t: &Thread) -> Vec<Line> {
    t.messages.iter().map(|m| Line { id: m.id.clone(), mine: m.role == Role::User, text: m.text.clone(), status: m.status, error: m.error.clone(), meta: meta(m), model: model(m, t), parts: reply_parts(m), at: m.at, wait: m.wait.clone() }).collect()
}

fn model(m: &Msg, t: &Thread) -> Option<String> {
    if m.role == Role::User {
        return None;
    }
    let r = m.via.as_ref().unwrap_or(&t.route);
    m.model.clone().or_else(|| r.model.clone()).or_else(|| Some(r.provider.clone())).filter(|s| !s.is_empty())
}

/// A reply's text and steps, when it recorded steps. In flight is `Sending` or `Streaming`, unless
/// the app closed under it.
fn reply_parts(m: &Msg) -> Vec<super::ledger::Part> {
    if m.role == Role::User || m.acts.is_empty() {
        return Vec::new();
    }
    super::ledger::parts(m, matches!(m.status, Status::Sending | Status::Streaming) && !m.interrupted)
}

fn meta(m: &Msg) -> Option<String> {
    if m.role == Role::User {
        return None;
    }
    let mut p = Vec::new();
    if let Some(n) = &m.author_name {
        p.push(n.clone());
    }
    if let Some(c) = m.cost_usd {
        p.push(format!("${c:.2}"));
    }
    (!p.is_empty()).then(|| p.join(" · "))
}

/// What the thread has cost so far, from the replies that reported a cost.
pub(super) fn cost(t: &Thread) -> f64 {
    // fold from +0.0: an empty f64 sum is -0.0, which prints as "$-0.00".
    t.messages.iter().filter_map(|m| m.cost_usd).fold(0.0, |a, b| a + b)
}

/// How a reply was made, from its trace: one plain line per kind of tool call, in the order each
/// kind first happened ("Read 12 files", "Made 3 edits"). Core does not time the thinking, so
/// "Thought" carries no duration.
pub(super) fn steps(t: &backspace_core::trace::Trace, thought: bool) -> Vec<String> {
    #[derive(PartialEq)]
    enum K {
        Read,
        Edit,
        Run,
        Web,
        Code,
        Agent,
        Plan,
        Other(String),
    }
    // (kind, calls, failed, distinct files)
    let mut ks: Vec<(K, usize, usize, std::collections::BTreeSet<String>)> = Vec::new();
    for sp in t.spans.iter().filter(|s| s.kind == "tool") {
        let k = match sp.name.as_str() {
            "Read" => K::Read,
            "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => K::Edit,
            "Bash" | "PowerShell" => K::Run,
            "WebSearch" | "WebFetch" => K::Web,
            "Grep" | "Glob" | "LS" => K::Code,
            "Task" | "Agent" => K::Agent,
            "TodoWrite" => K::Plan,
            n => K::Other(n.to_string()),
        };
        let i = match ks.iter().position(|e| e.0 == k) {
            Some(i) => i,
            None => {
                ks.push((k, 0, 0, Default::default()));
                ks.len() - 1
            }
        };
        let e = &mut ks[i];
        e.1 += 1;
        e.2 += usize::from(sp.error.is_some());
        let file = serde_json::from_str::<serde_json::Value>(&sp.input).ok().and_then(|v| v.get("file_path").or_else(|| v.get("path")).and_then(|p| p.as_str()).map(str::to_string));
        e.3.insert(file.unwrap_or_else(|| format!("#{}", e.1)));
    }
    let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
    let mut out: Vec<String> = thought.then(|| "Thought".to_string()).into_iter().collect();
    for (k, calls, failed, files) in ks {
        let line = match k {
            K::Read => format!("Read {}", n(files.len(), "file", "files")),
            K::Edit => format!("Made {}", n(calls, "edit", "edits")),
            K::Run => format!("Ran {}", n(calls, "command", "commands")),
            K::Web => "Searched the web".into(),
            K::Code => "Searched the code".into(),
            K::Agent => format!("Ran {}", n(calls, "subagent", "subagents")),
            K::Plan => "Updated the plan".into(),
            // MCP tools arrive as mcp__server__tool: say "Used linear: get_issue".
            K::Other(name) => match name.strip_prefix("mcp__").and_then(|r| r.split_once("__")) {
                Some((server, tool)) => format!("Used {server}: {tool}"),
                None => format!("Used {name}"),
            },
        };
        out.push(if failed > 0 { format!("{line}, {failed} failed") } else { line });
    }
    out
}

/// What a reply that is not finished says in place of its text.
fn pending(s: Status) -> Option<&'static str> {
    match s {
        Status::Stopped => Some("Stopped"),
        s => live(s),
    }
}

/// The first `n` characters of `s`.
fn head(s: &str, n: usize) -> &str {
    s.char_indices().nth(n).map_or(s, |(i, _)| &s[..i])
}

/// How much of a live paragraph of `total` characters to show at `now` (ms), from where it was
/// (`shown`, when last drawn at `last`). Providers send bursts (OpenCode: half a minute of
/// nothing, then a paragraph in under a second), so the text comes out at a steady pace that
/// gains on what has arrived: never slower than 60 characters a second, about a third of a second
/// behind at most.
fn pace(shown: f32, last: u64, total: usize, now: u64) -> f32 {
    let dt = now.saturating_sub(last).min(200) as f32;
    let behind = (total as f32 - shown).max(0.0);
    (shown + dt * (behind / 300.0).max(0.06)).min(total as f32)
}

/// A reply on its way: what its status line says.
fn live(s: Status) -> Option<&'static str> {
    match s {
        Status::Sending => Some("Waiting for the reply"),
        Status::Streaming => Some("Writing"),
        _ => None,
    }
}

/// A 6px dot in a 16px slot, then the label: the same slot and 8px gap as the step lines, so the
/// label starts where their text does. It stays under the text while the reply streams. No agent
/// bust: figures belong to the Agents page (Charles, 13 Nov).
fn status_line(i: usize, label: String, writing: bool, since: Option<u64>, now: u64, still: bool, pal: &'static Palette) -> impl IntoElement {
    let (grey, ink) = (pal.fg3, pal.fg);
    div()
        .id(("live", i))
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(20.))
        .text_size(px(12.5))
        .text_color(c(grey))
        .aria_label(label.clone())
        .child(div().id(("live-slot", i)).size(px(16.)).flex_none().flex().items_center().justify_center().child(super::mark::mark(("live-mark", i), if writing { Mark::Working } else { Mark::Waiting }, super::mark::SIZE, c(grey))).test_support())
        // The silver pulse: the same sweep as a running step, so every live line moves alike.
        .child(div().id(("live-text", i)).min_w_0().truncate().child(super::ledger::shimmer(&label, now, c(grey), c(ink), still)).test_support())
        // How long it has waited, as the step lines count theirs.
        .when_some(since, |d, t0| d.child(div().ml_auto().flex_none().font_family(MONO).font_features(tnum()).text_size(px(11.5)).child(super::ledger::dur(super::ledger::now_ms().saturating_sub(t0)))))
        .test_support()
}

/// The settings a send carries: the saved prefs, and Plan mode when it is on.
pub(super) fn ctx(plan: bool) -> Ctx {
    Ctx { prefs: Prefs::load(), memory: None, agents: Some(agents()), plan, ask: false }
}

impl Cauldron {
    /// + in the reply box: pick files to send with the next message.
    fn pick_reply_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some("Attach".into()) });
        cx.spawn_in(window, async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
            if let Ok(Ok(Some(ps))) = paths.await {
                let _ = this.update(cx, |c, cx| {
                    c.reply_files.extend(ps);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// The files waiting to go with the next reply, each with an x.
    fn reply_files_el(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.reply_files.is_empty() {
            return None;
        }
        let mut row = div().id("reply-files").flex().flex_wrap().gap(px(6.));
        for (i, p) in self.reply_files.iter().enumerate() {
            let name = p.file_name().map_or_else(|| "file".into(), |n| n.to_string_lossy().to_string());
            row = row.child(
                div()
                    .id(("reply-file", i))
                    .h(px(26.))
                    .pl(px(8.))
                    .pr(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(c(pal.line))
                    .bg(c(pal.inset))
                    .text_size(px(12.))
                    .text_color(c(pal.fg2))
                    .aria_label(name.clone())
                    .child(glyph("paperclip", 12., c(pal.fg3)))
                    .child(name)
                    .child(
                        div()
                            .id(("reply-file-x", i))
                            .size(px(18.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(radius::TAG))
                            .cursor_pointer()
                            .hover(|d| d.bg(c(pal.hover)))
                            .aria_label("Remove")
                            .child(glyph("x", 11., c(pal.fg3)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if i < this.reply_files.len() {
                                    this.reply_files.remove(i);
                                }
                                cx.notify();
                            })),
                    )
                    .test_support(),
            );
        }
        Some(row.test_support().into_any_element())
    }

    /// What a send carries from the composer's mode (Ask · Plan · Act).
    pub(super) fn mode_ctx(&self) -> Ctx {
        let mut c = ctx(self.plan);
        c.ask = self.ask && !self.plan;
        c
    }
}

/// The user's agents (familiars and their crews), opened once.
pub(super) fn agents() -> Arc<backspace_core::agents::Agents> {
    static A: std::sync::OnceLock<Arc<backspace_core::agents::Agents>> = std::sync::OnceLock::new();
    A.get_or_init(|| Arc::new(backspace_core::agents::Agents::open(Prefs::data_dir().join("agents")))).clone()
}

/// What the coding CLIs may do without asking (the `cli_permission` pref), as the board's trust
/// chip names it: "edits" is Ask first, "auto" Approve routine, "full" Full access.
pub(super) const TRUST: [(&str, &str); 3] = [("edits", "Ask first"), ("auto", "Approve routine"), ("full", "Full access")];

pub(super) fn trust_label(p: &Prefs) -> &'static str {
    TRUST.iter().find(|(k, _)| *k == p.cli_permission).map_or("Ask first", |(_, l)| l)
}

/// The open-in split button's targets; its left half repeats the last one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OpenIn {
    Folder,
    Editor,
}

impl OpenIn {
    pub(super) const ALL: [OpenIn; 2] = [OpenIn::Folder, OpenIn::Editor];
    pub(super) fn label(self) -> &'static str {
        match self {
            OpenIn::Folder => if cfg!(target_os = "macos") { "Show in Finder" } else { "Show in Explorer" },
            OpenIn::Editor => "Open in VS Code",
        }
    }
}

/// Which scroll tick is current: the last response at or above the transcript's top row.
pub(super) fn current_tick(replies: &[usize], top: usize) -> usize {
    replies.iter().rposition(|&r| r <= top).unwrap_or(0)
}

/// The branch checked out in `dir`, from `.git/HEAD`; None outside a repo or on a detached HEAD.
pub(super) fn branch(dir: &str) -> Option<String> {
    let head = std::fs::read_to_string(std::path::Path::new(dir).join(".git").join("HEAD")).ok()?;
    head.trim().strip_prefix("ref: refs/heads/").map(str::to_string)
}

/// Efforts the chip cycles through; None is Auto, the CLI's own default.
pub(super) const EFFORTS: [Option<&str>; 5] = [None, Some("low"), Some("medium"), Some("high"), Some("max")];

/// `a` mixed toward `b` by `t`, as CSS `color-mix(in srgb, b t, a)`.
/// An overlay `a` mixes its alpha toward `b`'s full opacity too.
pub(super) fn mix(a: impl Into<Rgba>, b: Rgb, t: f32) -> Hsla {
    let Rgba(a, al) = a.into();
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    c(Rgba(Rgb(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2)), al + (1.0 - al) * t))
}

/// Each finished reply's steps, read from its trace once (render never touches the disk).
/// Adds step lines for replies not yet in `map`. A saved trace never changes, so each one is
/// read once per thread load, not on every core poke.
fn fill_steps(chats: &Chats, t: Option<&Thread>, map: &mut std::collections::HashMap<String, Vec<String>>) {
    for m in t.into_iter().flat_map(|t| t.messages.iter()).filter(|m| m.role != Role::User) {
        if map.contains_key(&m.id) {
            continue;
        }
        if let Some(tr) = m.trace.as_ref().and_then(|id| chats.trace(id)) {
            map.insert(m.id.clone(), steps(&tr, !m.thinking.is_empty()));
        }
    }
}

impl Cauldron {
    /// Opens a thread in the main card. Reading it through core marks it read.
    pub(super) fn open_thread(&mut self, id: String) {
        self.usage_page = false;
        self.section_page = false;
        let Some((_, chats)) = &self.core else { return };
        self.thread = chats.thread(&id);
        self.steps.clear();
        self.term_cache.clear();
        fill_steps(chats, self.thread.as_ref(), &mut self.steps);
        self.term_blocks = super::terminal::blocks(chats, self.thread.as_ref(), &mut self.term_cache);
        self.term_scroll.scroll_to_bottom();
        self.familiar = None;
        self.coordinator = Some(self.thread.as_ref().map_or_else(|| "Thread".into(), |t| t.title.clone().into()));
        self.agents.clear();
        self.idle.clear();
        self.active = 0;
        self.selected = Some(id.clone());
        if self.open.as_deref() != Some(id.as_str()) {
            self.diff_since = None;
            self.restore_ask = false;
        }
        self.open = Some(id);
        self.open_since = Some(std::time::Instant::now());
        self.send_error = None;
        self.threads = Threads::Ready(chats.list());
        self.sync_sides();
    }

    /// Re-reads the list and the open thread from core.
    pub(super) fn refresh(&mut self) {
        if let Some((_, chats)) = &self.core {
            self.threads = Threads::Ready(chats.list());
            if let Some(id) = &self.open {
                self.thread = chats.peek(id);
                fill_steps(chats, self.thread.as_ref(), &mut self.steps);
                self.term_blocks = super::terminal::blocks(chats, self.thread.as_ref(), &mut self.term_cache);
            }
        }
        self.sync_sides();
    }

    /// Keeps the view current: core pokes `changes` whenever a thread changes.
    pub(super) fn watch(&mut self, changes: async_channel::Receiver<()>, cx: &mut Context<Self>) {
        self.watch = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while changes.recv().await.is_ok() {
                if this
                    .update(cx, |c, cx| {
                        c.refresh();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    /// Every second, off the UI thread: start the sub-agents streams asked for (`delegate`)
    /// and deliver letters between streams, then start whatever waits for a free stream.
    pub(super) fn pump(&mut self, cx: &mut Context<Self>) {
        let Some((_, chats)) = self.core.as_ref() else { return };
        let chats = chats.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| loop {
            cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
            if this.update(cx, |_, _| ()).is_err() {
                break;
            }
            let c = chats.clone();
            cx.background_executor().spawn(async move { c.pump(ctx(false)) }).await;
        })
        .detach();
    }

    pub(super) fn busy(&self) -> bool {
        match (&self.core, &self.open) {
            (Some((_, chats)), Some(id)) => chats.is_busy(id),
            _ => false,
        }
    }

    /// Sends the composer's text to the open thread; a refusal shows under the composer.
    pub(super) fn send_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(id), Some((st, _))) = (self.open.clone(), self.composer.as_ref()) else { return };
        let st = st.clone();
        let mut text = st.read(cx).value().trim().to_string();
        if text.is_empty() && self.reply_files.is_empty() {
            return;
        }
        // A slash command runs here; some send a prompt in its place.
        if let Some(out) = self.run_slash(&text.clone(), window, cx) {
            match out {
                Some(t) => text = t,
                None => {
                    st.update(cx, |s, cx| s.set_value("", window, cx));
                    self.refresh();
                    cx.notify();
                    return;
                }
            }
        }
        let Some((_, chats)) = self.core.as_ref() else { return };
        let chats = chats.clone();
        // The files added with + go along (read first, so a bad one sends nothing).
        let sent = (|| {
            use base64::Engine;
            let mut atts = Vec::new();
            for p in &self.reply_files {
                let name = p.file_name().map_or_else(|| "file".into(), |n| n.to_string_lossy().to_string());
                let b = std::fs::read(p).map_err(|e| anyhow::anyhow!("Couldn't read {name}: {e}"))?;
                atts.push(chats.attach(&id, &name, super::newthread::mime(&name), &base64::engine::general_purpose::STANDARD.encode(b))?);
            }
            chats.send(&id, &text, atts, None, self.mode_ctx())
        })();
        match sent {
            Ok(()) => {
                self.send_error = None;
                self.reply_files.clear();
                st.update(cx, |s, cx| s.set_value("", window, cx));
            }
            Err(e) => self.send_error = Some(e.to_string()),
        }
        self.refresh();
        cx.notify();
    }

    /// The composer's input, made once with the window it lives in.
    pub(super) fn ensure_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.is_some() {
            return;
        }
        let st = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 8).submit_on_enter(true).placeholder(super::newthread::REPLY_PLACEHOLDER));
        let sub = cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { shift: false, .. } => {
                if !this.slash_enter(window, cx) {
                    this.send_composer(window, cx);
                }
            }
            InputEvent::Change => {
                this.slash_closed = false;
                this.slash_sel = 0;
                cx.notify();
            }
            _ => {}
        });
        self.composer = Some((st, sub));
    }

    /// The main card's content: the open thread, a 1:1, or what stands in for them.
    pub(super) fn card_body(&self, pal: &'static Palette, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let fg3 = c(pal.fg3);
        let centred = |id: &'static str, title: String, body: &'static str| {
            div()
                .id(id)
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.))
                .aria_label(title.clone())
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg2)).child(title))
                .child(div().text_size(px(12.)).text_color(fg3).child(body))
                .test_support()
                .into_any_element()
        };
        if self.active > 0 {
            let name = self.agents.get(self.active - 1).map_or_else(String::new, |a| a.name.to_string());
            // ponytail: agent tabs are the debug fixture until core lists a thread's agents (THREAD-STATE.md).
            return centred("one-to-one", format!("1:1 with {name}"), "Agent 1:1 transcripts come with core's per-thread agents.");
        }
        match (&self.open, &self.thread) {
            (Some(_), Some(t)) => self.thread_view(t, pal, cx),
            (Some(_), None) => centred("thread-gone", "This thread is no longer here".into(), "It was deleted, or its file could not be read."),
            (None, _) if self.start.captain.is_some() => self.captain_view(pal, cx),
            (None, _) => self.new_thread_view(pal, window, cx),
        }
    }

    /// A failed reply (design system: status is a glyph plus a word; Charles, 21 Nov: the raw red
    /// CLI text was hard to read). The cause in one sentence, a hint when the cause is known, the
    /// fix first (Change model opens this stream's model picker), Retry, and the CLI's own output
    /// folded under Show output.
    #[allow(clippy::too_many_arguments)]
    fn failure_card(&self, i: usize, msg_id: &str, e: &str, last: bool, thread_id: &str, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let f = failure(e);
        let (fg, fg2, fg3, bad) = (c(pal.fg), c(pal.fg2), c(pal.fg3), c(pal.bad));
        let shown = self.err_open.contains(msg_id);
        let mut card = div()
            .id(("msg-error", i))
            .max_w(px(560.))
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .border_1()
            .border_color(c(pal.line_strong))
            .bg(c(pal.inset))
            .overflow_hidden()
            .aria_label(e.to_string())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .pt(px(9.))
                    .pb(px(if f.hint.is_some() { 4. } else { 9. }))
                    .text_size(px(13.))
                    .child(glyph("x", 14., bad))
                    .child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(bad).child("Failed"))
                    .child(div().id(("msg-error-reason", i)).flex_1().min_w_0().text_color(fg).child(f.reason.clone()).test_support())
                    .when_some(f.code.clone(), |d, code| d.child(div().flex_none().font_family(crate::ui::SANS_MONO).text_size(px(11.5)).text_color(fg3).child(code))),
            )
            .when_some(f.hint, |d, h| d.child(div().pl(px(34.)).pr(px(12.)).pb(px(10.)).text_size(px(12.5)).line_height(px(18.)).text_color(fg2).child(h)));
        let btn = |id: (&'static str, usize), label: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(radius::INNER))
                .text_size(px(12.5))
                .cursor_pointer()
                .shadow(super::lift(super::Lift::Button, pal))
                .aria_label(label)
                .when(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)).hover(|d| d.bg(c(pal.accent_hover))))
                .when(!primary, |d| d.border_1().border_color(c(pal.line_strong)).text_color(fg).hover(|d| d.bg(c(pal.hover))))
                .child(label)
        };
        let mut row = div().flex().items_center().gap(px(6.)).pl(px(34.)).pr(px(12.)).pb(px(10.));
        if last {
            let tid = thread_id.to_string();
            row = row
                .child(
                    btn(("change-model", i), "Change model", true)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.start.route_menu = Some(super::subs::PickFor::Thread(tid.clone()));
                            cx.notify();
                        }))
                        .test_support(),
                )
                .child(
                    btn(("retry", i), "Retry", false)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let (Some(id), Some((_, chats))) = (this.open.clone(), this.core.as_ref()) {
                                // A retry of a Plan reply stays a plan in core; nothing to add here.
                                if let Err(e) = chats.retry(&id, ctx(false)) {
                                    this.send_error = Some(e.to_string());
                                }
                            }
                            this.refresh();
                            cx.notify();
                        }))
                        .test_support(),
                );
        }
        if !f.output.is_empty() {
            let mid = msg_id.to_string();
            row = row.child(
                div()
                    .id(("show-output", i))
                    .h(px(26.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .rounded(px(radius::INNER))
                    .text_size(px(12.5))
                    .text_color(fg3)
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)).text_color(fg2))
                    .aria_label(if shown { "Hide output" } else { "Show output" })
                    .child(glyph(if shown { "chevron-down" } else { "chevron-right" }, 12., fg3))
                    .child(if shown { "Hide output" } else { "Show output" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.err_open.remove(&mid) {
                            this.err_open.insert(mid.clone());
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        card = card.child(row);
        if shown {
            card = card.child(
                div()
                    .id(("error-output", i))
                    .border_t_1()
                    .border_color(c(pal.line))
                    .px(px(12.))
                    .py(px(8.))
                    .font_family(crate::ui::SANS_MONO)
                    .text_size(px(11.5))
                    .line_height(px(17.))
                    .text_color(fg3)
                    .children(f.output.iter().map(|l| div().child(l.clone())))
                    .test_support(),
            );
        }
        card.test_support().into_any_element()
    }

    fn thread_view(&self, t: &Thread, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        // The transcript fades in over the second half of the carry after a thread opens.
        let now = std::time::Instant::now();
        let now_ms = super::ledger::now_ms();
        let open_k = self.open_since.map_or(1.0, |t0| cauldron_chrome::motion::fade_in(crate::motion::run(now, t0, cauldron_chrome::motion::time::CARRY)));
        let bubble_k = self.bubble_since.map_or(1.0, |t0| crate::motion::run(now, t0, cauldron_chrome::motion::time::CARRY));
        let mut msgs = div()
            .id("transcript")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.transcript_scroll)
            .flex()
            .flex_col()
            .gap(px(12.))
            .opacity(open_k)
            // A new press starts a new selection; letting go shows the bar for what got selected.
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if this.sel.take().is_some() {
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, ev: &MouseUpEvent, window, cx| this.note_selection(ev.position, window, cx)));
        let ls = lines(t);
        // Keep the newest message in view when a thread opens or grows, in a short pane too.
        let seen = Some((t.id.clone(), ls.len()));
        // While a reply grows, keep following it, unless the reader scrolled up.
        let at_bottom = -self.transcript_scroll.offset().y >= self.transcript_scroll.max_offset().y - px(40.);
        let growing = ls.last().is_some_and(|l| live(l.status).is_some());
        if *self.transcript_seen.borrow() != seen || (growing && at_bottom) {
            self.transcript_scroll.scroll_to_bottom();
            *self.transcript_seen.borrow_mut() = seen;
        }
        // In a narrow or short pane the view tightens and drops what is secondary, so nothing
        // overlaps: less padding under 360px, no cost line under 260px, no composer chips under 160px (Send stays).
        let (bw, bh) = self.thread_box.get();
        let narrow = bw > 0.0 && bw < 360.0;
        let short = bh > 0.0 && bh < 260.0;
        let tiny = bh > 0.0 && bh < 160.0;
        // The newest of the user's bubbles rises into place (a rise and fade, not a carry from
        // the composer box).
        let fresh_mine = ls.iter().rposition(|l| l.mine);
        if ls.is_empty() {
            msgs = msgs.child(
                div()
                    .id("thread-empty")
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .pt(px(8.))
                    .aria_label("No messages yet")
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(fg2).child("No messages yet"))
                    .child(div().text_size(px(12.)).text_color(fg3).child("Say what you want done below."))
                    .test_support(),
            );
        }
        let last = ls.len().saturating_sub(1);
        // This stream's side streams, each linked under the reply it branched off (Charles, 22 Nov).
        let sides: Vec<(u64, String, String)> = match (&self.threads, t.side_of.is_none()) {
            (Threads::Ready(v), true) => {
                let mut x: Vec<_> = v.iter().filter(|s| s.side_of.as_deref() == Some(t.id.as_str())).map(|s| (s.created, s.id.clone(), s.title.clone())).collect();
                x.sort();
                x
            }
            _ => Vec::new(),
        };
        let next_at: Vec<u64> = ls.iter().skip(1).map(|l| l.at).chain(std::iter::once(u64::MAX)).collect();
        // The scroll ticks' targets: one per response, by its row in the transcript.
        let replies: Vec<usize> = ls.iter().enumerate().filter(|(_, l)| !l.mine).map(|(i, _)| i).collect();
        // Only the messages near the view are built: one further is drawn as an empty box of the
        // height it had last frame, so a scroll step in a long stream lays out a screenful, not
        // every message (26 ms at 240 messages before). The boxes are trusted only when last
        // frame drew this stream at this width with these messages and steps; otherwise every
        // message is built once, which measures them again.
        let n = ls.len();
        let vb = self.transcript_scroll.bounds();
        let fit = (t.id.clone(), f32::from(vb.size.width).round() as i32, n, self.steps.values().map(Vec::len).sum::<usize>());
        let measured = vb.size.height > px(0.) && self.transcript_fit.borrow().as_ref() == Some(&fit);
        *self.transcript_fit.borrow_mut() = Some(fit);
        let off = self.transcript_scroll.offset().y;
        let (lo, hi) = (vb.top() - off - vb.size.height, vb.bottom() - off + vb.size.height);
        for (i, l) in ls.into_iter().enumerate() {
            if measured && i + 1 < n && live(l.status).is_none() {
                if let Some(b) = self.transcript_scroll.bounds_for_item(i).filter(|b| b.bottom() < lo || b.top() > hi) {
                    msgs = msgs.child(div().id(("msg", i)).flex_none().w_full().h(b.size.height));
                    continue;
                }
            }
            let streaming = l.status == Status::Streaming && !cx.reduce_motion();
            let mut shown = if l.text.is_empty() { pending(l.status).unwrap_or("").to_string() } else { l.text.clone() };
            // The live paragraph, paced (see `pace`), with a caret at its end.
            let last_text = l.parts.iter().rposition(|p| matches!(p, super::ledger::Part::Text(_)));
            let tail = if !streaming || l.mine {
                None
            } else if l.parts.is_empty() {
                (!l.text.is_empty()).then(|| (format!("{}:", l.id), l.text.clone()))
            } else {
                last_text.filter(|&j| j + 1 == l.parts.len()).and_then(|j| match &l.parts[j] {
                    super::ledger::Part::Text(t) => Some((format!("{}:{j}", l.id), t.clone())),
                    _ => None,
                })
            };
            let paced: Option<String> = tail.map(|(key, text)| {
                let total = text.chars().count();
                let mut map = self.paced.borrow_mut();
                // First seen with text already there (a stream opened mid-reply): no replay.
                let (was, last) = map.get(&key).copied().unwrap_or((total as f32, now_ms));
                let n = pace(was, last, total, now_ms);
                map.insert(key, (n, now_ms));
                if (n as usize) < total {
                    self.pace_behind.set(true);
                }
                head(&text, n as usize).to_string()
            });
            if let (Some(p), true) = (&paced, l.parts.is_empty()) {
                shown = p.clone();
            }
            if l.mine {
                let k = if fresh_mine == Some(i) { bubble_k } else { 1.0 };
                msgs = msgs.child(
                    div()
                        .id(("msg", i))
                        .self_end()
                        .relative()
                        .top(px(8. * (1. - k)))
                        .opacity(k)
                        .max_w(relative(if narrow { 0.9 } else { 0.72 }))
                        .px(px(12.))
                        .py(px(8.))
                        // Design system: your words are a quiet fill, in the reading size.
                        .rounded(px(12.))
                        .bg(c(pal.selected))
                        .text_size(px(14.))
                        .line_height(px(22.))
                        .text_color(fg)
                        .aria_label(l.text.clone())
                        .child(shown)
                        .test_support(),
                );
                continue;
            }
            // ponytail: replies draw as plain text; markdown rendering is a later pass.
            // The full column, the composer's width (Charles, 21 Nov: "using the entire space").
            // The model above the reply (Tuff UI), with its maker's mark.
            let model_el = l.model.clone().map(|md| {
                let mark = super::logos::maker_mark(&md).or_else(|| super::logos::house_maker(&t.route.provider)).and_then(|n| super::logos::image(n, &pal.fg3.to_hex()));
                div()
                    .id(("reply-model", i))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(18.))
                    .text_size(px(11.5))
                    .text_color(fg3)
                    .aria_label(format!("Model: {md}"))
                    .when_some(mark, |d, m| d.child(img(m).size(px(12.)).flex_none()))
                    .child(md)
                    .test_support()
            });
            let mut say = div()
                .id(("msg", i))
                .flex()
                .flex_col()
                .gap(px(6.))
                .when_some(model_el, |d, m| d.child(m))
                .aria_label(if l.text.is_empty() { shown.clone() } else { l.text.clone() })
                .when_some(self.steps.get(&l.id).filter(|s| !s.is_empty() && l.parts.is_empty()), |d, st| {
                    let mut col = div().id(("steps", i)).flex().flex_col().gap(px(2.)).aria_label(st.join(", "));
                    for (j, s) in st.iter().enumerate() {
                        col = col.child(div().flex().items_center().gap(px(8.)).h(px(20.)).text_size(px(12.5)).text_color(fg3).child(div().size(px(16.)).flex_none().flex().items_center().justify_center().child(glyph("check", 13., fg3))).child(div().id(format!("step-text-{i}-{j}")).child(s.clone()).test_support()));
                    }
                    d.child(col.test_support())
                })
                .when_live(l.parts.is_empty() && (!l.text.is_empty() || live(l.status).is_none()), |d| {
                    if l.text.is_empty() {
                        d.child(div().text_size(px(14.)).line_height(px(22.)).text_color(fg3).child(shown.clone()))
                    } else {
                        d.child(self.md_el(&shown, &l.id, paced.as_ref().map(|_| (now_ms, c(pal.accent))), l.at, pal, cx))
                    }
                })
                .when_some(live(l.status).filter(|_| l.parts.is_empty()), |d, label| {
                    let label = l.wait.clone().unwrap_or_else(|| label.to_string());
                    d.child(status_line(i, label, l.status == Status::Streaming && l.wait.is_none(), (l.status == Status::Sending).then_some(l.at), now_ms, cx.reduce_motion(), pal))
                });
            // The ledger (board A): the text and the steps in the order they happened.
            for (j, part) in l.parts.iter().enumerate() {
                say = match part {
                    super::ledger::Part::Text(t) => {
                        let live_tail = paced.as_ref().filter(|_| Some(j) == last_text);
                        say.child(div().id(("part", i * 1000 + j)).child(self.md_el(live_tail.unwrap_or(t), &format!("{}:{j}", l.id), live_tail.map(|_| (now_ms, c(pal.accent))), l.at, pal, cx)))
                    }
                    super::ledger::Part::Step(st) => say.child(self.step_el(st, st.end.is_none(), now_ms, pal, cx)),
                };
            }
            if let Some(e) = &l.error {
                say = say.child(self.failure_card(i, &l.id, e, i == last, &t.id, pal, cx));
            }
            // The reply's foot: Copy (its whole Markdown, Charles 22 Nov), then who wrote it and the cost.
            let whole = l.text.clone();
            let done = live(l.status).is_none() && !whole.trim().is_empty();
            if done || l.meta.is_some() {
                let copied = self.copied.as_deref() == Some(l.id.as_str());
                let lid = l.id.clone();
                say = say.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .text_size(px(12.))
                        .text_color(fg3)
                        .font_features(tnum())
                        .when(done, |d| {
                            d.child(
                                div()
                                    .id(("reply-copy", i))
                                    .h(px(24.))
                                    .px(px(6.))
                                    .ml(px(-6.))
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .rounded(px(radius::TAG))
                                    .cursor_pointer()
                                    .hover(|d| d.bg(c(pal.hover)).text_color(fg))
                                    .aria_label("Copy the reply as Markdown")
                                    .child(glyph(if copied { "check" } else { "copy" }, 13., if copied { c(pal.ok) } else { fg3 }))
                                    .child(if copied { "Copied" } else { "Copy" })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(whole.clone()));
                                        this.copied = Some(lid.clone());
                                        cx.notify();
                                    }))
                                    .test_support(),
                            )
                        })
                        .when_some(l.meta.clone(), |d, m| d.child(div().flex().items_center().gap(px(6.)).child(glyph("terminal", 13., fg3)).child(m))),
                );
            }
            for (k, (_, sid, title)) in sides.iter().enumerate().filter(|(_, (at, _, _))| *at >= l.at && *at < next_at[i]) {
                let sid = sid.clone();
                say = say.child(
                    div()
                        .id(("side-link", k))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h(px(22.))
                        .text_size(px(12.5))
                        .text_color(fg3)
                        .cursor_pointer()
                        .aria_label(format!("Side stream: {title}"))
                        .child(glyph("corner-down-right", 12., fg3))
                        .child("Side stream")
                        .child(div().min_w_0().truncate().text_color(c(pal.accent)).child(title.clone()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = Some(sid.clone());
                            this.open_thread(sid.clone());
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            msgs = msgs.child(say.test_support());
        }

        let busy = self.busy();
        // ponytail: reads the prefs file on each render so a change in Settings shows; cache it in
        // the shell if renders get frequent.
        let effort = Prefs::load().cli_effort;
        // The reasoning effort (Charles asked what "Auto" was): say so on the chip.
        let effort_label = effort.clone().map_or_else(
            || "Effort: Auto".to_string(),
            |e| {
                let mut c = e.chars();
                c.next().map_or_else(|| "Effort: Auto".into(), |f| format!("Effort: {}{}", f.to_uppercase(), c.as_str()))
            },
        );
        // The Send and Stop glyphs cross-fade when a reply starts or stops. Only the current one
        // takes a click, so the one fading out cannot send or stop anything.
        let k = self.action_since.map_or(1.0, |t0| crate::motion::run(std::time::Instant::now(), t0, cauldron_chrome::motion::time::SWAP));
        let ink = c(Rgb::hex(0x111111));
        let layer = |name: &'static str, size: f32, a: f32| {
            div().absolute().inset_0().flex().items_center().justify_center().opacity(a).child(glyph(name, size, ink))
        };
        let ((now_name, now_size), (was_name, was_size)) = if busy { (("square", 13.), ("arrow-up", 16.)) } else { (("arrow-up", 16.), ("square", 13.)) };
        let mut action = div()
            .id(if busy { "stop" } else { "send" })
            .shadow(super::lift(super::Lift::Button, pal))
            .relative()
            .aria_label(if busy { "Stop" } else { "Send" })
            .child(layer(now_name, now_size, k));
        if k < 1.0 {
            action = action.child(layer(was_name, was_size, 1.0 - k));
        }
        let action = if busy {
            action.on_click(cx.listener(|this, _, _, cx| {
                if let (Some(id), Some((_, chats))) = (this.open.clone(), this.core.as_ref()) {
                    chats.stop(&id);
                }
                this.refresh();
                cx.notify();
            }))
        } else {
            action.on_click(cx.listener(|this, _, window, cx| this.send_composer(window, cx)))
        };
        let action = action
            .ml_auto()
            .size(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::CONTROL))
            .primary_face(pal)
            .pressed()
            .cursor_pointer()
            .test_support();
        let input = self.composer.as_ref().expect("made in render").0.clone();
        let chip_btn = |id: &'static str, label: String| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(5.))
                .px(px(8.))
                .py(px(3.))
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(c(pal.line))
                .bg(c(pal.inset))
                .text_size(px(11.5))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg2)
                .aria_label(label.clone())
                .child(label)
        };
        let effort_chip = chip_btn("effort", effort_label)
            .aria_description("Reasoning effort: Auto lets the CLI choose; a click steps through low, medium, high and max").cursor_pointer().hover(|d| d.text_color(fg)).on_click(cx.listener(|_, _, _, cx| {
            // Cycles Auto, low, medium, high, max and saves it, as Settings would.
            let mut p = Prefs::load();
            let at = EFFORTS.iter().position(|e| *e == p.cli_effort.as_deref()).unwrap_or(0);
            p.cli_effort = EFFORTS[(at + 1) % EFFORTS.len()].map(str::to_string);
            p.save();
            cx.notify();
        }));
        let editor = div()
            .id("composer-input")
            // The multi-line editor pads itself by Size::Medium's 10/8px and ignores style
            // padding; pull it back so the text lines up with the chips under it.
            .mx(px(-10.))
            .when_live(!tiny, |d| d.mt(px(-8.)))
            .when_live(tiny, |d| d.my(px(-8.)).flex_1().min_w_0())
            .text_size(px(13.))
            .child(Textarea::new(&input).appearance(false))
            .test_support();
        let mut composer = div()
            .id("composer")
            .flex_none()
            .flex()
            .rounded(px(radius::SHELL))
            .border_1()
            .border_color(c(pal.line))
            .bg(super::backdrop::surface(self.painting, pal.raised, super::backdrop::GLASS_COMPOSER))
            // The buttons' lift (edge, top light, close 2px lip), not Lift::Box: its 24px long shadow
            // drew a double halo round the box on Windows (Charles, 21 Nov). The transcript fades
            // out above it ("composer-fade"). Charles, 22 Nov: same effect as buttons.
            .shadow(super::lift(super::Lift::Button, pal))
            .aria_label("Message the coordinator");
        composer = if tiny {
            // A very short pane: one row, the editor and Send, no chips.
            composer.items_center().gap(px(6.)).pl(px(12.)).pr(px(6.)).py(px(6.)).child(editor).child(action)
        } else {
            composer.flex_col().gap(px(8.)).px(px(12.)).py(px(10.)).when_some(self.reply_files_el(pal, cx), |d, f| d.child(f)).child(editor).child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    // In a narrow pane the trust and Plan chips give way first, so Send stays in view.
                    .child(
                        div()
                            .id("reply-add")
                            .size(px(24.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(radius::TAG))
                            .cursor_pointer()
                            .hover(|d| d.bg(c(pal.hover)))
                            .aria_label("Add files")
                            .child(glyph("plus", 15., fg2))
                            .on_click(cx.listener(|this, _, window, cx| this.pick_reply_files(window, cx)))
                            .test_support(),
                    )
                    .when_live(!narrow, |d| d.child(self.trust_chip("trust", pal, cx)).child(self.plan_chip("plan", pal, cx)))
                    .child(div().flex_1())
                    // In a narrow pane the route chip gives way first, so Send stays in view.
                    .child(self.route_chip("route", super::subs::PickFor::Thread(t.id.clone()), Some(&t.route), pal, cx))
                    .child(effort_chip.flex_none().test_support())
                    .child(action),
            )
        };
        // The model picker (Charles, 21 Nov: "a top rail on top ... on the chat box"): a tray as
        // wide as the box, rising from its top edge.
        if self.start.route_menu == Some(super::subs::PickFor::Thread(t.id.clone())) {
            let tray = self.model_picker(super::subs::PickFor::Thread(t.id.clone()), Some(&t.route), pal, cx);
            composer = composer.relative().child(
                deferred(
                    div()
                        .id("pick-tray")
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(1.))
                        .pb(px(6.))
                        .child(tray)
                        .test_support()
                        .with_animation("pick-rise", super::motion::anim(200.), |d, t| d.opacity(t).pb(px(6.0 - 6.0 * (1.0 - t)))),
                )
                .with_priority(3),
            );
        }
        // The slash list rises over the box like the model picker (slash.rs).
        if let Some(list) = self.slash_el(pal, cx) {
            composer = composer.relative().child(deferred(div().id("slash-tray").absolute().left_0().right_0().bottom(relative(1.)).pb(px(6.)).child(list).test_support()).with_priority(3));
        }
        // The editor binds these keys to its own actions, so the list takes the actions first.
        {
            use gpui_kit::base::input::{Escape, IndentInline, MoveDown, MoveUp};
            let key = |k: &'static str| move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                if this.slash_key(k, window, cx) {
                    cx.stop_propagation();
                }
            };
            let (up, down, tab, esc) = (key("up"), key("down"), key("tab"), key("escape"));
            composer = composer
                .capture_action(cx.listener(move |this, _: &MoveUp, window, cx| up(this, window, cx)))
                .capture_action(cx.listener(move |this, _: &MoveDown, window, cx| down(this, window, cx)))
                .capture_action(cx.listener(move |this, _: &IndentInline, window, cx| tab(this, window, cx)))
                .capture_action(cx.listener(move |this, _: &Escape, window, cx| esc(this, window, cx)));
        }
        if let Some(e) = &self.send_error {
            composer = composer.child(div().id("send-error").text_size(px(12.)).text_color(c(pal.bad)).aria_label(e.clone()).child(e.clone()).test_support());
        }

        // The stage bar: usage on the left. Tokens per thread are not in core, so cost only.
        let spent = cost(t);
        let usage = div()
            .id("usage")
            .flex()
            .items_center()
            .gap(px(6.))
            .pt(px(8.))
            .px(px(2.))
            .text_size(px(11.))
            .text_color(fg3)
            .font_features(tnum())
            .aria_label(format!("This stream has cost ${spent:.2}"))
            .child("This stream")
            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(fg2).child(format!("${spent:.2}")))
            .test_support();

        // The context bar over the box (the board's `.ctx`): where it runs, the branch, the worktree.
        let ctx_bar = (!short).then(|| {
            let item = |id: &'static str, icon: &'static str, text: String| div().id(id).flex().items_center().gap(px(5.)).aria_label(text.clone()).child(glyph(icon, 13., fg3)).child(text).test_support();
            let mut bar = div().id("ctx-bar").flex().items_center().gap(px(14.)).px(px(12.)).text_size(px(11.5)).text_color(fg3).child(item("ctx-machine", "laptop", "This machine".into()));
            if let Some(b) = t.project.as_deref().and_then(branch) {
                bar = bar.child(item("ctx-branch", "git-branch", b));
            }
            // No Worktree chip: it did nothing (Charles, 21 Nov), and moving an open stream to a new
            // folder would lose the CLI's session, which it finds by folder.
            // A side thread hands its work back to its stream, which judges it as its next turn.
            if t.side_of.is_some() {
                let sid = t.id.clone();
                bar = bar.child(
                    div()
                        .id("send-back")
                        .ml_auto()
                        .h(px(22.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded(px(radius::TAG))
                        .text_color(fg2)
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label("Send the last reply back to the stream")
                        .child(glyph("corner-up-left", 12., fg2))
                        .child("Send back to the stream")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some((_, chats)) = this.core.as_ref() {
                                if let Err(e) = chats.send_back(&sid, this.mode_ctx()) {
                                    this.send_error = Some(e.to_string());
                                }
                            }
                            this.refresh();
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            bar.test_support()
        });
        // Scroll ticks on the right edge (the board's `.mm`): one per response, the one at the
        // top of the view in the accent; a click jumps to it.
        let ticks = (replies.len() > 1).then(|| {
            let cur = current_tick(&replies, self.transcript_scroll.top_item());
            let mut col = div().id("ticks").absolute().right(px(6.)).top(px(16.)).flex().flex_col().items_end().aria_label("Jump to a response");
            for (k, &row) in replies.iter().enumerate() {
                let on = k == cur;
                col = col.child(
                    div()
                        .id(("tick", k))
                        .h(px(7.))
                        .w(px(24.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .cursor_pointer()
                        .aria_label(format!("Response {}", k + 1))
                        .aria_selected(on)
                        .child(div().h(px(2.)).w(px(if on { 20. } else { 14. })).rounded(px(1.)).bg(if on { c(pal.accent) } else { ca(pal.fg4, 0.75) }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.transcript_scroll.scroll_to_item(row);
                            // The jump lands after this frame is built: build every message.
                            *this.transcript_fit.borrow_mut() = None;
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            col.test_support()
        });
        // Room on the right for the ticks, so they never sit over a bubble.
        let msgs = msgs.when_live(ticks.is_some(), |d| d.pr(px(28.))).on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()));
        div()
            .id("thread")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(if short { 6. } else { 10. }))
            .px(px(if narrow { 10. } else { 18. }))
            .py(px(if short { 8. } else { 14. }))
            .child(msgs.test_support())
            .children(ticks)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    // A real fade: the transcript dissolves into the card above the box. Not over a
                    // picture (the Harbour or the user's), where a band of the card's colour would cover it.
                    .when_live(!self.painting && self.stream_bg.is_none(), |d| {
                        let bg = super::backdrop::card_bg(false, pal);
                        d.child(div().id("composer-fade").absolute().left_0().right_0().top(px(-40.)).h(px(40.)).bg(linear_gradient(180., linear_color_stop(bg.opacity(0.), 0.), linear_color_stop(bg, 1.))).test_support())
                    })
                    .children(ctx_bar)
                    .child(composer.test_support()),
            )
            .when_live(!short, |d| d.child(div().flex().items_end().justify_between().gap(px(8.)).child(usage).children(self.usage_badges(pal))))
            .into_any_element()
    }
}

#[cfg(test)]
pub(super) mod tests {

    #[test]
    fn a_cli_failure_reads_as_a_reason_a_code_and_its_output() {
        let f = super::failure("opencode.exe exited with code 1: > build · Qwen/Qwen3.6-35B-A3B-FP8\n\nError: Not Found: route not found");
        assert_eq!(f.reason, "The model's provider answered 404, route not found");
        assert_eq!(f.code.as_deref(), Some("exit 1"));
        assert!(f.hint.is_some());
        assert_eq!(f.output, vec!["> build · Qwen/Qwen3.6-35B-A3B-FP8", "Error: Not Found: route not found"]);
        let g = super::failure("claude is not installed");
        assert_eq!(g.reason, "claude is not installed");
        assert!(g.output.is_empty(), "nothing more to show than the reason");
    }
    use super::super::{Cauldron, Threads};
    use super::lines;
    use backspace_core::chat::{Chats, Status};
    use backspace_core::prefs::Prefs;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{px, size, AppContext, TestAppContext};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn msg(id: &str, role: &str, text: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut m = serde_json::json!({ "id": id, "role": role, "text": text, "at": 1, "status": "done" });
        m.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        m
    }

    /// A data folder with two threads: t1 answered once, t2 empty. Neither route can reach a
    /// model: the CLI they name is not installed, so a send fails fast and touches no network.
    fn data() -> PathBuf {
        let d = std::env::temp_dir().join(format!("cauldron-ui4-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("threads")).unwrap();
        let route = serde_json::json!({ "kind": "cli", "provider": "cauldron-test-missing-cli", "model": null });
        let t1 = serde_json::json!({
            "id": "t1", "title": "Get M3 through review", "created": 1, "updated": 3, "route": route,
            "project": "C:\\code\\Cauldron",
            "messages": [
                msg("m1", "user", "Run the tests", serde_json::json!({})),
                msg("m2", "assistant", "All 591 pass.", serde_json::json!({ "model": "opus", "cost_usd": 0.42, "thinking": "Check the suite.", "trace": "00ff00ff" })),
            ]
        });
        let t2 = serde_json::json!({ "id": "t2", "title": "Empty one", "created": 1, "updated": 2, "route": route, "messages": [] });
        std::fs::create_dir_all(d.join("traces")).unwrap();
        let tool = |name: &str, input: &str| serde_json::json!({ "span_id": "s", "name": name, "kind": "tool", "start": 1, "input": input });
        let trace = serde_json::json!({ "trace_id": "00ff00ff", "thread": "t1", "message": "m2", "spans": [
            { "span_id": "r", "name": "reply", "kind": "reply", "start": 1 },
            tool("Read", r#"{"file_path":"a.rs"}"#), tool("Bash", r#"{"command":"cargo test"}"#),
        ]});
        std::fs::write(d.join("traces").join("00ff00ff.json"), trace.to_string()).unwrap();
        for t in [t1, t2] {
            std::fs::write(d.join("threads").join(format!("{}.json", t["id"].as_str().unwrap())), t.to_string()).unwrap();
        }
        d
    }

    /// ⌘N on macOS, Ctrl+N elsewhere, starts a New stream from anywhere in the window: from an
    /// open stream with the composer focused, and with nothing focused at all.
    #[gpui_kit::test]
    fn the_new_stream_shortcut_works_from_anywhere(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update(super::super::bind_keys);
        let key = if cfg!(target_os = "macos") { "cmd-n" } else { "ctrl-n" };
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            window.click("composer-input", cx);
            assert!(window.try_find("nt-box").is_none(), "a stream is open");
            window.press(key, cx);
            window.render_frame(cx);
            assert!(window.try_find("nt-box").is_some(), "the shortcut from the composer opens New stream");
            window.click(("thread", 0usize), cx);
            window.blur(cx);
            window.press(key, cx);
            window.render_frame(cx);
            assert!(window.try_find("nt-box").is_some(), "and with nothing focused");
        })
        .unwrap();
    }

    /// The current tick is the last response at or above the top row in view.
    #[test]
    fn the_current_tick_follows_the_top_row() {
        let replies = [1, 3, 5];
        assert_eq!(super::current_tick(&replies, 0), 0, "above the first response: the first");
        assert_eq!(super::current_tick(&replies, 3), 1);
        assert_eq!(super::current_tick(&replies, 4), 1);
        assert_eq!(super::current_tick(&replies, 9), 2);
    }

    /// The stream view (Tight tiles): the context bar over the composer names where it runs, the
    /// composer has the trust chip and the Plan toggle, and a send in Plan mode reaches core as a
    /// plan. One response draws no ticks. Without a git folder there is no branch.
    #[gpui_kit::test]
    fn the_stream_view_has_the_boards_composer_and_sends_plans(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find("ctx-machine").label(), Some("This machine"));
            assert!(window.try_find("ctx-branch").is_none(), "no .git in the test folder, no branch");
            let (bar, comp) = (super::super::tests::rect(window, "ctx-bar"), super::super::tests::rect(window, "composer"));
            assert!(bar.bottom() <= comp.y, "the context bar sits over the box");
            assert!(window.try_find("ticks").is_none(), "one response: no ticks");
            assert_eq!(window.find("plan").label(), Some("Plan"));
            window.click("plan", cx);
            window.render_frame(cx);
            assert_eq!(window.find("plan").selected(), Some(true), "Ask · Plan · Act: Plan picked");
            assert_eq!(window.find("plan-act").selected(), Some(false));
            window.click(("thread", 1usize), cx);
            window.click("composer-input", cx);
            window.input("What would you change?", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let t = chats.peek("t2").unwrap();
        assert_eq!(t.messages[0].text, "What would you change?");
        assert!(t.messages[0].plan, "sent as a plan");
    }

    /// One temp prefs file for the whole test process, set once and never unset: env vars are
    /// process-wide and tests run in parallel, so no test can render or save against the user's
    /// real prefs.
    fn prefs() -> &'static PathBuf {
        static P: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        P.get_or_init(|| {
            let p = std::env::temp_dir().join(format!("cauldron-ui4-prefs-{}.json", std::process::id()));
            let _ = std::fs::remove_file(&p);
            std::env::set_var("BACKSPACE_PREFS", &p);
            p
        })
    }

    /// Held by tests that change a saved preference another test reads (prefs live in one file
    /// for the whole test binary), so a reader never sees a value mid-cycle.
    pub(in crate::shell) fn prefs_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
        GUARD.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(in crate::shell) fn open(cx: &mut TestAppContext) -> (gpui_kit::WindowHandle<Cauldron>, Arc<Chats>) {
        cx.update(gpui_kit::init);
        prefs();
        let d = data();
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        // No change channel: a wake from core's tokio thread into the test scheduler makes the run
        // non-deterministic (and panics). Tests call `refresh` where the app would be poked.
        let (tx, _) = async_channel::bounded(1);
        let chats = Chats::open(d, rt.handle().clone(), tx);
        let ch = chats.clone();
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(move |_, _| Cauldron { threads: Threads::Ready(ch.list()), core: Some((rt, ch)), accounts_file: Some(super::super::tests::no_accounts()), ..Cauldron::new() });
        cx.simulate_window_resize(handle.into(), size(px(1240.), px(702.)));
        (handle, chats)
    }

    fn span(name: &str, input: &str, error: bool) -> backspace_core::trace::Span {
        backspace_core::trace::Span { name: name.into(), kind: "tool".into(), input: input.into(), error: error.then(|| "failed".into()), ..Default::default() }
    }

    /// A reply's tool calls read as a few plain lines, in the order they first happened: files
    /// counted once, failures said, unknown tools named.
    #[test]
    fn steps_group_a_replys_tool_calls() {
        let root = backspace_core::trace::Span { name: "reply".into(), kind: "reply".into(), ..Default::default() };
        let t = backspace_core::trace::Trace {
            spans: vec![
                root,
                span("Read", r#"{"file_path":"a.rs"}"#, false),
                span("Bash", r#"{"command":"cargo test"}"#, false),
                span("Read", r#"{"file_path":"b.rs"}"#, false),
                span("Read", r#"{"file_path":"a.rs"}"#, false),
                span("Edit", r#"{"file_path":"a.rs"}"#, false),
                span("Write", r#"{"file_path":"c.rs"}"#, false),
                span("Edit", r#"{"file_path":"a.rs"}"#, false),
                span("Bash", r#"{"command":"cargo build"}"#, true),
                span("WebSearch", r#"{"query":"gpui"}"#, false),
                span("Grep", r#"{"pattern":"x"}"#, false),
                span("mcp__linear__get_issue", "{}", false),
            ],
            ..Default::default()
        };
        assert_eq!(super::steps(&t, true), ["Thought", "Read 2 files", "Ran 2 commands, 1 failed", "Made 3 edits", "Searched the web", "Searched the code", "Used linear: get_issue"]);
        let one = backspace_core::trace::Trace { spans: vec![t.spans[0].clone(), span("Read", r#"{"file_path":"a.rs"}"#, false), span("Bash", "{}", false)], ..Default::default() };
        assert_eq!(super::steps(&one, false), ["Read 1 file", "Ran 1 command"]);
        assert!(super::steps(&backspace_core::trace::Trace::default(), false).is_empty());
    }

    #[test]
    fn lines_carry_who_answered_and_the_cost() {
        let t: backspace_core::chat::Thread = serde_json::from_value(serde_json::json!({
            "id": "x", "title": "x", "created": 1, "updated": 1,
            "route": { "kind": "cli", "provider": "claude", "model": null },
            "messages": [
                msg("a", "user", "hi", serde_json::json!({})),
                msg("b", "assistant", "", serde_json::json!({ "status": "error", "error": "usage limit", "author_name": "Builder", "model": "opus", "cost_usd": 0.4 })),
            ]
        }))
        .unwrap();
        let l = lines(&t);
        assert!(l[0].mine && l[0].meta.is_none());
        assert_eq!(l[1].status, Status::Error);
        assert_eq!(l[1].meta.as_deref(), Some("Builder · $0.40"));
        assert_eq!((l[0].model.as_deref(), l[1].model.as_deref()), (None, Some("opus")));
        assert_eq!(super::cost(&t), 0.4);
        let empty = backspace_core::chat::Thread { messages: Vec::new(), ..t };
        assert_eq!(format!("{:.2}", super::cost(&empty)), "0.00", "no minus zero");
    }

    /// At the smallest pane (200 wide, 120 tall under its 32px header) the thread still reads: its
    /// parts stay inside the pane, the transcript ends above the composer, Send stays in view, and
    /// the cost line and the composer's chips make way when there is no room for them.
    #[gpui_kit::test]
    fn the_thread_fits_the_smallest_pane(cx: &mut TestAppContext) {
        use cauldron_chrome::panes::{Line, Shape};
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
        })
        .unwrap();
        let rect = |w: &gpui_kit::Window, id: (&'static str, usize)| super::super::tests::rect(w, id);
        let rect1 = |w: &gpui_kit::Window, id: &'static str| super::super::tests::rect(w, id);
        let inside = |p: cauldron_chrome::layout::Rect, r: cauldron_chrome::layout::Rect| r.x >= p.x - 0.5 && r.right() <= p.right() + 0.5 && r.y >= p.y - 0.5 && r.bottom() <= p.bottom() + 0.5;
        for (shape, line, to_x) in [(Shape::Cols, Line::V, true), (Shape::Rows, Line::H, false)] {
            handle.update(cx, |c, _, _| {
                c.panes.shape = shape;
                c.panes.drag(line, cauldron_chrome::layout::Rect::new(0.0, 0.0, 10_000.0, 10_000.0), 0.0);
            })
            .unwrap();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.render_frame(cx);
                let pane = rect(window, ("pane", 0usize));
                if to_x {
                    assert_eq!(pane.w, 200.0, "{shape:?}: the thread pane at its narrowest");
                } else {
                    assert_eq!(pane.h, 120.0, "{shape:?}: the thread pane at its shortest");
                }
                let composer = rect1(window, "composer");
                assert!(inside(pane, composer), "{shape:?}: composer {composer:?} inside {pane:?}");
                assert!(inside(pane, rect1(window, "send")), "{shape:?}: Send in view");
                let tr = rect1(window, "transcript");
                assert!(tr.bottom() <= composer.y + 0.5, "{shape:?}: the transcript ends above the composer");
                for i in 0..4usize {
                    if let Some(m) = window.try_find(("msg", i)) {
                        let b = m.bounds();
                        assert!(f32::from(b.origin.x + b.size.width) <= pane.right() + 0.5, "{shape:?}: message {i} wraps inside the pane");
                    }
                }
                if !to_x {
                    assert!(window.try_find("usage").is_none(), "no room for the cost line");
                }
            })
            .unwrap();
        }
    }

    /// Clicking a row opens its thread: the title on the pinned tab and above the transcript,
    /// each message, who answered at what cost, and the composer. An empty thread says so.
    #[gpui_kit::test]
    fn a_row_opens_its_thread(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("nt-box").is_some(), "no thread open: the New thread screen");
            window.click(("thread", 0usize), cx);
            // Tight tiles: the stream's title is its tab, in the stream card's header.
            assert_eq!(window.find(("tab", 0usize)).label(), Some("Get M3 through review"));
            let (tab, head) = (super::super::tests::rect(window, ("tab", 0usize)), super::super::tests::rect(window, "card-head"));
            assert!(tab.x >= head.x && tab.right() <= head.right() && tab.y >= head.y && tab.bottom() <= head.bottom(), "the title tab sits in the card header");
            // The stream's status mark sits inside its tab, left of the title (Charles, 21 Nov).
            let mk = super::super::tests::rect(window, ("tab-mark", 0usize));
            assert!(mk.x >= tab.x && mk.right() <= tab.right() && mk.y >= tab.y && mk.bottom() <= tab.bottom(), "the mark is inside the tab");
            assert_eq!(window.find(("msg", 0usize)).label(), Some("Run the tests"));
            assert_eq!(window.find(("msg", 1usize)).label(), Some("All 591 pass."));
            assert_eq!(window.find(("steps", 1usize)).label(), Some("Thought, Read 1 file, Ran 1 command"), "how the reply was made, from its trace");
            assert!(window.find(("msg", 0usize)).bounds().origin.x > window.find(("msg", 1usize)).bounds().origin.x, "your message sits right");
            assert_eq!(window.find("usage").label(), Some("This stream has cost $0.42"));
            assert_eq!(window.find("composer").label(), Some("Message the coordinator"));
            assert_eq!(window.find("send").label(), Some("Send"));
            window.click(("thread", 1usize), cx);
            assert_eq!(window.find("thread-empty").label(), Some("No messages yet"));
        })
        .unwrap();
    }

    /// A reply still on its way shows a dot and what it is doing, in the step lines' grey with its
    /// label lined up on the step text, under any text so far; a finished reply has none. No agent
    /// bust: figures belong to the Agents page (Charles, 13 Nov).
    #[gpui_kit::test]
    fn a_running_reply_shows_a_status_line(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
        })
        .unwrap();
        for (status, label, text) in [(Status::Streaming, "Writing", ""), (Status::Sending, "Waiting for the reply", ""), (Status::Streaming, "Writing", "Half a sen")] {
            handle
                .update(cx, |c, _, _| {
                    let t = c.thread.as_mut().unwrap();
                    t.messages.truncate(2);
                    let mut m = t.messages[1].clone();
                    m.id = "m3".into();
                    m.text = text.into();
                    m.status = status;
                    t.messages.push(m);
                })
                .unwrap();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(window.find(("live", 2usize)).label(), Some(label));
                assert!(window.try_find(("bust", 2usize)).is_none(), "no agent bust");
                let slot = window.find(("live-slot", 2usize)).bounds();
                assert_eq!((slot.size.width, slot.size.height), (px(16.), px(16.)), "a 16px square slot");
                let tx = window.find(("live-text", 2usize)).bounds().origin.x;
                let step = window.find("step-text-1-0").bounds().origin.x;
                assert_eq!(tx, step, "the label lines up with the step text");
                assert!(window.try_find(("live", 1usize)).is_none(), "a finished reply has no status line");
                if !text.is_empty() {
                    assert_eq!(window.find(("msg", 2usize)).label(), Some(text), "the text shows above the status line while it streams");
                }
            })
            .unwrap();
        }
    }

    /// Typing and Enter send to core; the message appears at once, and the reply's failure
    /// (the CLI is not installed) shows inline with Retry.
    #[gpui_kit::test]
    fn the_composer_sends_through_core(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 1usize), cx);
            window.click("composer-input", cx);
            window.input("Ship", cx);
            window.press("shift-enter", cx);
            window.input("it", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(chats.peek("t2").unwrap().messages.is_empty(), "Shift+Enter adds a line, it does not send");
        cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx)).unwrap();
        cx.run_until_parked();
        assert_eq!(chats.peek("t2").unwrap().messages[0].text, "Ship
it");
        let failed = |cx: &mut TestAppContext| {
            for _ in 0..200 {
                let t = chats.peek("t2").unwrap();
                if t.messages.last().is_some_and(|m| m.status == Status::Error) && !chats.is_busy("t2") {
                    handle.update(cx, |c, _, _| c.refresh()).unwrap();
                    return t.messages.last().unwrap().id.clone();
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                cx.run_until_parked();
            }
            panic!("the reply never failed");
        };
        let first = failed(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("msg", 0usize)).label(), Some("Ship
it"));
            let err = window.find(("msg-error", 1usize)).label().unwrap_or_default().to_string();
            assert!(err.contains("not installed"), "{err}");
            window.click(("retry", 1usize), cx);
        })
        .unwrap();
        let second = failed(cx);
        assert_ne!(first, second, "Retry replaced the failed reply with a new attempt");
        assert_eq!(chats.peek("t2").unwrap().messages.len(), 2);
    }

    /// The effort chip cycles Auto, low, ... and saves it to prefs (the temp file in tests).
    #[gpui_kit::test]
    fn the_effort_chip_cycles_and_saves(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            assert_eq!(window.find("effort").label(), Some("Effort: Auto"));
            window.click("effort", cx);
            window.render_frame(cx);
            assert_eq!(window.find("effort").label(), Some("Effort: Low"));
        })
        .unwrap();
        assert_eq!(Prefs::load().cli_effort.as_deref(), Some("low"));
        let saved = std::fs::read_to_string(prefs()).unwrap();
        assert!(saved.contains("low"), "{saved}");
    }
}
