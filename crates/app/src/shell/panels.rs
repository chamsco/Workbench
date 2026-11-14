//! Resizable panels that learn your width (the coordinator's panels spec, Charles chose A, the
//! learned peak; model in `cauldron_chrome::snap`). The sidebar's right edge and the drawer's left
//! edge are handles: drag to resize, and near the width you keep choosing it snaps there ("your
//! usual"); Alt or Shift held while dragging turns the magnet off ("exact"). Double-click returns
//! to your usual width (or the default) without recording. A handle takes focus: arrows move it
//! 8px, Alt+arrow 1px, Home and End go to the ends, recorded after a 700ms pause.
//!
//! Each panel keeps its own history, in `panels.json` in the data folder (core's prefs are not
//! touched). Only a width the user chose is recorded: one the window forced (too narrow for the
//! card) is not, and opening a panel records nothing. Settings > Appearance > Panels forgets them.

use super::*;
use cauldron_chrome::layout::Widths;
use cauldron_chrome::snap::{self, Range};
use std::path::PathBuf;

/// The panels that resize.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Panel {
    Sidebar,
    /// The drawer beside a started stream.
    Drawer,
    /// Agents' own drawer.
    AgentsDrawer,
}

impl Panel {
    pub(crate) const ALL: [Panel; 3] = [Panel::Sidebar, Panel::Drawer, Panel::AgentsDrawer];

    /// The spec's ranges; the defaults are the widths the app had (the sidebar 236; the stream
    /// drawer 340, its range starting at 280; Agents' drawer 200).
    pub(crate) fn range(self) -> Range {
        match self {
            Panel::Sidebar => Range { min: 200.0, max: 360.0, default: metric::SIDEBAR_W },
            Panel::Drawer => Range { min: 280.0, max: 560.0, default: 340.0 },
            Panel::AgentsDrawer => Range { min: 168.0, max: 280.0, default: metric::DRAWER_W },
        }
    }

    /// Which way a drag to the right moves the edge: the sidebar grows, a drawer shrinks.
    fn dir(self) -> f32 {
        if self == Panel::Sidebar {
            1.0
        } else {
            -1.0
        }
    }

    fn key(self) -> &'static str {
        match self {
            Panel::Sidebar => "sidebar",
            Panel::Drawer => "drawer",
            Panel::AgentsDrawer => "agents-drawer",
        }
    }
}

/// One panel's history and width.
#[derive(Clone, Default, Debug)]
pub(crate) struct Learnt {
    pub hist: Vec<f32>,
    /// The width it is at, when the user set one.
    pub width: Option<f32>,
}

impl Learnt {
    fn from_json(v: &serde_json::Value) -> Learnt {
        let hist = v.get("hist").and_then(|h| h.as_array()).map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default();
        Learnt { hist, width: v.get("width").and_then(|w| w.as_f64()).map(|w| w as f32) }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "hist": self.hist, "width": self.width })
    }
}

/// Every panel's history and width, saved as one small file.
#[derive(Default, Debug)]
pub(crate) struct Panels {
    pub by: std::collections::HashMap<&'static str, Learnt>,
    pub file: Option<PathBuf>,
}

impl Panels {
    pub(crate) fn load(file: Option<PathBuf>) -> Panels {
        let path = file.clone().unwrap_or_else(|| Prefs::data_dir().join("panels.json"));
        // Read field by field: a file of another shape gives what it can, never an error.
        let saved: serde_json::Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut by = std::collections::HashMap::new();
        for p in Panel::ALL {
            if let Some(v) = saved.get(p.key()) {
                by.insert(p.key(), Learnt::from_json(v));
            }
        }
        Panels { by, file }
    }

    fn save(&self) {
        let path = self.file.clone().unwrap_or_else(|| Prefs::data_dir().join("panels.json"));
        let out: serde_json::Map<String, serde_json::Value> = self.by.iter().map(|(k, v)| (k.to_string(), v.to_json())).collect();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, serde_json::Value::Object(out).to_string());
    }

    pub(crate) fn hist(&self, p: Panel) -> &[f32] {
        self.by.get(p.key()).map_or(&[], |l| &l.hist)
    }

    /// The panel's width: the one set, else the learned one, else the default.
    pub(crate) fn width(&self, p: Panel) -> f32 {
        let r = p.range();
        match self.by.get(p.key()) {
            Some(Learnt { width: Some(w), .. }) => r.clamp(*w),
            _ => snap::usual(self.hist(p), r),
        }
    }

    pub(crate) fn set(&mut self, p: Panel, w: f32) {
        self.by.entry(p.key()).or_default().width = Some(p.range().clamp(w));
    }

    /// A settled width the user chose: kept, and saved.
    pub(crate) fn record(&mut self, p: Panel, w: f32) {
        let l = self.by.entry(p.key()).or_default();
        snap::record(&mut l.hist, w);
        l.width = Some(p.range().clamp(w));
        self.save();
    }

    /// Settings > Appearance > Panels: forget every learned width.
    pub(crate) fn reset(&mut self) {
        self.by.clear();
        self.save();
    }
}

/// A drag in progress.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PanelDrag {
    pub panel: Panel,
    pub x0: f32,
    pub w0: f32,
    /// The window held the width back (the card needs its room): not recorded.
    pub forced: bool,
    pub locked: bool,
    pub free: bool,
}

impl Cauldron {
    /// The widths the layout uses now.
    pub(super) fn widths(&self) -> Widths {
        let drawer = if self.page == pages::AppPage::Agents { Panel::AgentsDrawer } else { Panel::Drawer };
        Widths { sidebar: self.panels.width(Panel::Sidebar), drawer: self.panels.width(drawer) }
    }

    fn drawer_panel(&self) -> Panel {
        if self.page == pages::AppPage::Agents {
            Panel::AgentsDrawer
        } else {
            Panel::Drawer
        }
    }

    pub(super) fn start_panel_drag(&mut self, panel: Panel, x: f32) {
        self.panel_drag = Some(PanelDrag { panel, x0: x, w0: self.panels.width(panel), forced: false, locked: false, free: false });
    }

    /// The handle followed to `x`, through the magnet unless `free`; held back where the window
    /// cannot fit it.
    pub(super) fn drag_panel_to(&mut self, x: f32, free: bool, win: (f32, f32)) {
        let Some(mut d) = self.panel_drag else { return };
        let r = d.panel.range();
        let raw = r.clamp(d.w0 + d.panel.dir() * (x - d.x0));
        let l = snap::learn(self.panels.hist(d.panel), r);
        let land = snap::land(raw, &l, free);
        let mut w = r.clamp(land.width);
        // The card keeps its room: past that, the width stops and is not the user's choice.
        let fits = |this: &Self, w: f32| {
            let mut ws = this.widths();
            if d.panel == Panel::Sidebar {
                ws.sidebar = w;
            } else {
                ws.drawer = w;
            }
            let want = ShellState { sidebar_open: this.sidebar_open, drawer_open: this.drawer.is_some(), split_open: false, terminal_open: this.terminal, plugins: this.has_rail() };
            layout::layout_with(win.0, win.1, want, this.plat, ws).main.w >= metric::MAIN_MIN_W
        };
        d.forced = false;
        while w > r.min && !fits(self, w) {
            w -= 1.0;
            d.forced = true;
        }
        d.locked = land.locked && !d.forced;
        d.free = free;
        self.panels.set(d.panel, w);
        self.panel_drag = Some(d);
    }

    pub(super) fn end_panel_drag(&mut self) {
        if let Some(d) = self.panel_drag.take() {
            if !d.forced {
                let w = self.panels.width(d.panel);
                self.panels.record(d.panel, w);
            }
        }
    }

    /// Double-click: back to your usual width (or the default), not recorded.
    pub(super) fn panel_usual(&mut self, panel: Panel) {
        let w = snap::usual(self.panels.hist(panel), panel.range());
        self.panels.set(panel, w);
    }

    /// Arrows, Alt+arrows, Home and End on a focused handle; recorded after 700ms of quiet.
    fn panel_key(&mut self, panel: Panel, ev: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let r = panel.range();
        let w = self.panels.width(panel);
        let step = if ev.keystroke.modifiers.alt { 1.0 } else { 8.0 };
        let next = match ev.keystroke.key.as_str() {
            "right" => w + step * panel.dir(),
            "left" => w - step * panel.dir(),
            "home" => r.min,
            "end" => r.max,
            _ => return false,
        };
        self.panels.set(panel, next);
        self.panel_key_gen += 1;
        let gen = self.panel_key_gen;
        self.panel_key_task = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            cx.background_executor().timer(std::time::Duration::from_millis(700)).await;
            let _ = this.update(cx, |this, _| {
                if this.panel_key_gen == gen {
                    let w = this.panels.width(panel);
                    this.panels.record(panel, w);
                }
            });
        }));
        cx.notify();
        true
    }

    /// The handles on the sidebar's right edge and the drawer's left edge: a line that lights on
    /// hover, accent while dragging, with the width (and "your usual" or "exact") beside it.
    pub(super) fn panel_handles(&mut self, s: &Frame, pal: &'static Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out = Vec::new();
        let edges = [(Panel::Sidebar, s.sidebar.map(|p| (p.rect.right() - SIDE_PR + 3.0, p.rect.y, p.rect.h))), (self.drawer_panel(), s.drawer.map(|p| (p.rect.x - 3.0, p.rect.y, p.rect.h)))];
        for (panel, at) in edges {
            let Some((x, y, h)) = at else { continue };
            let focus = self.panel_focus.entry(panel).or_insert_with(|| cx.focus_handle()).clone();
            let dragging = self.panel_drag.filter(|d| d.panel == panel);
            let id: &'static str = if panel == Panel::Sidebar { "handle-sidebar" } else { "handle-drawer" };
            let line = if dragging.is_some() { c(pal.accent) } else { c(pal.line_strong) };
            let w = self.panels.width(panel);
            let el = div()
                .id(id)
                .w(px(8.))
                .h(px(h))
                .cursor_col_resize()
                .track_focus(&focus)
                .group(id)
                .aria_label(format!("Resize, {} px", w.round()))
                .child(
                    div()
                        .absolute()
                        .left(px(3.))
                        .top(px(8.))
                        .w(px(2.))
                        .h(px((h - 16.0).max(0.0)))
                        .rounded(px(1.))
                        .bg(line)
                        .when_live(dragging.is_none(), |d| d.opacity(0.0).group_hover(id, |d| d.opacity(1.0))),
                )
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    if ev.click_count >= 2 {
                        this.panel_usual(panel);
                    } else {
                        this.start_panel_drag(panel, f32::from(ev.position.x));
                    }
                    if let Some(f) = this.panel_focus.get(&panel) {
                        f.focus(window, cx);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
                .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _, cx| {
                    if this.panel_key(panel, ev, cx) {
                        cx.stop_propagation();
                    }
                }));
            let el = self.tipped((id, 1usize), el, "Drag to resize. Hold Alt or Shift for exact. Double-click for your usual width.", if panel == Panel::Sidebar { Side::Right } else { Side::Left }, pal, cx);
            // The tip's wrapper makes its element relative, so an absolute slot places it.
            out.push(div().absolute().left(px(x - 4.0)).top(px(y)).w(px(8.)).h(px(h)).child(el.test_support()).into_any_element());
            if let Some(d) = dragging {
                let label = format!("{} px{}", w.round(), if d.locked { " · your usual" } else if d.free { " · exact" } else { "" });
                let lx = if panel == Panel::Sidebar { x + 8.0 } else { x - 8.0 - 150.0 };
                out.push(
                    div()
                        .id("panel-width")
                        .absolute()
                        .left(px(lx))
                        .top(px(y + 12.0))
                        .w(px(150.))
                        .flex()
                        .when_live(panel != Panel::Sidebar, |d| d.justify_end())
                        .child(div().h(px(22.)).px(px(8.)).flex().items_center().rounded(px(radius::CONTROL)).bg(c(pal.raised)).border_1().border_color(c(pal.accent)).text_size(px(11.5)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg)).child(label.clone()))
                        .aria_label(label)
                        .test_support()
                        .into_any_element(),
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{Panel, Panels};
    use gpui_kit::AppContext;

    /// A drag lands on the learned width when near it, exact with Alt or Shift; release records
    /// it; a width the window forced is not recorded; double-click goes back without recording.
    #[gpui_kit::test]
    fn dragging_learns_and_snaps(cx: &mut gpui_kit::TestAppContext) {
        let (handle, _chats) = super::super::thread::tests::open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            use gpui_kit::test::TestWindowExt;
            window.render_frame(cx);
            let (h, side) = (super::super::tests::rect(window, "handle-sidebar"), super::super::tests::rect(window, "sidebar"));
            assert!((h.x + h.w / 2.0 - (side.right() - 6.0)).abs() <= 1.0 && h.h > 100.0, "the handle on the sidebar's edge: {h:?} {side:?}");
        })
        .unwrap();
        let file = std::env::temp_dir().join(format!("cauldron-panels-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&file);
        handle
            .update(cx, |c, _, _| {
                c.panels = Panels::load(Some(file.clone()));
                let win = (1240.0, 702.0);
                // Teach it 280, five times.
                for _ in 0..5 {
                    c.start_panel_drag(Panel::Sidebar, 100.0);
                    let w0 = c.panels.width(Panel::Sidebar);
                    c.drag_panel_to(100.0 + (280.0 - w0), true, win);
                    c.end_panel_drag();
                }
                assert_eq!(c.panels.hist(Panel::Sidebar), [280.0; 5]);
                // Near 280 it snaps; with Alt it does not.
                c.panels.set(Panel::Sidebar, 236.0);
                c.start_panel_drag(Panel::Sidebar, 100.0);
                c.drag_panel_to(100.0 + 47.0, false, win);
                assert_eq!(c.panels.width(Panel::Sidebar), 280.0, "283 snaps to your usual 280");
                assert!(c.panel_drag.unwrap().locked);
                c.drag_panel_to(100.0 + 47.0, true, win);
                assert_eq!(c.panels.width(Panel::Sidebar), 283.0, "exact with Alt or Shift");
                c.end_panel_drag();
                assert_eq!(c.panels.hist(Panel::Sidebar).last(), Some(&283.0));
                // The window too narrow for the card: held back, and not recorded.
                let n = c.panels.hist(Panel::Sidebar).len();
                c.start_panel_drag(Panel::Sidebar, 100.0);
                c.drag_panel_to(100.0 + 200.0, true, (700.0, 600.0));
                assert!(c.panel_drag.unwrap().forced);
                c.end_panel_drag();
                assert_eq!(c.panels.hist(Panel::Sidebar).len(), n, "a forced width is not recorded");
                // Double-click: back to the usual, nothing recorded.
                c.panel_usual(Panel::Sidebar);
                assert_eq!(c.panels.hist(Panel::Sidebar).len(), n);
            })
            .unwrap();
        // Saved, and read back.
        let back = Panels::load(Some(file.clone()));
        assert_eq!(back.hist(Panel::Sidebar).len(), 6);
        let mut back = back;
        back.reset();
        assert!(Panels::load(Some(file.clone())).hist(Panel::Sidebar).is_empty(), "reset forgets");
        let _ = std::fs::remove_file(file);
    }
}
