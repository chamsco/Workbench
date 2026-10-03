//! Backspace workbench, GPUI edition: the same design the Tauri shell
//! renders, drawn natively.
//!
//! Tabs on the left of the title bar each hold up to four canvases (2x2 at
//! four): an agent session, a worktree, a browser, the diagram review or
//! PLAN.md; a new canvas starts as a picker. Tickets open in a drawer off the
//! review pill; machines (this one plus followed remotes) switch from the
//! sidebar foot; settings sit behind the gear. Tabs, theme and machines
//! persist in the prefs file the Tauri build shares.
//!
//!   backspace [workspace]     (defaults to the current directory)
//!
//! GPUI has no web engine, so a browser canvas shows a text snapshot of the
//! page and opens the real thing in the system browser.

mod canvas;
mod icons;
mod pal;
mod stage;
mod ui;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use backspace_core::files::FileEntry;
use backspace_core::fleet::{Backend, Fleet, MachineInfo};
use backspace_core::prefs::{PaneSpec, Prefs, TabSpec};
use backspace_core::remote::Link;
use backspace_core::update::UpdateInfo;
use backspace_core::{
    AgentKind, AgentRecord, AgentStatus, Harness, ProjectState, Ticket, TicketState, MAIN,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use pal::Pal;
use stage::Cam;
use ui::*;

// ------------------------------------------------------------------ model

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideView {
    Projects,
    Agents,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Viewport {
    Desktop,
    Tablet,
    Phone,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Drag {
    Side,
    /// Column gutter 0 or 1 (three canvases have two).
    Col(usize),
    Rows,
}

pub(crate) type FitKey = (usize, usize, f32, f32, f32);

/// What a canvas slot remembers while its tab is showing.
pub(crate) struct PaneState {
    pub(crate) scroll: ScrollHandle,
    pub(crate) seen: usize,
    pub(crate) filter: Entity<InputState>,
    pub(crate) url: Entity<InputState>,
    pub(crate) feedback: Entity<InputState>,
    pub(crate) preview: Option<(String, String)>,
    pub(crate) viewport: Viewport,
    pub(crate) cam: Option<Cam>,
    /// Inputs of the last automatic fit; a change refits until the user pans.
    pub(crate) fitted: Option<FitKey>,
    pub(crate) user_cam: bool,
    pub(crate) fb_err: bool,
    pub(crate) bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(crate) stage_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(crate) card_bounds: Rc<Cell<Bounds<Pixels>>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Pop {
    NewTab(usize),
    TabMenu(usize),
    AddCanvas,
}

enum Upd {
    Idle,
    Checking,
    Done(Result<UpdateInfo, String>),
}

pub(crate) struct Workbench {
    fleet: Arc<Fleet>,
    pub(crate) snap: ProjectState,
    machines: Vec<MachineInfo>,
    pub(crate) pal: Pal,
    prefs: Prefs,
    bench: bool,

    pub(crate) focus: usize,
    pub(crate) max: Option<usize>,
    side: SideView,
    side_open: bool,
    side_w: f32,
    pub(crate) drag: Option<Drag>,
    pub(crate) grid_bounds: Rc<Cell<Bounds<Pixels>>>,
    win_bounds: Rc<Cell<Bounds<Pixels>>>,

    pub(crate) composer: Entity<InputState>,
    pub(crate) panes: Vec<PaneState>,
    pub(crate) pages: HashMap<String, Result<ui::Page, String>>,
    trees: HashMap<(usize, usize), (Instant, Vec<FileEntry>)>,
    loading: HashSet<(usize, usize)>,
    plan: Option<(usize, Instant, Option<String>)>,
    pub(crate) review: Option<usize>,
    pub(crate) pan: Option<(usize, Point<Pixels>)>,
    last_pending: usize,

    pop: Option<(Pop, Point<Pixels>)>,
    new_name: Entity<InputState>,
    renaming: Option<usize>,
    rename: Entity<InputState>,

    drawer: bool,
    pinned: bool,
    hov_cta: bool,
    hov_drawer: bool,
    drawer_task: Option<Task<()>>,
    ticket_sel: Option<String>,
    ticket_input: Entity<InputState>,
    ticket_err: Option<String>,

    settings: bool,
    m_name: Entity<InputState>,
    m_url: Entity<InputState>,
    m_tok: Entity<InputState>,
    m_err: Option<String>,
    m_busy: bool,
    share_addr: Entity<InputState>,
    upd: Upd,

    desk: Option<((u32, u32, Hsla), Arc<Image>)>,
    pub(crate) spin: usize,
    pub(crate) started: Instant,
    focus_handle: FocusHandle,
    _subs: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

fn on_enter(
    input: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<Workbench>,
    f: impl Fn(&mut Workbench, String, &mut Window, &mut Context<Workbench>) -> bool + 'static,
) -> Subscription {
    cx.subscribe_in(
        input,
        window,
        move |this, input, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = input.read(cx).value().trim().to_string();
                if f(this, text, window, cx) {
                    input.update(cx, |s, cx| s.set_value("", window, cx));
                }
                cx.notify();
            }
        },
    )
}

impl Workbench {
    fn new(fleet: Arc<Fleet>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |p: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(p))
        };
        let composer = input("Describe the goal, or answer the main agent", window, cx);
        let new_name = input("Name (optional)", window, cx);
        let rename = input("Tab name", window, cx);
        let ticket_input = input("File a ticket: title: what is wrong or wanted", window, cx);
        let m_name = input("Name, e.g. Cloud", window, cx);
        let m_url = input("http://10.0.0.5:7420", window, cx);
        let m_tok = input("Token", window, cx);
        let share_addr = input("127.0.0.1:7420", window, cx);

        let mut subs = vec![
            on_enter(&composer, window, cx, |this, text, _, _| {
                if text.is_empty() {
                    return false;
                }
                this.backend().send(text);
                true
            }),
            on_enter(&new_name, window, cx, |this, text, window, cx| {
                if let Some((Pop::NewTab(n), _)) = this.pop {
                    this.create_tab(text, n, window, cx);
                }
                true
            }),
            on_enter(&rename, window, cx, |this, text, _, cx| {
                this.commit_rename(Some(text), cx);
                false
            }),
            on_enter(&ticket_input, window, cx, |this, text, _, cx| {
                this.file_ticket(text, cx);
                true
            }),
            on_enter(&m_tok, window, cx, |this, _, _, cx| {
                this.add_machine(cx);
                false
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.apply_theme(window, cx);
            }),
        ];

        let mut panes = vec![];
        for slot in 0..4 {
            let url = input("http://localhost:5173", window, cx);
            let filter = input("Search", window, cx);
            let feedback = input("Feedback for the agent (needed to reject)", window, cx);
            subs.push(on_enter(&url, window, cx, move |this, text, _, cx| {
                if text.is_empty() {
                    return false;
                }
                let u = if text.starts_with("http://") || text.starts_with("https://") {
                    text
                } else {
                    format!("http://{text}")
                };
                if let Some(p) = this.tab_mut().panes.get_mut(slot) {
                    p.url = Some(u.clone());
                }
                this.save_tabs();
                this.fetch(u, cx);
                false
            }));
            subs.push(cx.observe(&filter, |_, _, cx| cx.notify()));
            subs.push(cx.observe(&feedback, move |this, _, cx| {
                this.panes[slot].fb_err = false;
                cx.notify()
            }));
            panes.push(PaneState {
                scroll: ScrollHandle::new(),
                seen: usize::MAX,
                filter,
                url,
                feedback,
                preview: None,
                viewport: Viewport::Desktop,
                cam: None,
                fitted: None,
                user_cam: false,
                fb_err: false,
                bounds: Rc::default(),
                stage_bounds: Rc::default(),
                card_bounds: Rc::default(),
            });
        }

        let changes = fleet.changes();
        let refresh = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while changes.recv().await.is_ok() {
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
            }
        });
        // Spinner and pulsing dots, only while something is working.
        let tick = cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut AsyncApp| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(130))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    let busy = this.snap.agents.iter().any(|a| {
                        matches!(
                            a.status,
                            AgentStatus::Running | AgentStatus::AwaitingApproval
                        )
                    });
                    if busy {
                        this.spin = (this.spin + 1) % SPIN.len();
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            },
        );

        let mut prefs = fleet.prefs();
        let bench = std::env::var_os("BACKSPACE_READY_FILE").is_some();
        // bench/: a fixed first tab with that many canvases.
        if let Some(n) = std::env::var("BACKSPACE_LAYOUT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| (1..=4).contains(n))
        {
            let panes = vec![
                PaneSpec::agent(MAIN),
                PaneSpec::files(MAIN),
                PaneSpec::agent(1),
                PaneSpec::agent(2),
            ];
            prefs
                .tabs
                .insert(0, TabSpec::new(Some("Bench".into()), panes[..n].to_vec()));
            prefs.active_tab = 0;
        }
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let mut this = Self {
            snap: fleet.snapshot(),
            machines: fleet.machines(),
            fleet,
            pal: Pal::light(),
            prefs,
            bench,
            focus: 0,
            max: None,
            side: SideView::Projects,
            side_open: true,
            side_w: 264.,
            drag: None,
            grid_bounds: Rc::default(),
            win_bounds: Rc::default(),
            composer,
            panes,
            pages: HashMap::new(),
            trees: HashMap::new(),
            loading: HashSet::new(),
            plan: None,
            review: None,
            pan: None,
            last_pending: 0,
            pop: None,
            new_name,
            renaming: None,
            rename,
            drawer: false,
            pinned: false,
            hov_cta: false,
            hov_drawer: false,
            drawer_task: None,
            ticket_sel: None,
            ticket_input,
            ticket_err: None,
            settings: false,
            m_name,
            m_url,
            m_tok,
            m_err: None,
            m_busy: false,
            share_addr,
            upd: Upd::Idle,
            desk: None,
            spin: 0,
            started: Instant::now(),
            focus_handle,
            _subs: subs,
            _tasks: vec![refresh, tick],
        };
        this.apply_theme(window, cx);
        this.load_pane_inputs(window, cx);
        if this.prefs.check_updates && !this.bench {
            this.check_update(cx);
        }
        // bench/ times launch to the first frame with data on screen.
        if let Ok(path) = std::env::var("BACKSPACE_READY_FILE") {
            cx.on_next_frame(window, move |_, _, _| {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis());
                let _ = std::fs::write(&path, ms.to_string());
            });
        }
        this
    }

    // -------------------------------------------------------------- data

    pub(crate) fn backend(&self) -> Arc<dyn Backend> {
        self.fleet.backend()
    }

    fn machine(&self) -> MachineInfo {
        self.machines
            .iter()
            .find(|m| m.selected)
            .cloned()
            .unwrap_or_else(|| self.machines[0].clone())
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.snap = self.fleet.snapshot();
        self.machines = self.fleet.machines();
        // A new review is queued up front, the way a notification would be.
        let pending: Vec<usize> = self.snap.pending_approvals().map(|a| a.id).collect();
        if pending.len() > self.last_pending {
            self.review = pending.last().copied();
            for p in &mut self.panes {
                p.cam = None;
                p.user_cam = false;
            }
        }
        self.last_pending = pending.len();
        cx.notify();
    }

    pub(crate) fn pending(&self) -> usize {
        self.snap.pending_approvals().count()
    }

    /// Where the current machine can't show its project yet.
    pub(crate) fn offline_note(&self) -> Option<AnyElement> {
        let m = self.machine();
        let p = self.pal;
        if m.local || (m.link == Link::Online && !self.snap.agents.is_empty()) {
            return None;
        }
        let (head, body) = match &m.link {
            Link::Offline { error } => (
                format!("{} is offline.", m.name),
                format!("{error}. Retrying every few seconds."),
            ),
            _ => (format!("Connecting to {}…", m.name), String::new()),
        };
        Some(
            div()
                .m(px(8.))
                .p(px(10.))
                .rounded(px(9.))
                .bg(p.pane)
                .border_1()
                .border_color(p.pane_edge)
                .text_size(px(12.))
                .text_color(p.fg2)
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.fg)
                        .child(head),
                )
                .when(!body.is_empty(), |d| d.child(body))
                .into_any_element(),
        )
    }

    /// A worktree listing; fetched off the UI thread (remote machines answer
    /// over the network) and kept for two seconds.
    pub(crate) fn tree(&mut self, id: usize, cx: &mut Context<Self>) -> Vec<FileEntry> {
        let key = (self.fleet.current(), id);
        let stale = self
            .trees
            .get(&key)
            .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(2));
        if stale && self.loading.insert(key) {
            let b = self.backend();
            let task = cx.background_spawn(async move { b.list_files(id) });
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let rows = task.await;
                let _ = this.update(cx, |this, cx| {
                    this.loading.remove(&key);
                    this.trees.insert(key, (Instant::now(), rows));
                    cx.notify();
                });
            })
            .detach();
        }
        self.trees
            .get(&key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    pub(crate) fn read_preview(&mut self, slot: usize, path: String, cx: &mut Context<Self>) {
        let b = self.backend();
        let p = path.clone();
        let task = cx.background_spawn(async move { b.read_file(&p) });
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let text = task.await.unwrap_or_else(|e| e.to_string());
            let _ = this.update(cx, |this, cx| {
                this.panes[slot].preview = Some((path, text));
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn plan_text(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let m = self.fleet.current();
        let stale = self
            .plan
            .as_ref()
            .is_none_or(|(pm, at, _)| *pm != m || at.elapsed() > Duration::from_millis(1500));
        if stale && !self.snap.workspace.as_os_str().is_empty() {
            // Mark it fresh now so one read is in flight at a time.
            let old = self
                .plan
                .take()
                .filter(|(pm, _, _)| *pm == m)
                .and_then(|p| p.2);
            self.plan = Some((m, Instant::now(), old));
            let b = self.backend();
            let path = self
                .snap
                .workspace
                .join("PLAN.md")
                .to_string_lossy()
                .into_owned();
            let task = cx.background_spawn(async move { b.read_file(&path).ok() });
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let text = task.await;
                let _ = this.update(cx, |this, cx| {
                    if this.plan.as_ref().and_then(|p| p.2.as_ref()) != text.as_ref() {
                        this.plan = Some((m, Instant::now(), text));
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        self.plan.as_ref().and_then(|(_, _, t)| t.clone())
    }

    pub(crate) fn fetch(&mut self, url: String, cx: &mut Context<Self>) {
        self.pages.remove(&url);
        let task = cx.background_spawn({
            let url = url.clone();
            async move { probe(&url) }
        });
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let page = task.await;
            let _ = this.update(cx, |this, cx| {
                this.pages.insert(url, page);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn file_ticket(&mut self, text: String, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }
        let (title, body) = text.split_once(':').unwrap_or((&text, &text));
        let (title, body) = (title.trim().to_string(), body.trim().to_string());
        let b = self.backend();
        let task = cx.background_spawn(async move { b.file_ticket(&title, &body) });
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let res = task.await;
            let _ = this.update(cx, |this, cx| {
                this.ticket_err = res.err().map(|e| e.to_string());
                cx.notify();
            });
        })
        .detach();
    }

    fn check_update(&mut self, cx: &mut Context<Self>) {
        self.upd = Upd::Checking;
        let fleet = self.fleet.clone();
        let task = cx.background_spawn(async move { fleet.check_update() });
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let res = task.await.map_err(|e| e.to_string());
            let _ = this.update(cx, |this, cx| {
                this.upd = Upd::Done(res);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn zoom_center(&mut self, slot: usize, f: f32, cx: &mut Context<Self>) {
        let ps = &mut self.panes[slot];
        let b = ps.stage_bounds.get();
        let (sw, sh) = (f32::from(b.size.width), f32::from(b.size.height));
        if let Some(cam) = ps.cam.as_mut() {
            cam.zoom(f, sw / 2., sh / 2., sw);
            ps.user_cam = true;
            cx.notify();
        }
    }

    // -------------------------------------------------------------- theme

    /// Pick the palette, and keep gpui-kit's inputs in the same colours.
    fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = match self.prefs.theme.as_str() {
            "light" => false,
            "dark" => true,
            _ => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        self.pal = if dark { Pal::dark() } else { Pal::light() };
        Theme::change(
            if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        let p = self.pal;
        let t = Theme::global_mut(cx);
        t.foreground = p.fg;
        t.muted_foreground = p.fg4;
        t.caret = p.accent;
        t.selection = p.blue.opacity(0.25);
        t.font_family = UI.into();
        cx.notify();
    }

    fn set_theme(&mut self, theme: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.prefs.theme = theme.into();
        let t = theme.to_string();
        self.fleet.update_prefs(|p| p.theme = t);
        self.apply_theme(window, cx);
    }

    // -------------------------------------------------------------- tabs

    pub(crate) fn tab(&self) -> &TabSpec {
        &self.prefs.tabs[self.prefs.active_tab]
    }

    pub(crate) fn tab_mut(&mut self) -> &mut TabSpec {
        let i = self.prefs.active_tab;
        &mut self.prefs.tabs[i]
    }

    pub(crate) fn save_tabs(&self) {
        // Bench runs keep their scripted tab out of the user's prefs.
        if std::env::var_os("BACKSPACE_LAYOUT").is_some() {
            return;
        }
        let (tabs, active) = (self.prefs.tabs.clone(), self.prefs.active_tab);
        self.fleet.update_prefs(|p| {
            p.tabs = tabs;
            p.active_tab = active;
        });
    }

    /// Per-slot state follows the tab: reset it and load browser addresses.
    fn load_pane_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let urls: Vec<Option<String>> = self.tab().panes.iter().map(|p| p.url.clone()).collect();
        for (slot, ps) in self.panes.iter_mut().enumerate() {
            ps.preview = None;
            ps.cam = None;
            ps.fitted = None;
            ps.user_cam = false;
            ps.seen = usize::MAX;
            let u = urls.get(slot).cloned().flatten().unwrap_or_default();
            ps.url.update(cx, |s, cx| s.set_value(u, window, cx));
        }
        for u in urls.into_iter().flatten() {
            if !self.pages.contains_key(&u) {
                self.fetch(u, cx);
            }
        }
    }

    fn switch_tab(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        if i >= self.prefs.tabs.len() {
            return;
        }
        self.settings = false;
        if i != self.prefs.active_tab {
            self.prefs.active_tab = i;
            self.focus = 0;
            self.max = None;
            self.load_pane_inputs(window, cx);
            self.save_tabs();
        }
        cx.notify();
    }

    fn create_tab(&mut self, name: String, n: usize, window: &mut Window, cx: &mut Context<Self>) {
        let name = Some(name.trim().to_string()).filter(|s| !s.is_empty());
        self.prefs
            .tabs
            .push(TabSpec::new(name, vec![PaneSpec::default(); n]));
        self.pop = None;
        let last = self.prefs.tabs.len() - 1;
        self.switch_tab(last, window, cx);
    }

    fn close_tab(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.prefs.tabs.len() < 2 {
            return;
        }
        self.prefs.tabs.remove(i);
        let active = self.prefs.active_tab;
        let next = if active >= i {
            active.saturating_sub(1)
        } else {
            active
        };
        self.prefs.active_tab = next.min(self.prefs.tabs.len() - 1);
        self.focus = 0;
        self.max = None;
        self.load_pane_inputs(window, cx);
        self.save_tabs();
        cx.notify();
    }

    fn start_rename(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pop = None;
        self.renaming = Some(i);
        let name = self.prefs.tabs[i].name.clone().unwrap_or_default();
        self.rename.update(cx, |s, cx| {
            s.set_value(name, window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }

    fn commit_rename(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        if let Some(i) = self.renaming.take() {
            if let (Some(name), Some(t)) = (name, self.prefs.tabs.get_mut(i)) {
                t.name = Some(name.trim().to_string()).filter(|s| !s.is_empty());
                self.save_tabs();
            }
        }
        cx.notify();
    }

    pub(crate) fn set_pane(
        &mut self,
        slot: usize,
        spec: PaneSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = self.tab_mut().panes.get_mut(slot) {
            *p = spec;
        }
        self.focus = slot;
        self.save_tabs();
        self.load_pane_inputs(window, cx);
        cx.notify();
    }

    pub(crate) fn close_pane(&mut self, slot: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.max.is_some() {
            self.max = None;
        } else if self.tab().panes.len() > 1 {
            let t = self.tab_mut();
            t.panes.remove(slot);
            t.cols.clear();
        } else {
            self.tab_mut().panes[0] = PaneSpec::default();
        }
        self.focus = self.focus.min(self.tab().panes.len() - 1);
        self.save_tabs();
        self.load_pane_inputs(window, cx);
        cx.notify();
    }

    fn add_canvas(&mut self, spec: PaneSpec, window: &mut Window, cx: &mut Context<Self>) {
        self.pop = None;
        if self.tab().panes.len() >= 4 {
            return;
        }
        let t = self.tab_mut();
        t.panes.push(spec);
        t.cols.clear();
        self.focus = self.tab().panes.len() - 1;
        self.max = None;
        self.settings = false;
        self.save_tabs();
        self.load_pane_inputs(window, cx);
        cx.notify();
    }

    /// Show something in this tab: focus a canvas already showing it, else
    /// take over the focused one.
    pub(crate) fn open(&mut self, spec: PaneSpec, cx: &mut Context<Self>) {
        self.settings = false;
        let same = |p: &PaneSpec| {
            p.kind == spec.kind
                && (!matches!(spec.kind.as_str(), "agent" | "files") || p.agent == spec.agent)
        };
        match self.tab().panes.iter().position(same) {
            Some(i) => self.focus = i,
            None => {
                let f = self.focus;
                self.tab_mut().panes[f] = spec;
                self.panes[f].seen = usize::MAX;
                self.panes[f].preview = None;
                self.save_tabs();
            }
        }
        self.max = None;
        cx.notify();
    }

    fn show_diagram(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for ti in 0..self.prefs.tabs.len() {
            if let Some(pi) = self.prefs.tabs[ti]
                .panes
                .iter()
                .position(|p| p.kind == "diagram")
            {
                self.switch_tab(ti, window, cx);
                self.focus = pi;
                cx.notify();
                return;
            }
        }
        self.open(PaneSpec::of("diagram"), cx);
    }

    // -------------------------------------------------------------- machines

    fn select_machine(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.fleet.select(i);
        self.review = None;
        self.ticket_sel = None;
        self.load_pane_inputs(window, cx);
        self.refresh(cx);
    }

    fn add_machine(&mut self, cx: &mut Context<Self>) {
        if self.m_busy {
            return;
        }
        let name = self.m_name.read(cx).value().to_string();
        let url = self.m_url.read(cx).value().to_string();
        let tok = self.m_tok.read(cx).value().to_string();
        self.m_busy = true;
        self.m_err = None;
        let fleet = self.fleet.clone();
        let task = cx.background_spawn(async move { fleet.add_machine(&name, &url, &tok) });
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let res = task.await;
            let _ = this.update(cx, |this, cx| {
                this.m_busy = false;
                match res {
                    Ok(_) => {
                        this.review = None;
                        this.refresh(cx);
                    }
                    Err(e) => this.m_err = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_share(&mut self, enabled: bool, new_token: bool, cx: &mut Context<Self>) {
        let addr = self.share_addr.read(cx).value().to_string();
        let _ = self.fleet.set_share(enabled, Some(&addr), new_token);
        cx.notify();
    }

    // -------------------------------------------------------------- drawer

    /// Opens after a short hover on the review pill, stays while the pointer
    /// is on the pill or the drawer, closes a little after it leaves.
    fn hover_drawer(&mut self, cta: bool, on: bool, cx: &mut Context<Self>) {
        if cta {
            self.hov_cta = on;
        } else {
            self.hov_drawer = on;
        }
        let want = self.hov_cta || self.hov_drawer || self.pinned;
        if want == self.drawer {
            self.drawer_task = None;
            return;
        }
        let delay = if want { 320 } else { 450 };
        self.drawer_task = Some(cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let want = this.hov_cta || this.hov_drawer || this.pinned;
                    if want != this.drawer {
                        this.drawer = want;
                        cx.notify();
                    }
                });
            },
        ));
    }
}

// ------------------------------------------------------------------ render

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let w = hair(window);
        let t = self.started.elapsed().as_secs_f32();
        let win_bounds = self.win_bounds.clone();
        let vs = window.viewport_size();
        let key = (
            f32::from(vs.width) as u32,
            f32::from(vs.height) as u32,
            self.pal.desk_base,
        );
        if self.desk.as_ref().is_none_or(|(k, _)| *k != key) {
            let img = desk_image(&self.pal, key.0 as f32, key.1 as f32);
            self.desk = Some((key, img));
        }
        let desk = self.desk.as_ref().unwrap().1.clone();
        let body = if self.settings {
            self.settings_page(w, window, cx)
        } else {
            self.grid(w, window, cx)
        };
        let drawer = self.drawer.then(|| self.drawer_el(w, cx));

        let main = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .relative()
            .flex()
            .flex_col()
            .child(self.tbar(w, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .px(px(6.))
                    .pb(px(6.))
                    .child(body),
            )
            .children(drawer);

        let side = self.side_open.then(|| self.sidebar(w, t, cx));
        // Sidebar and main column share one sheet of glass: no divider.
        let win = div()
            .size_full()
            .relative()
            .flex()
            .bg(p.glass)
            .children(side)
            .child(main)
            .when(self.side_open, |d| {
                d.child(
                    div()
                        .id("side-gut")
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(self.side_w - 3.))
                        .w(px(7.))
                        .cursor_col_resize()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, _| this.drag = Some(Drag::Side)),
                        ),
                )
            });
        let pop = self.pop.map(|(pop, at)| self.popover(pop, at, w, cx));

        div()
            .id("root")
            .track_focus(&self.focus_handle)
            .size_full()
            .relative()
            .font_family(UI)
            .text_size(px(13.))
            .line_height(relative(1.4))
            .text_color(p.fg)
            .when(!GLASS, |d| {
                d.bg(p.desk_base)
                    .child(img(desk).absolute().size_full().object_fit(ObjectFit::Fill))
            })
            .child(
                canvas(move |b, _, _| win_bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(win)
            .children(pop)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.pop.is_some() {
                        this.pop = None;
                        cx.notify();
                    }
                }),
            )
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                let key = e.keystroke.key.as_str();
                if key == "escape" {
                    if this.renaming.is_some() {
                        this.commit_rename(None, cx);
                    } else if this.pop.is_some() {
                        this.pop = None;
                    } else if this.settings {
                        this.settings = false;
                    } else if this.drawer {
                        this.drawer = false;
                        this.pinned = false;
                    }
                    cx.notify();
                    return;
                }
                let m = e.keystroke.modifiers;
                if !(m.platform || m.control) {
                    return;
                }
                match key {
                    k if k.len() == 1 && k.as_bytes()[0].is_ascii_digit() && k != "0" => {
                        let i: usize = k.parse().unwrap();
                        this.switch_tab(i - 1, window, cx);
                        cx.stop_propagation();
                    }
                    "t" => {
                        this.pop = Some((Pop::NewTab(2), point(px(320.), px(32.))));
                        this.new_name.update(cx, |s, cx| {
                            s.set_value("", window, cx);
                            s.focus(window, cx)
                        });
                        cx.notify();
                    }
                    "," => {
                        this.settings = true;
                        cx.notify();
                    }
                    "\\" => {
                        this.side_open = !this.side_open;
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if let Some(d) = this.drag {
                    let b = this.grid_bounds.get();
                    let fx = f32::from(e.position.x - b.origin.x) / f32::from(b.size.width).max(1.);
                    let fy =
                        f32::from(e.position.y - b.origin.y) / f32::from(b.size.height).max(1.);
                    let n = this.tab().panes.len();
                    let (mut cols, _) = this.splits(n);
                    match d {
                        Drag::Side => {
                            let left = this.win_bounds.get().origin.x;
                            this.side_w = f32::from(e.position.x - left).clamp(220., 380.);
                        }
                        Drag::Col(0) => {
                            let hi = if n == 3 { cols[1] - 0.12 } else { 0.8 };
                            cols[0] = fx.clamp(0.15, hi);
                            this.tab_mut().cols = cols;
                        }
                        Drag::Col(_) => {
                            cols[1] = fx.clamp(cols[0] + 0.12, 0.85);
                            this.tab_mut().cols = cols;
                        }
                        Drag::Rows => this.tab_mut().rows = fy.clamp(0.2, 0.8),
                    }
                    cx.notify();
                } else if let Some((slot, last)) = this.pan {
                    if let Some(cam) = this.panes[slot].cam.as_mut() {
                        let d = e.position - last;
                        cam.x -= f32::from(d.x) / cam.k;
                        cam.y -= f32::from(d.y) / cam.k;
                        this.pan = Some((slot, e.position));
                        this.panes[slot].user_cam = true;
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.drag.take().is_some_and(|d| d != Drag::Side) {
                        this.save_tabs();
                    }
                    this.pan = None;
                    cx.notify();
                }),
            )
    }
}

fn shadow(a: f32, y: f32, blur: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(0., 0., 0., a),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }]
}

impl Workbench {
    // -------------------------------------------------------------- sidebar

    fn sidebar(&mut self, w: Pixels, t: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let m = self.machine();
        let seg = [
            (SideView::Projects, "folder", "Projects"),
            (SideView::Agents, "sparkle", "Agents"),
        ];
        let rows = self.side_rows(t, cx);
        let link = match &m.link {
            Link::Online => p.green,
            Link::Connecting => p.amber,
            Link::Offline { .. } => p.red,
        };
        div()
            .w(px(self.side_w))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .h(px(38.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .px(px(10.))
                    .window_control_area(WindowControlArea::Drag)
                    .child(ib("side-close", "sidebar", &p, false).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.side_open = false;
                            cx.notify()
                        },
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .pt(px(4.))
                    .px(px(12.))
                    .pb(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.))
                            .child(icon(if m.local { "pc" } else { "cloud" }, 18., p.fg))
                            .child(m.name.clone())
                            .when(!m.local, |d| {
                                d.child(div().size(px(7.)).ml(px(2.)).rounded_full().bg(link))
                            }),
                    )
                    .child(div().w_full().flex().gap(px(2.)).mt(px(6.)).children(
                        seg.into_iter().map(|(k, ic, l)| {
                            let on = self.side == k;
                            div()
                                .id(l)
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .h(px(22.))
                                .px(px(7.))
                                .rounded(px(6.))
                                .text_size(px(11.5))
                                .cursor_pointer()
                                .text_color(if on { p.fg } else { p.fg3 })
                                .when(on, |d| d.bg(p.sel))
                                .hover(|d| d.text_color(p.fg))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.side = k;
                                    cx.notify()
                                }))
                                .child(icon(ic, 14., if on { p.fg } else { p.fg3 }))
                                .child(l)
                        }),
                    )),
            )
            .child(
                div()
                    .id("side-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pt(px(2.))
                    .px(px(6.))
                    .pb(px(8.))
                    .children(self.offline_note())
                    .children(rows),
            )
            .child(self.run_card(w))
            .child(
                div()
                    .h(px(34.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .px(px(10.))
                    .child(div().flex_1())
                    .child(self.machine_switch(cx))
                    .child(div().flex_1())
                    .child(ib("gear", "gear", &p, self.settings).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.settings = !this.settings;
                            cx.notify()
                        },
                    ))),
            )
    }

    /// This machine and every followed one; a dot flags offline, connecting,
    /// or work going on on a machine you are not looking at.
    fn machine_switch(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        div()
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(9.))
            .bg(p.hover)
            .children(self.machines.iter().map(|m| {
                let i = m.index;
                let busy = (m.running > 0 || m.pending > 0) && !m.selected;
                let mark = match &m.link {
                    Link::Offline { .. } => Some(p.red),
                    Link::Connecting => Some(p.amber),
                    Link::Online if busy => Some(p.accent),
                    _ => None,
                };
                div()
                    .id(("mach", i))
                    .relative()
                    .w(px(26.))
                    .h(px(24.))
                    .rounded(px(7.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(m.selected, |d| d.bg(p.pill_on).shadow(shadow(0.08, 1., 2.)))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.select_machine(i, window, cx)),
                    )
                    .child(icon(
                        if m.local { "pc" } else { "cloud" },
                        14.,
                        if m.selected { p.fg } else { p.fg3 },
                    ))
                    .when_some(mark, |d, c| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(3.))
                                .right(px(3.))
                                .size(px(5.))
                                .rounded_full()
                                .bg(c),
                        )
                    })
            }))
            .child(
                div()
                    .id("mach-add")
                    .w(px(26.))
                    .h(px(24.))
                    .rounded(px(7.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|d| d.bg(p.sel))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.settings = true;
                        this.m_name.update(cx, |s, cx| s.focus(window, cx));
                        cx.notify()
                    }))
                    .child(icon("plus", 14., p.fg3)),
            )
    }

    fn row(&self, id: impl Into<ElementId>, indent: f32, sel: bool) -> Stateful<Div> {
        let p = self.pal;
        div()
            .id(id)
            .w_full()
            .flex()
            .items_center()
            .gap(px(7.))
            .h(px(27.))
            .pl(px(indent))
            .pr(px(8.))
            .rounded(px(6.))
            .whitespace_nowrap()
            .cursor_pointer()
            .when(sel, |d| d.bg(p.sel))
            .hover(|d| d.bg(if sel { p.sel } else { p.hover }))
    }

    fn agent_row(
        &self,
        a: &AgentRecord,
        indent: f32,
        t: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.pal;
        let id = a.id;
        let sel = self
            .tab()
            .panes
            .get(self.focus)
            .is_some_and(|s| s.kind == "agent" && s.agent == id);
        self.row(("agent", id), indent, sel)
            .on_click(cx.listener(move |this, _, _, cx| this.open(PaneSpec::agent(id), cx)))
            .child(icon("sparkle", 14., p.accent))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .when(a.kind == AgentKind::Triage, |d| d.text_color(p.fg3))
                    .child(a.title.clone()),
            )
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(11.))
                    .text_color(p.fg3)
                    .when(!a.escalations.is_empty(), |d| {
                        d.child(
                            div()
                                .text_color(p.amber)
                                .child(format!("↑{}", a.escalations.len())),
                        )
                    })
                    .child(status_dot(&p, a.status, t)),
            )
    }

    fn side_rows(&mut self, t: f32, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = self.pal;
        let s = &self.snap;
        let mut out: Vec<AnyElement> = vec![];
        if s.agents.is_empty() {
            return out;
        }
        match self.side {
            SideView::Projects => {
                out.push(
                    self.row("proj", 8., true)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(icon("folder", 14., p.fg))
                        .child(div().min_w_0().truncate().child(s.name.clone()))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(p.fg3)
                                .child(s.agents.len().to_string()),
                        )
                        .into_any_element(),
                );
                for id in tree_order(s) {
                    let a = &s.agents[id];
                    out.push(
                        self.agent_row(a, if a.depth > 0 { 36. } else { 22. }, t, cx)
                            .into_any_element(),
                    );
                }
                for (ti, tab) in self.prefs.tabs.iter().enumerate() {
                    for (pi, pane) in tab.panes.iter().enumerate() {
                        let Some(u) = pane.url.as_ref().filter(|_| pane.kind == "browser") else {
                            continue;
                        };
                        out.push(
                            self.row(("pv", ti * 4 + pi), 22., false)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.switch_tab(ti, window, cx);
                                    this.focus = pi;
                                }))
                                .child(icon("globe", 14., p.blue))
                                .child(div().min_w_0().truncate().child(format!(
                                    "Preview · {}",
                                    u.trim_start_matches("http://")
                                        .trim_start_matches("https://")
                                )))
                                .into_any_element(),
                        );
                    }
                }
                let wts: Vec<&AgentRecord> = s
                    .agents
                    .iter()
                    .filter(|a| a.branch.is_some() && a.id != MAIN)
                    .collect();
                if !wts.is_empty() {
                    out.push(
                        self.row("wts", 8., false)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(icon("folder", 14., p.fg))
                            .child(div().child("worktrees"))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(p.fg3)
                                    .child(wts.len().to_string()),
                            )
                            .into_any_element(),
                    );
                    for a in wts {
                        let id = a.id;
                        out.push(
                            self.row(("wt", id), 22., false)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open(PaneSpec::files(id), cx)
                                }))
                                .child(icon("branch", 14., p.fg3))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .child(a.branch.clone().unwrap_or_default()),
                                )
                                .child(
                                    div()
                                        .ml_auto()
                                        .text_size(px(11.))
                                        .text_color(p.fg3)
                                        .child(a.ticket.clone().unwrap_or_default()),
                                )
                                .into_any_element(),
                        );
                    }
                }
            }
            SideView::Agents => {
                out.push(
                    div()
                        .pt(px(10.))
                        .px(px(10.))
                        .pb(px(4.))
                        .text_size(px(11.))
                        .text_color(p.fg3)
                        .child(format!("{} · {} agents", s.name, s.agents.len()))
                        .into_any_element(),
                );
                for id in tree_order(s) {
                    let a = &s.agents[id];
                    out.push(self.agent_row(a, 8., t, cx).into_any_element());
                    out.push(
                        div()
                            .ml(px(37.))
                            .mt(px(-3.))
                            .mb(px(3.))
                            .font_family(MONO)
                            .text_size(px(11.))
                            .text_color(p.fg3)
                            .truncate()
                            .child(format!("{} · ${:.3}", route(a), s.subtree_cost(id)))
                            .into_any_element(),
                    );
                }
            }
        }
        out
    }

    fn run_card(&self, w: Pixels) -> impl IntoElement {
        let p = self.pal;
        let s = &self.snap;
        let running = s
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Running)
            .count();
        let wts = s
            .agents
            .iter()
            .filter(|a| a.branch.is_some() && a.id != MAIN)
            .count();
        let pending = self.pending();
        let row = |d: Div, l: &str, v: String| {
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(24.))
                .px(px(3.))
                .text_size(px(12.))
                .child(d)
                .child(div().truncate().child(l.to_string()))
                .child(
                    div()
                        .ml_auto()
                        .font_family(MONO)
                        .text_size(px(11.))
                        .text_color(p.fg3)
                        .whitespace_nowrap()
                        .child(v),
                )
        };
        let name = if s.name.is_empty() {
            self.machine().name
        } else {
            s.name.clone()
        };
        div()
            .mx(px(8.))
            .my(px(6.))
            .py(px(8.))
            .px(px(9.))
            .rounded(px(10.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.fg3)
                    .mb(px(4.))
                    .child(format!("This run · {name}")),
            )
            .child(row(
                dot(
                    p.accent,
                    (running > 0).then(|| self.started.elapsed().as_secs_f32()),
                ),
                "Agents running",
                running.to_string(),
            ))
            .child(row(
                dot(if pending > 0 { p.amber } else { p.fg4 }, None),
                "Waiting on you",
                pending.to_string(),
            ))
            .child(row(dot(p.green, None), "Worktrees", wts.to_string()))
            .child(row(
                dot(p.fg4, None),
                "Spent",
                format!(
                    "${:.3} · router ${:.3}",
                    s.total_cost_usd, s.router_cost_usd
                ),
            ))
    }

    // -------------------------------------------------------------- title bar

    fn tbar(&mut self, w: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let pending = self.pending();
        let tickets = self.snap.tickets.len();
        let pill = |id: ElementId, sel: bool| {
            div()
                .id(id)
                .relative()
                .h(px(22.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(5.))
                .rounded(px(6.))
                .text_size(px(11.5))
                .whitespace_nowrap()
                .cursor_pointer()
                .bg(if sel { p.pill_on } else { p.pill })
                .hair_all(w, p.pill_edge)
                .text_color(if sel { p.fg } else { p.fg2 })
                .when(sel, |d| d.shadow(shadow(0.08, 1., 2.)))
                .hover(|d| d.text_color(p.fg))
        };
        let tabs: Vec<AnyElement> = self
            .prefs
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let sel = i == self.prefs.active_tab && !self.settings;
                let badge = pending > 0 && t.panes.iter().any(|p| p.kind == "diagram");
                let n = t.panes.len().clamp(1, 4);
                let el = pill(("tab", i).into(), sel);
                if self.renaming == Some(i) {
                    return el
                        .border_color(p.blue)
                        .child(
                            div().w(px(110.)).child(
                                Input::new(&self.rename)
                                    .appearance(false)
                                    .text_size(px(11.5))
                                    .h(px(18.))
                                    .p_0(),
                            ),
                        )
                        .into_any_element();
                }
                el.on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                    if e.click_count() == 2 {
                        this.start_rename(i, window, cx);
                    } else {
                        this.commit_rename(None, cx);
                        this.switch_tab(i, window, cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        this.pop = Some((Pop::TabMenu(i), e.position));
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .map(|d| match &t.name {
                    Some(name) => d.child(name.clone()),
                    None => d.child(
                        svg()
                            .path(icons::path(&format!("lay{n}")))
                            .w(px(16.))
                            .h(px(12.))
                            .text_color((if sel { p.fg } else { p.fg3 }).opacity(0.85)),
                    ),
                })
                .when(badge, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(-3.))
                            .right(px(-3.))
                            .size(px(7.))
                            .rounded_full()
                            .bg(p.blue)
                            .border(px(1.5))
                            .border_color(p.glass),
                    )
                })
                .into_any_element()
            })
            .collect();
        let (upd_label, upd_icon, upd_new, upd_url) = match &self.upd {
            Upd::Checking => ("Checking…".to_string(), None, false, None),
            Upd::Done(Ok(u)) if u.newer => (
                "Download & Update".to_string(),
                Some("down"),
                true,
                Some(u.url.clone()),
            ),
            Upd::Done(Ok(_)) => ("Up to date".to_string(), Some("check"), false, None),
            _ => ("Check for updates".to_string(), Some("reload"), false, None),
        };
        div()
            .h(px(38.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(if !self.side_open && GLASS { 84. } else { 4. }))
            .pr(px(10.))
            .window_control_area(WindowControlArea::Drag)
            .when(!self.side_open, |d| {
                d.child(ib("side-open", "sidebar", &p, false).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.side_open = true;
                        cx.notify()
                    },
                )))
            })
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .items_center()
                    .min_w_0()
                    .children(tabs)
                    .child(
                        pill("newtab".into(), false)
                            .px(px(6.))
                            .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                                this.pop = Some((Pop::NewTab(2), e.position()));
                                this.new_name.update(cx, |s, cx| {
                                    s.set_value("", window, cx);
                                    s.focus(window, cx)
                                });
                                cx.notify();
                            }))
                            .child(icon("plus", 14., p.fg2)),
                    ),
            )
            .child(div().flex_1())
            .child(ib("addcanvas", "plus", &p, false).on_click(cx.listener(
                |this, e: &ClickEvent, _, cx| {
                    this.pop = Some((Pop::AddCanvas, e.position()));
                    cx.notify();
                },
            )))
            .child(
                div()
                    .id("cta")
                    .h(px(22.))
                    .px(px(10.))
                    .rounded_full()
                    .text_size(px(11.5))
                    .border_1()
                    .border_color(p.fg2)
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .when(self.drawer, |d| d.bg(p.sel))
                    .hover(|d| d.bg(p.hover))
                    .on_hover(
                        cx.listener(|this, on: &bool, _, cx| this.hover_drawer(true, *on, cx)),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.drawer_task = None;
                        this.pinned = !this.pinned || !this.drawer;
                        this.drawer = this.pinned;
                        cx.notify();
                    }))
                    .map(|d| {
                        if pending > 0 {
                            d.child(dot(p.amber, None))
                                .child(format!("{pending} to review"))
                        } else {
                            d.child(icon("check", 14., p.fg)).child("Nothing to review")
                        }
                    })
                    .when(tickets > 0, |d| {
                        d.child(
                            div()
                                .text_color(p.fg3)
                                .child(format!("· {tickets} tickets")),
                        )
                    }),
            )
            .when(
                self.prefs.check_updates || !matches!(self.upd, Upd::Idle),
                |d| {
                    d.child(
                        div()
                            .id("upd")
                            .h(px(22.))
                            .px(px(10.))
                            .rounded_full()
                            .text_size(px(11.5))
                            .border_1()
                            .border_color(if upd_new { p.fg2 } else { p.pill_edge })
                            .text_color(if upd_new { p.fg } else { p.fg3 })
                            .when(upd_new, |d| d.font_weight(FontWeight::MEDIUM))
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .whitespace_nowrap()
                            .cursor_pointer()
                            .hover(|d| d.bg(p.hover).text_color(p.fg))
                            .on_click(cx.listener(move |this, _, _, cx| match &upd_url {
                                Some(u) => cx.open_url(u),
                                None => this.check_update(cx),
                            }))
                            .when_some(upd_icon, |d, ic| {
                                d.child(icon(ic, 14., if upd_new { p.fg } else { p.fg3 }))
                            })
                            .child(upd_label),
                    )
                },
            )
    }

    // -------------------------------------------------------------- popovers

    fn popover(
        &mut self,
        pop: Pop,
        at: Point<Pixels>,
        w: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.pal;
        let item = |id: ElementId, ic: &'static str, label: String, ic_color: Hsla| {
            div()
                .id(id)
                .w_full()
                .h(px(28.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .rounded(px(6.))
                .whitespace_nowrap()
                .cursor_pointer()
                .hover(|d| d.bg(p.sel))
                .child(icon(ic, 14., ic_color))
                .child(label)
        };
        let label = |s: &'static str| {
            div()
                .mt(px(4.))
                .mx(px(8.))
                .mb(px(6.))
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.fg3)
                .child(s)
        };
        let rule = || div().my(px(5.)).mx(px(4.)).h(w).bg(p.pane_edge);
        let content: AnyElement = match pop {
            Pop::TabMenu(i) => {
                let many = self.prefs.tabs.len() > 1;
                div()
                    .min_w(px(200.))
                    .child(
                        item("pm-rename".into(), "doc", "Rename".into(), p.fg3)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.start_rename(i, window, cx)
                            }))
                            .child(
                                div()
                                    .ml_auto()
                                    .pl(px(12.))
                                    .text_size(px(11.))
                                    .text_color(p.fg4)
                                    .child("double-click"),
                            ),
                    )
                    .child(
                        item("pm-dup".into(), "copy", "Duplicate".into(), p.fg3).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.pop = None;
                                let t = this.prefs.tabs[i].clone();
                                this.prefs.tabs.insert(i + 1, t);
                                this.switch_tab(i + 1, window, cx);
                            }),
                        ),
                    )
                    .child(rule())
                    .child(
                        item("pm-close".into(), "close", "Close tab".into(), p.fg3)
                            .when(!many, |d| d.opacity(0.45))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.pop = None;
                                this.close_tab(i, window, cx);
                            })),
                    )
                    .into_any_element()
            }
            Pop::NewTab(n) => {
                let lay = |k: usize| {
                    let on = k == n;
                    div()
                        .id(("lay", k))
                        .flex_1()
                        .h(px(44.))
                        .rounded(px(8.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(2.))
                        .cursor_pointer()
                        .hair_all(w, if on { p.fg3 } else { p.pane_edge })
                        .when(on, |d| d.bg(p.pill_on))
                        .text_size(px(10.5))
                        .text_color(if on { p.fg } else { p.fg3 })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some((Pop::NewTab(_), at)) = this.pop {
                                this.pop = Some((Pop::NewTab(k), at));
                            }
                            this.new_name.update(cx, |s, cx| s.focus(window, cx));
                            cx.notify()
                        }))
                        .child(
                            svg()
                                .path(icons::path(&format!("lay{k}")))
                                .w(px(22.))
                                .h(px(16.))
                                .text_color(if on { p.fg } else { p.fg3 }),
                        )
                        .child(if k == 4 {
                            "2×2".to_string()
                        } else {
                            k.to_string()
                        })
                };
                div()
                    .w(px(260.))
                    .child(label("New tab"))
                    .child(
                        div()
                            .mx(px(4.))
                            .mb(px(10.))
                            .h(px(28.))
                            .px(px(9.))
                            .rounded(px(7.))
                            .bg(p.hover)
                            .hair_all(w, p.pane_edge)
                            .flex()
                            .items_center()
                            .child(
                                Input::new(&self.new_name)
                                    .appearance(false)
                                    .text_size(px(12.5))
                                    .p_0(),
                            ),
                    )
                    .child(label("Canvases"))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .mx(px(4.))
                            .mb(px(10.))
                            .children((1..=4).map(lay)),
                    )
                    .child(
                        div().flex().justify_end().mx(px(4.)).mb(px(2.)).child(
                            btn("nt-go", "Create", &p, w)
                                .bg(p.fg)
                                .text_color(p.desk_base)
                                .border_color(transparent_black())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let name = this.new_name.read(cx).value().to_string();
                                    this.create_tab(name, n, window, cx);
                                })),
                        ),
                    )
                    .into_any_element()
            }
            Pop::AddCanvas => {
                if self.tab().panes.len() >= 4 {
                    div()
                        .w(px(260.))
                        .child(label("Add a canvas"))
                        .child(
                            div()
                                .mx(px(8.))
                                .mb(px(6.))
                                .text_size(px(11.5))
                                .text_color(p.fg3)
                                .child("This tab already has four canvases. Close one, or open a new tab with ⌘T."),
                        )
                        .into_any_element()
                } else {
                    let agents: Vec<(usize, String, String)> = tree_order(&self.snap)
                        .into_iter()
                        .map(|id| {
                            let a = &self.snap.agents[id];
                            let title = if id == MAIN {
                                "Main agent".to_string()
                            } else {
                                a.title.clone()
                            };
                            (id, title, format!("{:?}", a.status).to_lowercase())
                        })
                        .collect();
                    let views = [
                        ("files", "folder", "Worktree files"),
                        ("browser", "globe", "Browser"),
                        ("diagram", "diagram", "Diagram review"),
                        ("docs", "doc", "PLAN.md"),
                        ("empty", "window", "Empty canvas"),
                    ];
                    div()
                        .min_w(px(240.))
                        .child(label("Sessions"))
                        .children(agents.into_iter().map(|(id, title, st)| {
                            item(("pa", id).into(), "sparkle", title, p.accent)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.add_canvas(PaneSpec::agent(id), window, cx)
                                }))
                                .child(
                                    div()
                                        .ml_auto()
                                        .pl(px(12.))
                                        .text_size(px(11.))
                                        .text_color(p.fg4)
                                        .child(st),
                                )
                        }))
                        .child(rule())
                        .child(label("Views"))
                        .children(views.into_iter().map(|(k, ic, l)| {
                            item(k.into(), ic, l.into(), p.fg3).on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.add_canvas(PaneSpec::of(k), window, cx)
                                },
                            ))
                        }))
                        .into_any_element()
                }
            }
        };
        let win = self.win_bounds.get();
        let x = f32::from(at.x - win.origin.x)
            .min(f32::from(win.size.width) - 280.)
            .max(8.);
        div()
            .id("pop")
            .absolute()
            .left(px(x))
            .top(px(f32::from(at.y - win.origin.y).max(30.) + 10.))
            .p(px(6.))
            .rounded(px(11.))
            .bg(p.sheet)
            .hair_all(w, p.pane_edge)
            .shadow(shadow(0.18, 14., 34.))
            .text_size(px(12.5))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(content)
            .into_any_element()
    }

    // -------------------------------------------------------------- tickets drawer

    fn drawer_el(&mut self, w: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let s = self.snap.clone();
        let detail = self
            .ticket_sel
            .as_ref()
            .and_then(|k| s.tickets.iter().find(|t| &t.key == k))
            .cloned();
        let group_label = |s: String| {
            div()
                .pt(px(10.))
                .px(px(10.))
                .pb(px(4.))
                .text_size(px(11.))
                .text_color(p.fg3)
                .child(s)
        };
        let body: AnyElement = match detail {
            Some(t) => self.ticket_detail(&t, w, cx),
            None => {
                let mut items: Vec<AnyElement> = vec![div()
                    .flex()
                    .gap(px(6.))
                    .m(px(2.))
                    .mb(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h(px(28.))
                            .px(px(9.))
                            .rounded(px(7.))
                            .bg(p.hover)
                            .hair_all(w, p.pane_edge)
                            .flex()
                            .items_center()
                            .child(
                                Input::new(&self.ticket_input)
                                    .appearance(false)
                                    .text_size(px(12.5))
                                    .p_0(),
                            ),
                    )
                    .child(btn("tfile", "File", &p, w).on_click(cx.listener(
                        |this, _, window, cx| {
                            let text = this.ticket_input.read(cx).value().trim().to_string();
                            this.ticket_input
                                .update(cx, |s, cx| s.set_value("", window, cx));
                            this.file_ticket(text, cx);
                        },
                    )))
                    .into_any_element()];
                if let Some(e) = &self.ticket_err {
                    items.push(
                        div()
                            .mx(px(4.))
                            .mb(px(6.))
                            .text_size(px(12.))
                            .text_color(p.red)
                            .child(e.clone())
                            .into_any_element(),
                    );
                }
                let pend: Vec<_> = s.pending_approvals().cloned().collect();
                if !pend.is_empty() {
                    items.push(
                        group_label(format!("Waiting on you · {}", pend.len())).into_any_element(),
                    );
                    for a in pend {
                        let id = a.id;
                        let title = match a.kind {
                            backspace_core::ApprovalKind::Plan => {
                                format!("Plan · {} tickets", a.tickets.len())
                            }
                            _ if a.agent == MAIN => "Final deliverable".into(),
                            _ => s
                                .agents
                                .get(a.agent)
                                .map_or("Deliverable".into(), |x| x.title.clone()),
                        };
                        items.push(
                            div()
                                .id(("drv", id))
                                .w_full()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .p(px(8.))
                                .mb(px(6.))
                                .rounded(px(8.))
                                .bg(p.pane)
                                .hair_all(w, p.pane_edge)
                                .cursor_pointer()
                                .hover(|d| d.border_color(p.pane_edge_on))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.review = Some(id);
                                    this.show_diagram(window, cx);
                                }))
                                .child(icon("diagram", 14., p.amber))
                                .child(div().flex_1().min_w_0().truncate().child(title))
                                .child(st_pill(w, "review", p.amber))
                                .into_any_element(),
                        );
                    }
                }
                use TicketState::*;
                let groups: [(&str, &[TicketState]); 5] = [
                    ("Needs you", &[Proposed, InReview, ReadyForHuman, NeedsInfo]),
                    ("Running", &[Queued, InProgress]),
                    ("Ready", &[ReadyForAgent, NeedsTriage]),
                    ("Done", &[Done]),
                    ("Closed", &[Failed, Wontfix]),
                ];
                for (name, states) in groups {
                    let rows: Vec<&Ticket> = s
                        .tickets
                        .iter()
                        .filter(|t| states.contains(&t.state))
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    items.push(group_label(format!("{name} · {}", rows.len())).into_any_element());
                    for t in rows {
                        let key = t.key.clone();
                        let c = ticket_color(&p, t.state);
                        items.push(
                            div()
                                .id(("trow", t.num))
                                .w_full()
                                .min_h(px(30.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .py(px(4.))
                                .px(px(8.))
                                .rounded(px(7.))
                                .cursor_pointer()
                                .hover(|d| d.bg(p.hover))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.ticket_sel = Some(key.clone());
                                    cx.notify()
                                }))
                                .child(
                                    div()
                                        .w(px(22.))
                                        .font_family(MONO)
                                        .text_size(px(11.))
                                        .text_color(p.fg3)
                                        .child(format!("{:02}", t.num)),
                                )
                                .child(div().flex_1().min_w_0().truncate().child(t.title.clone()))
                                .child(st_pill(w, t.state.label(), c))
                                .into_any_element(),
                        );
                    }
                }
                if s.tickets.is_empty() && s.pending_approvals().next().is_none() {
                    items.push(nothing(
                        &p,
                        "No tickets yet. The main agent creates them from your goal; you can file one above.",
                    ));
                }
                div().children(items).into_any_element()
            }
        };
        let name = if s.name.is_empty() {
            self.machine().name
        } else {
            s.name.clone()
        };
        div()
            .id("drawer")
            .absolute()
            .top(px(38.))
            .right(px(6.))
            .bottom(px(6.))
            .w(px(400.))
            .flex()
            .flex_col()
            .rounded(px(12.))
            .bg(p.sheet)
            .hair_all(w, p.pane_edge)
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.16),
                offset: point(px(-10.), px(16.)),
                blur_radius: px(40.),
                spread_radius: px(0.),
                inset: false,
            }])
            .overflow_hidden()
            .on_hover(cx.listener(|this, on: &bool, _, cx| this.hover_drawer(false, *on, cx)))
            .child(
                div()
                    .h(px(38.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .pl(px(12.))
                    .pr(px(8.))
                    .hair_b(w, p.pane_edge)
                    .text_size(px(12.))
                    .text_color(p.fg3)
                    .child(icon("ticket", 14., p.fg3))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.fg)
                            .child("Tickets"),
                    )
                    .child(format!("· {name}"))
                    .child(div().flex_1())
                    .child(ib("dr-pin", "pin", &p, self.pinned).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.pinned = !this.pinned;
                            cx.notify()
                        },
                    )))
                    .child(ib("dr-close", "close", &p, false).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.drawer = false;
                            this.pinned = false;
                            cx.notify()
                        },
                    ))),
            )
            .child(
                div()
                    .id("dr-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(8.))
                    .child(body),
            )
            .into_any_element()
    }

    fn ticket_detail(&mut self, t: &Ticket, w: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let c = ticket_color(&p, t.state);
        let a = t.assignee.and_then(|i| self.snap.agents.get(i)).cloned();
        let kv = |k: &str, v: AnyElement| {
            div()
                .flex()
                .items_start()
                .gap(px(16.))
                .child(div().text_color(p.fg3).child(k.to_string()))
                .child(div().flex_1().flex().justify_end().text_right().child(v))
        };
        let code = |s: String| {
            div()
                .font_family(MONO)
                .text_size(px(11.5))
                .bg(p.hover)
                .px(px(5.))
                .py(px(1.))
                .rounded(px(4.))
                .child(s)
                .into_any_element()
        };
        let text = |s: String| div().child(s).into_any_element();
        let section = |title: &str, body: AnyElement| {
            div()
                .py(px(10.))
                .hair_b(w, p.pane_edge)
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(p.fg3)
                        .mb(px(8.))
                        .child(format!("▾ {title}")),
                )
                .child(body)
        };
        let list = |xs: &[String]| {
            if xs.is_empty() {
                div().text_color(p.fg3).child("None").into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .children(xs.iter().map(|x| {
                        div()
                            .flex()
                            .child(div().w(px(14.)).flex_none().child("•"))
                            .child(div().flex_1().child(x.clone()))
                    }))
                    .into_any_element()
            }
        };
        let state_word = {
            let l = t.state.label().replace('-', " ");
            let mut c = l.chars();
            c.next().map_or(String::new(), |f| {
                f.to_uppercase().collect::<String>() + c.as_str()
            })
        };
        let mut rows = div()
            .flex()
            .flex_col()
            .gap(px(9.))
            .pb(px(14.))
            .hair_b(w, p.pane_edge)
            .text_size(px(12.5))
            .child(kv("Ticket", code(format!("{:02} · {}", t.num, t.key))))
            .child(kv(
                "Category",
                text(format!("{:?}", t.category).to_lowercase()),
            ));
        rows = rows.child(kv(
            "Agent",
            match &a {
                Some(a) => {
                    let id = a.id;
                    div()
                        .id("td-agent")
                        .text_color(p.blue)
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.open(PaneSpec::agent(id), cx)),
                        )
                        .child(a.title.clone())
                        .into_any_element()
                }
                None => text("unassigned".into()),
            },
        ));
        if let Some(b) = a.as_ref().and_then(|a| a.branch.clone()) {
            rows = rows.child(kv("Branch", code(b)));
        }
        rows = rows.child(kv(
            "Check",
            t.check.clone().map_or(text("none".into()), code),
        ));
        if !t.blocked_by.is_empty() {
            rows = rows.child(kv("After", text(t.blocked_by.join(", "))));
        }
        if let Some(b) = t.budget_usd {
            rows = rows.child(kv("Budget", text(format!("${b:.2}"))));
        }
        div()
            .px(px(8.))
            .pt(px(6.))
            .pb(px(20.))
            .child(
                div()
                    .id("td-back")
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(12.))
                    .text_color(p.fg3)
                    .mb(px(10.))
                    .cursor_pointer()
                    .hover(|d| d.text_color(p.fg))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.ticket_sel = None;
                        cx.notify()
                    }))
                    .child(icon("back", 14., p.fg3))
                    .child("All tickets"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c)
                    .child(icon(
                        match t.state {
                            TicketState::Done => "check",
                            TicketState::InProgress => "sparkle",
                            _ => "ticket",
                        },
                        14.,
                        c,
                    ))
                    .child(state_word),
            )
            .child(
                div()
                    .mt(px(6.))
                    .mb(px(14.))
                    .text_size(px(18.))
                    .line_height(relative(1.3))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t.title.clone()),
            )
            .child(rows)
            .child(section(
                "What to build",
                div()
                    .line_height(relative(1.55))
                    .child(t.what_to_build.clone())
                    .into_any_element(),
            ))
            .child(section("Acceptance", list(&t.acceptance)))
            .when(!t.out_of_scope.is_empty(), |d| {
                d.child(section("Out of scope", list(&t.out_of_scope)))
            })
            .when(!t.notes.is_empty(), |d| {
                d.child(section("Notes", list(&t.notes)))
            })
            .into_any_element()
    }

    // -------------------------------------------------------------- settings

    fn settings_page(
        &mut self,
        w: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.pal;
        let share = self.fleet.prefs().share;
        let sharing = self.fleet.sharing();
        let share_err = self.fleet.share_error();
        if self.share_addr.read(cx).value().is_empty() {
            let a = share.addr.clone();
            self.share_addr
                .update(cx, |s, cx| s.set_value(a, window, cx));
        }
        let h2 = |s: &str| {
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::BOLD)
                .mb(px(4.))
                .child(s.to_string())
        };
        let lead = |s: &str| {
            div()
                .text_size(px(12.5))
                .text_color(p.fg3)
                .line_height(relative(1.5))
                .mb(px(12.))
                .child(s.to_string())
        };
        let section = || div().py(px(18.)).hair_t(w, p.pane_edge);
        let line = |label: AnyElement| {
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .min_h(px(34.))
                .child(div().flex_1().min_w_0().child(label))
        };
        let lab = |t: String, small: Option<String>| {
            div()
                .child(t)
                .when_some(small, |d, s| {
                    d.child(
                        div()
                            .text_size(px(11.5))
                            .text_color(p.fg3)
                            .mt(px(1.))
                            .child(s),
                    )
                })
                .into_any_element()
        };
        let toggle = |id: &'static str, on: bool| {
            div()
                .id(id)
                .w(px(34.))
                .h(px(20.))
                .rounded_full()
                .flex_none()
                .relative()
                .cursor_pointer()
                .bg(if on { p.green } else { p.fg4 })
                .child(
                    div()
                        .absolute()
                        .top(px(2.))
                        .left(px(if on { 16. } else { 2. }))
                        .size(px(16.))
                        .rounded_full()
                        .bg(rgb(0xffffff)),
                )
        };
        let field = |input: &Entity<InputState>, width: Option<f32>, mono: bool| {
            div()
                .h(px(28.))
                .px(px(9.))
                .rounded(px(7.))
                .bg(p.hover)
                .hair_all(w, p.pane_edge)
                .flex()
                .items_center()
                .map(|d| match width {
                    Some(x) => d.w(px(x)).flex_none(),
                    None => d.flex_1().min_w_0(),
                })
                .child(
                    Input::new(input)
                        .appearance(false)
                        .text_size(px(12.5))
                        .when(mono, |i| i.font_family(MONO))
                        .p_0(),
                )
        };
        let theme = self.prefs.theme.clone();
        let seg = div()
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(8.))
            .bg(p.hover)
            .children(
                [("system", "System"), ("light", "Light"), ("dark", "Dark")].map(|(k, l)| {
                    let on = theme == k;
                    div()
                        .id(k)
                        .h(px(24.))
                        .px(px(12.))
                        .rounded(px(6.))
                        .flex()
                        .items_center()
                        .text_size(px(12.))
                        .cursor_pointer()
                        .text_color(if on { p.fg } else { p.fg3 })
                        .when(on, |d| d.bg(p.pill_on).shadow(shadow(0.08, 1., 2.)))
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.set_theme(k, window, cx)),
                        )
                        .child(l)
                }),
            );
        let machines: Vec<AnyElement> = self
            .machines
            .clone()
            .into_iter()
            .map(|m| {
                let i = m.index;
                let state = match &m.link {
                    Link::Online => "online".to_string(),
                    Link::Connecting => "connecting".to_string(),
                    Link::Offline { error } => format!("offline: {error}"),
                };
                let small = format!(
                    "{} · {state}{}",
                    m.url.clone().unwrap_or_else(|| "This machine".into()),
                    if m.project.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", m.project)
                    }
                );
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .py(px(8.))
                    .px(px(10.))
                    .mb(px(6.))
                    .rounded(px(9.))
                    .hair_all(w, p.pane_edge)
                    .child(icon(if m.local { "pc" } else { "cloud" }, 14., p.fg2))
                    .child(div().flex_1().min_w_0().child(lab(
                        format!("{}{}", m.name, if m.selected { " · showing" } else { "" }),
                        Some(small),
                    )))
                    .when(!m.selected, |d| {
                        d.child(btn(("m-show", i), "Show", &p, w).on_click(cx.listener(
                            move |this, _, window, cx| this.select_machine(i, window, cx),
                        )))
                    })
                    .when(!m.local, |d| {
                        d.child(
                            btn(("m-rm", i), "Remove", &p, w)
                                .text_color(p.red)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.fleet.remove_machine(i);
                                    this.refresh(cx);
                                })),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let (upd_line, upd_new, upd_url) = match &self.upd {
            Upd::Idle => ("Not checked yet.".to_string(), false, None),
            Upd::Checking => ("Checking…".to_string(), false, None),
            Upd::Done(Err(e)) => (e.clone(), false, None),
            Upd::Done(Ok(u)) if u.newer => (
                format!("Version {} is available.", u.latest),
                true,
                Some(u.url.clone()),
            ),
            Upd::Done(Ok(_)) => ("You're on the latest release.".to_string(), false, None),
        };
        let check_updates = self.prefs.check_updates;
        let share_on = share.enabled;
        div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .rounded(px(9.))
            .bg(p.sheet)
            .hair_all(w, p.pane_edge)
            .child(
                div()
                    .max_w(px(680.))
                    .mx_auto()
                    .pt(px(28.))
                    .px(px(28.))
                    .pb(px(60.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .mb(px(18.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Settings"),
                            )
                            .child(ib("set-close", "close", &p, false).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.settings = false;
                                    cx.notify()
                                },
                            ))),
                    )
                    .child(
                        section()
                            .child(h2("Appearance"))
                            .child(line(lab("Theme".into(), None)).child(seg)),
                    )
                    .child(
                        section()
                            .child(h2("Machines"))
                            .child(lead("Follow and drive the harness on other machines. Each one runs backspace-cli serve (or shares from its own Backspace, below); add it with its address and token. Switch machines from the sidebar foot."))
                            .children(machines)
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .mt(px(10.))
                                    .child(field(&self.m_name, Some(150.), false))
                                    .child(field(&self.m_url, None, true))
                                    .child(field(&self.m_tok, None, true))
                                    .child(
                                        btn("m-add", if self.m_busy { "Checking…" } else { "Add" }, &p, w)
                                            .bg(p.fg)
                                            .text_color(p.desk_base)
                                            .border_color(transparent_black())
                                            .on_click(cx.listener(|this, _, _, cx| this.add_machine(cx))),
                                    ),
                            )
                            .when_some(self.m_err.clone(), |d, e| {
                                d.child(div().mt(px(6.)).text_size(px(12.)).text_color(p.red).child(e))
                            }),
                    )
                    .child(
                        section()
                            .child(h2("Share this machine"))
                            .child(lead("Let other machines follow this harness and approve its work. Plain HTTP with a token: keep it on localhost and use an SSH tunnel, or a private network such as Tailscale."))
                            .child(
                                line(lab(
                                    "Sharing".into(),
                                    Some(if sharing {
                                        format!("On at http://{}", share.addr)
                                    } else {
                                        "Off".into()
                                    }),
                                ))
                                .child(toggle("share-t", share_on).on_click(cx.listener(
                                    move |this, _, _, cx| this.set_share(!share_on, false, cx),
                                ))),
                            )
                            .child(
                                line(lab("Address".into(), None))
                                    .child(field(&self.share_addr, Some(200.), true))
                                    .child(btn("sh-apply", "Apply", &p, w).on_click(cx.listener(
                                        move |this, _, _, cx| this.set_share(share_on, false, cx),
                                    ))),
                            )
                            .child(
                                line(lab("Token".into(), None))
                                    .child(div().font_family(MONO).text_size(px(12.)).child(share.token.clone()))
                                    .child(btn("sh-new", "New token", &p, w).on_click(cx.listener(
                                        move |this, _, _, cx| this.set_share(share_on, true, cx),
                                    ))),
                            )
                            .when_some(share_err, |d, e| {
                                d.child(div().mt(px(6.)).text_size(px(12.)).text_color(p.red).child(e))
                            })
                            .child(
                                div()
                                    .mt(px(10.))
                                    .py(px(10.))
                                    .px(px(12.))
                                    .rounded(px(8.))
                                    .bg(p.hover)
                                    .font_family(MONO)
                                    .text_size(px(12.))
                                    .child(format!(
                                        "ssh -L 7420:{} you@this-machine   # then add http://127.0.0.1:7420 elsewhere",
                                        share.addr
                                    )),
                            ),
                    )
                    .child(
                        section()
                            .child(h2("Updates"))
                            .child(
                                line(lab("Check for updates automatically".into(), None)).child(
                                    toggle("upd-t", check_updates).on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.prefs.check_updates = !this.prefs.check_updates;
                                            let on = this.prefs.check_updates;
                                            this.fleet.update_prefs(|p| p.check_updates = on);
                                            cx.notify()
                                        },
                                    )),
                                ),
                            )
                            .child(
                                line(lab(
                                    format!("Backspace {}", backspace_core::update::VERSION),
                                    Some(upd_line),
                                ))
                                .child(if upd_new {
                                    let u = upd_url.unwrap_or_default();
                                    btn("upd-go", "Download & Update", &p, w)
                                        .bg(p.fg)
                                        .text_color(p.desk_base)
                                        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&u)))
                                } else {
                                    btn("upd-now", "Check now", &p, w)
                                        .on_click(cx.listener(|this, _, _, cx| this.check_update(cx)))
                                }),
                            ),
                    )
                    .child(
                        section()
                            .child(h2("Project"))
                            .child(line(lab(
                                "Workspace".into(),
                                Some(self.snap.workspace.display().to_string())
                                    .filter(|s| !s.is_empty()),
                            )))
                            .child(line(lab(
                                "Config".into(),
                                Some(self.snap.config_source.as_ref().map_or(
                                    "built-in defaults".into(),
                                    |p| p.display().to_string(),
                                )),
                            ))),
                    ),
            )
            .into_any_element()
    }
}

fn ticket_color(p: &Pal, s: TicketState) -> Hsla {
    use TicketState::*;
    match s {
        Proposed | InReview | ReadyForHuman | NeedsInfo => p.amber,
        Queued | InProgress => p.accent,
        Done => p.green,
        Failed | Wontfix => p.red,
        _ => p.fg3,
    }
}

fn st_pill(w: Pixels, label: &str, c: Hsla) -> Div {
    div()
        .text_size(px(10.5))
        .px(px(7.))
        .py(px(1.))
        .rounded_full()
        .hair_all(w, c)
        .text_color(c)
        .whitespace_nowrap()
        .child(label.to_string())
}

fn main() -> anyhow::Result<()> {
    let ws = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let harness = Harness::open(ws)?;
    // Scripted runs (bench/) hand the goal over without typing it.
    if let Ok(goal) = std::env::var("BACKSPACE_GOAL") {
        harness.send(goal);
    }
    let fleet = Fleet::new(harness, Prefs::load())?;

    gpui_kit::application()
        .with_assets(icons::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            if let Err(e) = cx.text_system().add_fonts(bundled_fonts()) {
                eprintln!("bundled fonts: {e}");
            }
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(720.), px(480.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Backspace".into()),
                        appears_transparent: GLASS,
                        traffic_light_position: GLASS.then(|| point(px(14.), px(13.))),
                    }),
                    window_background: if GLASS {
                        WindowBackgroundAppearance::Blurred
                    } else {
                        WindowBackgroundAppearance::Opaque
                    },
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| Workbench::new(fleet, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("failed to open window");
            cx.activate(true);
        });
    Ok(())
}
