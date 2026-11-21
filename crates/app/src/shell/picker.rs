//! The model picker (board "Model picker A", 21 Nov, from Charles's four pictures): a rail of
//! coding CLIs on the left, a searchable list of the shown CLI's models in the middle, grouped by
//! the upstream provider (OpenCode alone lists 200+), with Reasoning and Access under it, and a
//! detail panel on the right for the model under the pointer: context, price per 1M, reasoning,
//! Add to favorites. Facts come only from the CLI's own listing (`harnesses::ModelInfo`); a blank
//! is a fact the CLI did not report, never a guess.

use super::subs::{login_line, model_opts, PickFor};
use super::thread::{EFFORTS, TRUST};
use super::*;
use backspace_core::accounts::CliStatus;
use backspace_core::chat::{Route, RouteKind};
use backspace_core::harnesses::ModelInfo;
use gpui_kit::prelude::FluentBuilder;

const RAIL_W: f32 = 44.0;
const LIST_W: f32 = 320.0;
const DETAIL_W: f32 = 228.0;
const PICKER_H: f32 = 440.0;

/// "1M", "262k", "8k".
pub(crate) fn tokens_short(n: u64) -> String {
    if n >= 1_000_000 {
        let m = n as f64 / 1_048_576.0;
        let m = if (m - m.round()).abs() < 0.05 { m.round() } else { (n as f64 / 1_000_000.0 * 10.0).round() / 10.0 };
        format!("{m}M")
    } else {
        format!("{}k", (n as f64 / 1000.0).round() as u64)
    }
}

/// "$3", "$0.15", "$1.4".
pub(crate) fn dollars(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("${s}")
}

/// What a row shows on its right: Free, "$in / $out", or nothing.
pub(crate) fn price_tag(m: Option<&ModelInfo>) -> Option<String> {
    let m = m?;
    if m.free() {
        return Some("Free".into());
    }
    Some(format!("{} / {}", dollars(m.input?), dollars(m.output?)))
}

/// Two letters for a CLI on the rail: brand marks are not ours to draw.
fn rail_mark(provider: &str, name: &str) -> String {
    match provider {
        "claude" => "Cl".into(),
        "codex" => "Cx".into(),
        "cursor" => "Cu".into(),
        "opencode" => "Oc".into(),
        "gemini" => "Ge".into(),
        "grok" => "Gk".into(),
        _ => name.chars().take(2).collect(),
    }
}

/// CLIs whose runner passes the Reasoning level on (`--effort`, `model_reasoning_effort`).
fn takes_effort(provider: &str) -> bool {
    matches!(provider, "claude" | "codex")
}

/// A row's label: the CLI's display name for the model, else its id, without the group's prefix
/// when the list is grouped by provider.
fn row_label(m: &str, info: Option<&ModelInfo>, grouped: bool) -> String {
    if let Some(n) = info.and_then(|i| i.name.clone()) {
        return n;
    }
    if grouped {
        if let Some((_, rest)) = m.split_once('/') {
            return rest.to_string();
        }
    }
    m.to_string()
}

fn fav_key(cli: &str, model: Option<&str>) -> String {
    format!("{cli}:{}", model.unwrap_or(""))
}

/// The model rows one CLI offers, each with what the CLI says about it.
fn rows_of(s: &CliStatus) -> Vec<(Option<String>, Option<&ModelInfo>)> {
    model_opts(s).into_iter().map(|m| {
        let info = m.as_deref().and_then(|id| s.model_info.iter().find(|i| i.id == id));
        (m, info)
    }).collect()
}

/// Does `q` (lower case) match this model? Id, display name, CLI name.
fn matches(q: &str, cli: &CliStatus, m: Option<&str>, info: Option<&ModelInfo>) -> bool {
    let hay = format!("{} {} {}", m.unwrap_or("default"), info.and_then(|i| i.name.as_deref()).unwrap_or(""), cli.name).to_lowercase();
    q.split_whitespace().all(|w| hay.contains(w))
}

impl Cauldron {
    /// Opens the picker on the CLI that answers now, with an empty search.
    pub(super) fn open_picker(&mut self, route: Option<&Route>, window: &mut Window, cx: &mut Context<Self>) {
        self.start.pick_cli = route.filter(|r| r.kind == RouteKind::Cli).map(|r| r.provider.clone());
        self.start.pick_focus = route.map(|r| fav_key(&r.provider, r.model.as_deref()));
        self.start.pick_query.clear();
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search models"));
        let sub = cx.subscribe(&st, |this, st, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                this.start.pick_query = st.read(cx).value().to_string();
                cx.notify();
            }
        });
        st.read(cx).focus_handle(cx).focus(window, cx);
        self.start.pick_search = Some((st, sub));
    }

    fn pick_model(&mut self, to: &PickFor, provider: &str, model: Option<String>, cx: &mut Context<Self>) {
        self.pick_route(to, Route { kind: RouteKind::Cli, provider: provider.to_string(), model });
        self.start.route_menu = None;
        cx.notify();
    }

    pub(super) fn model_picker(&self, to: PickFor, route: Option<&Route>, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let current = route.filter(|r| r.kind == RouteKind::Cli);
        let mut clis: Vec<&CliStatus> = self.start.providers.iter().collect();
        clis.sort_by_key(|s| (!s.installed, !s.ready()));
        let shown = self.start.pick_cli.clone().or_else(|| current.map(|r| r.provider.clone())).or_else(|| clis.first().map(|s| s.provider.clone()));
        let favs = Prefs::load().fav_models;
        let shown_name = shown.as_deref().and_then(|p| clis.iter().find(|s| s.provider == p)).map_or_else(String::new, |s| s.name.clone());
        let query = self.start.pick_query.trim().to_lowercase();

        // ---- rail: one button per CLI
        let mut rail = div().id("pick-rail").w(px(RAIL_W)).flex_none().flex().flex_col().items_center().gap(px(4.)).py(px(8.)).border_r_1().border_color(c(pal.line));
        for (i, s) in clis.iter().enumerate() {
            let on = shown.as_deref() == Some(s.provider.as_str());
            let state = if !s.installed { "Not installed".to_string() } else { s.logins.get(s.active).map_or_else(String::new, login_line) };
            let dot = if !s.installed { None } else if s.ready() { Some(c(pal.ok)) } else { Some(c(pal.warn)) };
            let prov = s.provider.clone();
            let letter = rail_mark(&s.provider, &s.name);
            rail = rail.child(
                div()
                    .id(("route-cli", i))
                    .relative()
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if on { fg } else { fg3 })
                    .when(on, |d| d.bg(c(pal.selected)))
                    .when(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .when(!s.installed, |d| d.opacity(0.45))
                    .aria_label(format!("{} — {state}", s.name))
                    .aria_selected(on)
                    .child(letter)
                    .when_some(dot, |d, dot| d.child(div().absolute().right(px(3.)).bottom(px(3.)).size(px(6.)).rounded_full().bg(dot)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.start.pick_cli = Some(prov.clone());
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        // ---- list
        let search = self.start.pick_search.as_ref().map(|(s, _)| s.clone());
        let by_provider = !self.start.pick_flat;
        let head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(40.))
            .px(px(12.))
            .border_b_1()
            .border_color(c(pal.line))
            .child(glyph("search", 14., fg3))
            .child(div().flex_1().min_w_0().when_some(search, |d, s| d.child(Input::new(&s).appearance(false).text_size(px(13.)).p_0())))
            .child(
                div()
                    .id("pick-by-provider")
                    .h(px(22.))
                    .px(px(7.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .rounded(px(radius::TAG))
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .text_color(if by_provider { fg } else { fg3 })
                    .when(by_provider, |d| d.bg(c(pal.inset)))
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label("By provider")
                    .aria_selected(by_provider)
                    .child(glyph("layers", 12., if by_provider { fg2 } else { fg3 }))
                    .child("By provider")
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.start.pick_flat = !this.start.pick_flat;
                        cx.notify();
                    }))
                    .test_support(),
            );
        let header = |id: (&'static str, usize), text: String| div().id(id).flex().items_center().gap(px(6.)).px(px(8.)).pt(px(8.)).pb(px(3.)).text_size(px(11.5)).text_color(fg3).child(text);
        let mut list = div().id("pick-list").flex_1().min_h_0().overflow_y_scroll().px(px(6.)).pb(px(6.)).flex().flex_col();
        let mut n = 0usize;
        let mut hn = 0usize;
        let mut push_row = |list: Stateful<Div>, s: &CliStatus, m: Option<String>, info: Option<&ModelInfo>, lead: bool, grouped: bool, n: &mut usize| -> Stateful<Div> {
            let sel = current.is_some_and(|r| r.provider == s.provider && r.model == m);
            let key = fav_key(&s.provider, m.as_deref());
            let label = m.as_deref().map_or_else(|| format!("{}'s default", s.name), |m| row_label(m, info, grouped));
            let named = info.is_some_and(|i| i.name.is_some());
            let price = price_tag(info);
            let free = info.is_some_and(ModelInfo::free);
            let (to2, prov, m2, key2) = (to.clone(), s.provider.clone(), m.clone(), key.clone());
            let ready = s.ready();
            let row = div()
                .id(("route-model", *n))
                .h(px(28.))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_color(if sel { fg } else { fg2 })
                .when(sel, |d| d.bg(c(pal.selected)))
                .when(!sel, |d| d.hover(|d| d.bg(c(pal.hover))))
                .when(!ready, |d| d.opacity(0.6))
                .aria_label(label.clone())
                .aria_selected(sel)
                .when(lead, |d| d.child(div().w(px(18.)).flex_none().text_size(px(10.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg3).child(rail_mark(&s.provider, &s.name))))
                .child(div().flex_1().min_w_0().truncate().when(m.is_some() && !named, |d| d.font_family(crate::ui::SANS_MONO).text_size(px(12.))).child(label))
                .when_some(price, |d, p| {
                    d.child(if free {
                        div().flex_none().px(px(6.)).h(px(18.)).flex().items_center().rounded(px(radius::TAG)).bg(ca(pal.ok, 0.14)).text_size(px(11.)).text_color(c(pal.ok)).child(p)
                    } else {
                        div().flex_none().font_family(crate::ui::SANS_MONO).text_size(px(11.)).text_color(fg3).child(p)
                    })
                })
                .when(sel, |d| d.child(glyph("check", 13., fg2)))
                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                    if *h && this.start.pick_focus.as_deref() != Some(key2.as_str()) {
                        this.start.pick_focus = Some(key2.clone());
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    if !ready {
                        let active = this.start.providers.iter().find(|s| s.provider == prov).and_then(|s| s.logins.get(s.active)).and_then(|l| l.id.clone());
                        this.sign_in(&prov, active.as_deref(), cx);
                        return;
                    }
                    this.pick_model(&to2, &prov, m2.clone(), cx);
                }))
                .test_support();
            *n += 1;
            list.child(row)
        };
        if self.start.providers.is_empty() {
            list = list.child(div().id("route-loading").px(px(8.)).py(px(10.)).text_size(px(12.5)).text_color(fg3).aria_label("Looking for coding CLIs…").child("Looking for coding CLIs…").test_support());
        } else if !query.is_empty() {
            // Search runs across every installed CLI, grouped by CLI.
            let mut any = false;
            for s in clis.iter().filter(|s| s.installed) {
                let hits: Vec<_> = rows_of(s).into_iter().filter(|(m, i)| matches(&query, s, m.as_deref(), *i)).collect();
                if hits.is_empty() {
                    continue;
                }
                any = true;
                list = list.child(header(("pick-head", hn), s.name.clone()).test_support());
                hn += 1;
                for (m, i) in hits {
                    list = push_row(list, s, m, i, false, false, &mut n);
                }
            }
            if !any {
                list = list.child(div().id("pick-none").px(px(8.)).py(px(10.)).text_size(px(12.5)).text_color(fg3).aria_label("No model matches").child(format!("No model matches “{}”.", self.start.pick_query.trim())).test_support());
            }
        } else {
            // Favorites first, across CLIs.
            let fav_rows: Vec<(&CliStatus, Option<String>, Option<&ModelInfo>)> = favs
                .iter()
                .filter_map(|k| {
                    let (cli, m) = k.split_once(':')?;
                    let s = clis.iter().find(|s| s.provider == cli && s.installed)?;
                    let m = (!m.is_empty()).then(|| m.to_string());
                    let info = m.as_deref().and_then(|id| s.model_info.iter().find(|i| i.id == id));
                    Some((*s, m, info))
                })
                .collect();
            if !fav_rows.is_empty() {
                list = list.child(header(("pick-head", hn), "Favorites".into()).child(glyph("star", 11., fg3)).test_support());
                hn += 1;
                for (s, m, i) in fav_rows {
                    list = push_row(list, s, m, i, true, false, &mut n);
                }
            }
            if let Some(s) = shown.as_deref().and_then(|p| clis.iter().find(|s| s.provider == p)) {
                let state = if !s.installed { "Not installed".to_string() } else { s.logins.get(s.active).map_or_else(String::new, login_line) };
                list = list.child(header(("pick-head", hn), format!("{} · {state}", s.name)).test_support());
                hn += 1;
                if !s.installed {
                    list = list.child(div().id("pick-install").px(px(8.)).py(px(6.)).text_size(px(12.)).text_color(fg3).font_family(crate::ui::SANS_MONO).aria_label(format!("Install: {}", s.install)).child(s.install.clone()).test_support());
                } else {
                    let rows = rows_of(s);
                    if by_provider && rows.iter().any(|(m, _)| m.as_deref().is_some_and(|m| m.contains('/'))) {
                        // The CLI's default first, then one group per upstream provider.
                        let mut groups: Vec<(String, Vec<(Option<String>, Option<&ModelInfo>)>)> = Vec::new();
                        for (m, i) in rows {
                            let up = m.as_deref().and_then(|m| m.split_once('/')).map_or(String::new(), |(p, _)| p.to_string());
                            match groups.iter_mut().find(|(g, _)| *g == up) {
                                Some((_, v)) => v.push((m, i)),
                                None => groups.push((up, vec![(m, i)])),
                            }
                        }
                        for (g, v) in groups {
                            if !g.is_empty() {
                                list = list.child(header(("pick-head", hn), format!("{g} · {}", v.len())).test_support());
                                hn += 1;
                            }
                            for (m, i) in v {
                                list = push_row(list, s, m, i, false, !g.is_empty(), &mut n);
                            }
                        }
                    } else {
                        for (m, i) in rows {
                            list = push_row(list, s, m, i, false, false, &mut n);
                        }
                    }
                }
            }
        }

        // ---- footer: Reasoning and Access, then Manage
        let p = Prefs::load();
        let seg = |id: &'static str, opts: Vec<(String, bool, Box<dyn Fn(&mut Prefs)>)>, cx: &mut Context<Self>| {
            let mut row = div().id(id).flex_1().flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).bg(c(pal.inset));
            for (k, (label, on, set)) in opts.into_iter().enumerate() {
                let set = std::rc::Rc::new(set);
                row = row.child(
                    div()
                        .id((id, k))
                        .flex_1()
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::TAG))
                        .cursor_pointer()
                        .text_size(px(11.5))
                        .whitespace_nowrap()
                        .text_color(if on { fg } else { fg3 })
                        .when(on, |d| d.bg(c(pal.raised)).shadow(vec![drop(tokens::shadow::TIP)]))
                        .when(!on, |d| d.hover(|d| d.text_color(fg2)))
                        .aria_selected(on)
                        .aria_label(label.clone())
                        .child(label)
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            let mut p = Prefs::load();
                            set(&mut p);
                            p.save();
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            row
        };
        let cap = |v: &str| {
            let mut ch = v.chars();
            ch.next().map(|f| format!("{}{}", f.to_uppercase(), ch.as_str())).unwrap_or_default()
        };
        let effort_opts = EFFORTS
            .iter()
            .map(|e| {
                let val = e.map(str::to_string);
                (e.map_or_else(|| "Auto".to_string(), cap), p.cli_effort.as_deref() == *e, Box::new(move |p: &mut Prefs| p.cli_effort = val.clone()) as Box<dyn Fn(&mut Prefs)>)
            })
            .collect();
        let trust_opts = TRUST
            .iter()
            .map(|(k, l)| {
                let k = k.to_string();
                let on = p.cli_permission == k;
                (l.to_string(), on, Box::new(move |p: &mut Prefs| p.cli_permission = k.clone()) as Box<dyn Fn(&mut Prefs)>)
            })
            .collect();
        let label = |t: &str| div().w(px(62.)).flex_none().text_size(px(12.)).text_color(fg3).child(t.to_string());
        let footer = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(10.))
            .py(px(8.))
            .border_t_1()
            .border_color(c(pal.line))
            .child(if shown.as_deref().is_none_or(takes_effort) {
                div().flex().items_center().gap(px(8.)).child(label("Reasoning")).child(seg("pick-effort", effort_opts, cx))
            } else {
                div().flex().items_center().gap(px(8.)).h(px(26.)).child(label("Reasoning")).child(div().text_size(px(11.5)).text_color(fg3).child(format!("Set per model in {}'s own config", shown_name.clone())))
            })
            .child(div().flex().items_center().gap(px(8.)).child(label("Access")).child(seg("pick-trust", trust_opts, cx)))
            .child(
                div()
                    .id("route-manage")
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(2.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(fg3)
                    .hover(|d| d.text_color(fg2))
                    .aria_label("Manage subscriptions")
                    .child(glyph("settings", 12., fg3))
                    .child("Manage subscriptions…")
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.start.route_menu = None;
                        this.settings = Some(0);
                        cx.notify();
                    }))
                    .test_support(),
            );
        let middle = div().w(px(LIST_W)).flex_none().flex().flex_col().min_h_0().child(head).child(list.test_support()).child(footer);

        // ---- detail panel for the model under the pointer
        let focus = self.start.pick_focus.clone().and_then(|k| {
            let (cli, m) = k.split_once(':').map(|(a, b)| (a.to_string(), b.to_string()))?;
            let s = clis.iter().find(|s| s.provider == cli)?;
            let m = (!m.is_empty()).then_some(m);
            let info = m.as_deref().and_then(|id| s.model_info.iter().find(|i| i.id == id)).cloned();
            Some((*s, m, info, k))
        });
        let detail = focus.map(|(s, m, info, key)| {
            let title = info.as_ref().and_then(|i| i.name.clone()).or_else(|| m.clone()).unwrap_or_else(|| format!("{}'s default", s.name));
            let via = match m.as_deref().and_then(|m| m.split_once('/')) {
                Some((up, _)) => format!("via {} · {up}", s.name),
                None => format!("via {}", s.name),
            };
            let fact = |k: &str, v: Option<String>| {
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(12.))
                    .child(div().text_color(fg3).child(k.to_string()))
                    .child(div().font_family(crate::ui::SANS_MONO).text_color(if v.is_some() { fg } else { fg3 }).child(v.unwrap_or_else(|| "—".into())))
            };
            let any = info.as_ref().is_some_and(|i| i.context.is_some() || i.input.is_some());
            let fav = favs.contains(&key);
            let key2 = key.clone();
            div()
                .id("pick-detail")
                .w(px(DETAIL_W))
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(12.))
                .border_l_1()
                .border_color(c(pal.line))
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(title))
                .when_some(m.clone().filter(|_| info.as_ref().is_some_and(|i| i.name.is_some())), |d, m| d.child(div().font_family(crate::ui::SANS_MONO).text_size(px(11.)).text_color(fg3).child(m)))
                .child(div().text_size(px(12.)).text_color(fg3).mb(px(4.)).child(via))
                .child(fact("Context", info.as_ref().and_then(|i| i.context).map(tokens_short)))
                .child(fact("Input per 1M", info.as_ref().and_then(|i| i.input).map(dollars)))
                .child(fact("Output per 1M", info.as_ref().and_then(|i| i.output).map(dollars)))
                .child(fact("Reasoning", info.as_ref().map(|i| if i.variants.is_empty() { if i.reasoning { "yes".into() } else { "no".into() } } else { i.variants.join(" · ") })))
                .when(!any, |d| d.child(div().mt(px(2.)).text_size(px(11.)).line_height(px(15.)).text_color(fg3).child(format!("{} does not report price or context for this one.", s.name))))
                .child(div().flex_1())
                .child(
                    div()
                        .id("pick-fav")
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(6.))
                        .rounded(px(radius::CONTROL))
                        .bg(c(pal.inset))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .text_size(px(12.5))
                        .text_color(fg2)
                        .aria_label(if fav { "Remove from favorites" } else { "Add to favorites" })
                        .child(glyph(if fav { "star-off" } else { "star" }, 13., fg2))
                        .child(if fav { "Remove from favorites" } else { "Add to favorites" })
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            let mut p = Prefs::load();
                            if let Some(at) = p.fav_models.iter().position(|k| *k == key2) {
                                p.fav_models.remove(at);
                            } else {
                                p.fav_models.push(key2.clone());
                            }
                            p.save();
                            cx.notify();
                        }))
                        .test_support(),
                )
                .test_support()
        });

        div()
            .id("route-menu")
            .h(px(PICKER_H))
            .flex()
            .rounded(px(radius::PANEL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow(vec![drop(tokens::shadow::MENU)])
            .text_size(px(12.5))
            .overflow_hidden()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.start.route_menu = None;
                cx.notify();
            }))
            .child(rail.test_support())
            .child(middle)
            .when_some(detail, |d, x| d.child(x))
            .test_support()
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{dollars, price_tag, tokens_short, ModelInfo};

    #[test]
    fn facts_read_short() {
        assert_eq!(tokens_short(1_048_576), "1M");
        assert_eq!(tokens_short(262_144), "262k");
        assert_eq!(dollars(3.0), "$3");
        assert_eq!(dollars(0.15), "$0.15");
        assert_eq!(dollars(1.4), "$1.4");
        let m = ModelInfo { id: "a/b".into(), input: Some(0.0), output: Some(0.0), ..Default::default() };
        assert_eq!(price_tag(Some(&m)).as_deref(), Some("Free"));
        let m = ModelInfo { id: "a/b".into(), input: Some(3.0), output: Some(15.0), ..Default::default() };
        assert_eq!(price_tag(Some(&m)).as_deref(), Some("$3 / $15"));
        assert_eq!(price_tag(Some(&ModelInfo::default())), None, "unknown price shows nothing");
    }
}
