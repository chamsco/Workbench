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
const DETAIL_W: f32 = 228.0;
pub(crate) const PICKER_H: f32 = 460.0;

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

/// CLIs whose runner passes the Reasoning level on (`--effort`, `model_reasoning_effort`, OpenCode's variant).
fn takes_effort(provider: &str) -> bool {
    matches!(provider, "claude" | "codex" | "opencode")
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

    /// The picker in a card of its own: in a stream it sits on top of the chat box (Charles,
    /// 21 Nov: "a top rail on top ... on the chat box").
    pub(super) fn model_picker(&self, to: PickFor, route: Option<&Route>, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
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
            .child(self.picker_body(to, route, true, pal, cx))
            .test_support()
            .into_any_element()
    }

    /// Picker v2 (Charles, 21 Nov 00:43Z, a mix of boards A and B): a rail with a star for
    /// favourites then every CLI; one list that scrolls through every CLI's section (the rail
    /// jumps to one); a model family drops down to its versions; search with sort and filters;
    /// Reasoning and Access under it; a detail panel when `detail` and a model is under the pointer.
    #[inline(never)]
    pub(super) fn picker_body(&self, to: PickFor, route: Option<&Route>, detail: bool, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg2, fg3) = (c(pal.fg2), c(pal.fg3));
        let mut clis: Vec<&CliStatus> = self.start.providers.iter().collect();
        clis.sort_by_key(|s| (!s.installed, !s.ready()));
        let favs = Prefs::load().fav_models;
        let (list, sections) = self.picker_list(&to, route, &clis, &favs, pal, cx);

        let rail = self.picker_rail(&clis, &sections, pal, cx);
        let head = self.picker_head(pal, cx);
        // ---- footer: Reasoning and Access, then Manage
        let footer = self.picker_footer(route, pal, cx);
        let middle = div().flex_1().min_w_0().flex().flex_col().min_h_0().child(head).child(list.test_support()).child(footer);

        let detail_el = if detail { self.picker_detail(&clis, &favs, pal, cx) } else { None };
        div().size_full().flex().min_h_0().child(rail.test_support()).child(middle).when_some(detail_el, |d, x| d.child(x))
    }

    #[inline(never)]
    fn picker_rail(&self, clis: &[&CliStatus], sections: &[(String, usize)], pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let _ = (fg, fg2, fg3);
        // ---- rail: the star, then one button per CLI; each jumps to its section
        let rail_btn = |id: (&'static str, usize), on: bool, label: String| {
            div()
                .id(id)
                .relative()
                .size(px(30.))
                .flex_none()
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
                .aria_label(label)
                .aria_selected(on)
        };
        let shown = self.start.pick_cli.clone();
        let mut rail = div().id("pick-rail").w(px(RAIL_W)).flex_none().flex().flex_col().items_center().gap(px(4.)).py(px(8.)).border_r_1().border_color(c(pal.line));
        let fav_ix = sections.iter().find(|(k, _)| k == "★").map(|(_, i)| *i);
        rail = rail.child(
            rail_btn(("pick-rail-fav", 0), shown.as_deref() == Some("★"), "Favorites".into())
                .child(glyph("star", 14., if shown.as_deref() == Some("★") { fg } else { fg3 }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.start.pick_cli = Some("★".into());
                    this.start.pick_scroll.scroll_to_top_of_item(fav_ix.unwrap_or(0));
                    cx.notify();
                }))
                .test_support(),
        );
        rail = rail.child(div().w(px(20.)).h(px(1.)).my(px(2.)).bg(c(pal.line)));
        for (i, s) in clis.iter().enumerate() {
            let on = shown.as_deref() == Some(s.provider.as_str());
            let state = if !s.installed { "Not installed".to_string() } else { s.logins.get(s.active).map_or_else(String::new, login_line) };
            let dot = if !s.installed { None } else if s.ready() { Some(c(pal.ok)) } else { Some(c(pal.warn)) };
            let prov = s.provider.clone();
            let ix = sections.iter().find(|(k, _)| *k == s.provider).map(|(_, i)| *i);
            rail = rail.child(
                rail_btn(("route-cli", i), on, format!("{} — {state}", s.name))
                    .when(!s.installed, |d| d.opacity(0.45))
                    .child(rail_mark(&s.provider, &s.name))
                    .when_some(dot, |d, dot| d.child(div().absolute().right(px(3.)).bottom(px(3.)).size(px(6.)).rounded_full().bg(dot)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.start.pick_cli = Some(prov.clone());
                        if let Some(ix) = ix {
                            this.start.pick_scroll.scroll_to_top_of_item(ix);
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        rail
    }

    #[inline(never)]
    fn picker_head(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let _ = (fg, fg2, fg3);
        // ---- search, sort and filters
        let search = self.start.pick_search.as_ref().map(|(s, _)| s.clone());
        let chip = |id: (&'static str, usize), on: bool, label: &'static str| {
            div()
                .id(id)
                .h(px(22.))
                .px(px(8.))
                .flex()
                .items_center()
                .flex_none()
                .rounded(px(radius::TAG))
                .cursor_pointer()
                .text_size(px(11.5))
                .text_color(if on { fg } else { fg3 })
                .when(on, |d| d.bg(c(pal.selected)))
                .when(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                .aria_label(label)
                .aria_selected(on)
                .child(label)
        };
        let mut chips = div().flex().items_center().gap(px(4.)).px(px(10.)).pb(px(8.)).child(div().text_size(px(11.5)).text_color(fg3).mr(px(2.)).child("By"));
        for (k, label) in ["Provider", "Price", "Name"].into_iter().enumerate() {
            chips = chips.child(
                chip(("pick-sort", k), self.start.pick_sort as usize == k, label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.start.pick_sort = k as u8;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        chips = chips.child(div().w(px(1.)).h(px(14.)).mx(px(4.)).bg(c(pal.line)));
        chips = chips
            .child(
                chip(("pick-filter", 0), self.start.pick_reason, "Reasoning")
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.start.pick_reason = !this.start.pick_reason;
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(
                chip(("pick-filter", 1), self.start.pick_free, "Free")
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.start.pick_free = !this.start.pick_free;
                        cx.notify();
                    }))
                    .test_support(),
            );
        let head = div()
            .flex_none()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(c(pal.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(38.))
                    .px(px(12.))
                    .child(glyph("search", 14., fg3))
                    .child(div().flex_1().min_w_0().when_some(search, |d, s| d.child(Input::new(&s).appearance(false).text_size(px(13.)).p_0()))),
            )
            .child(chips);

        head
    }

    #[inline(never)]
    fn picker_detail(&self, clis: &[&CliStatus], favs: &[String], pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let _ = (fg, fg2, fg3);
        // ---- detail panel for the model under the pointer
        let focus = self.start.pick_focus.clone().and_then(|k| {
            let (cli, m) = k.split_once(':').map(|(a, b)| (a.to_string(), b.to_string()))?;
            let s = clis.iter().find(|s| s.provider == cli)?;
            let m = (!m.is_empty()).then_some(m);
            let info = m.as_deref().and_then(|id| s.model_info.iter().find(|i| i.id == id)).cloned();
            Some((*s, m, info, k))
        });
        let detail_el = focus.map(|(s, m, info, key)| {
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
                .child(fact("Reasoning", info.as_ref().map(|i| match i.variants.as_slice() {
                    [] => if i.reasoning { "yes".into() } else { "no".into() },
                    [one] => one.clone(),
                    [first, .., last] => format!("{first} – {last}"),
                })))
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
        detail_el.map(IntoElement::into_any_element)
    }

    /// Reasoning and Access as segmented rows, then Manage subscriptions.
    #[inline(never)]
    fn picker_footer(&self, route: Option<&Route>, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
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
        let cur = route.filter(|r| r.kind == RouteKind::Cli).map(|r| r.provider.clone());
        let cur_name = cur.as_deref().and_then(|p| self.start.providers.iter().find(|s| s.provider == p)).map_or_else(String::new, |s| s.name.clone());
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(10.))
            .py(px(8.))
            .border_t_1()
            .border_color(c(pal.line))
            .child(if cur.as_deref().is_none_or(takes_effort) {
                div().flex().items_center().gap(px(8.)).child(label("Reasoning")).child(seg("pick-effort", effort_opts, cx))
            } else {
                div().flex().items_center().gap(px(8.)).h(px(26.)).child(label("Reasoning")).child(div().text_size(px(11.5)).text_color(fg3).child(format!("Set per model in {cur_name}'s own config")))
            })
            .child(
                div().flex().items_center().gap(px(8.)).child(label("Access")).child(seg("pick-trust", trust_opts, cx)).child(
                    div()
                        .id("route-manage")
                        .size(px(26.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::CONTROL))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label("Manage subscriptions")
                        .child(glyph("settings", 13., fg3))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.start.route_menu = None;
                            this.settings = Some(0);
                            cx.notify();
                        }))
                        .test_support(),
                ),
            )
    }

    /// The scrolling list: Favorites, then every CLI's section, its models folded into families.
    /// Returns the list and where each section starts (its child index, for the rail's jumps).
    #[inline(never)]
    fn picker_list(&self, to: &PickFor, route: Option<&Route>, clis: &[&CliStatus], favs: &[String], pal: &'static Palette, cx: &mut Context<Self>) -> (Stateful<Div>, Vec<(String, usize)>) {
        let fg3 = c(pal.fg3);
        let current = route.filter(|r| r.kind == RouteKind::Cli);
        let query = self.start.pick_query.trim().to_lowercase();
        let (want_reason, want_free) = (self.start.pick_reason, self.start.pick_free);
        let filtering = !query.is_empty() || want_reason || want_free;
        let keep = |s: &CliStatus, m: Option<&str>, i: Option<&ModelInfo>| {
            (query.is_empty() || matches(&query, s, m, i))
                && (!want_reason || i.is_some_and(|i| i.reasoning || !i.variants.is_empty()))
                && (!want_free || i.is_some_and(ModelInfo::free))
                && !(filtering && m.is_none())
        };
        let mut list = div().id("pick-list").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.start.pick_scroll).px(px(6.)).pb(px(6.)).flex().flex_col();
        let mut sections: Vec<(String, usize)> = Vec::new();
        let mut kids = 0usize;
        let (mut n, mut hn) = (0usize, 0usize);
        let header = |id: (&'static str, usize), text: String, big: bool| {
            div()
                .id(id)
                .flex_none()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .pt(px(if big { 10. } else { 6. }))
                .pb(px(3.))
                .text_size(px(if big { 12. } else { 11. }))
                .font_weight(if big { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                .text_color(if big { c(pal.fg2) } else { fg3 })
                .child(text)
        };
        if self.start.providers.is_empty() {
            list = list.child(div().id("route-loading").px(px(8.)).py(px(10.)).text_size(px(12.5)).text_color(fg3).aria_label("Looking for coding CLIs…").child("Looking for coding CLIs…").test_support());
            return (list, sections);
        }
        // Favorites.
        let fav_rows: Vec<(&CliStatus, Option<String>, Option<&ModelInfo>)> = favs
            .iter()
            .filter_map(|k| {
                let (cli, m) = k.split_once(':')?;
                let s = clis.iter().find(|s| s.provider == cli && s.installed)?;
                let m = (!m.is_empty()).then(|| m.to_string());
                let info = m.as_deref().and_then(|id| s.model_info.iter().find(|i| i.id == id));
                keep(s, m.as_deref(), info).then_some((*s, m, info))
            })
            .collect();
        sections.push(("★".into(), kids));
        list = list.child(header(("pick-head", hn), "Favorites".into(), true).child(glyph("star", 11., fg3)).test_support());
        hn += 1;
        kids += 1;
        if fav_rows.is_empty() {
            list = list.child(div().id("pick-fav-empty").px(px(8.)).pb(px(6.)).text_size(px(11.5)).text_color(fg3).child("Star a model to keep it here.").test_support());
            kids += 1;
        }
        for (s, m, i) in fav_rows {
            let label = format!("{} · {}", rail_mark(&s.provider, &s.name), m.as_deref().map_or_else(|| "default".to_string(), |m| row_label(m, i, false)));
            list = list.child(self.pick_row(n, to, current, s, m, i, label, 0.0, pal, cx));
            n += 1;
            kids += 1;
        }
        let mut any = false;
        for s in clis {
            sections.push((s.provider.clone(), kids));
            let state = if !s.installed { "Not installed".to_string() } else { s.logins.get(s.active).map_or_else(String::new, login_line) };
            list = list.child(header(("pick-head", hn), s.name.clone(), true).child(div().font_weight(FontWeight::NORMAL).text_color(fg3).child(format!("· {state}"))).test_support());
            hn += 1;
            kids += 1;
            if !s.installed {
                list = list.child(div().id(("pick-install", hn)).px(px(8.)).pb(px(6.)).text_size(px(11.5)).text_color(fg3).font_family(crate::ui::SANS_MONO).aria_label(format!("Install: {}", s.install)).child(s.install.clone()).test_support());
                kids += 1;
                continue;
            }
            let rows: Vec<_> = rows_of(s).into_iter().filter(|(m, i)| keep(s, m.as_deref(), *i)).collect();
            if rows.is_empty() {
                list = list.child(div().id(("pick-none", hn)).px(px(8.)).pb(px(6.)).text_size(px(11.5)).text_color(fg3).child("Nothing here matches.").test_support());
                kids += 1;
                continue;
            }
            any = true;
            for f in families(rows, self.start.pick_sort) {
                if let Some(up) = &f.upstream_head {
                    list = list.child(header(("pick-head", hn), up.clone(), false).test_support());
                    hn += 1;
                    kids += 1;
                }
                if f.items.len() == 1 {
                    let (m, i) = f.items.into_iter().next().expect("one");
                    let label = m.as_deref().map_or_else(|| format!("{}'s default", s.name), |m| row_label(m, i, true));
                    list = list.child(self.pick_row(n, to, current, s, m, i, label, 0.0, pal, cx));
                    n += 1;
                    kids += 1;
                    continue;
                }
                // A family: one row that drops down to its versions.
                let key = format!("{}:{}", s.provider, f.key);
                let open = filtering || self.start.pick_open.contains(&key) || f.items.iter().any(|(m, _)| current.is_some_and(|r| r.provider == s.provider && r.model == *m));
                let from = f.items.iter().filter_map(|(_, i)| i.and_then(|i| i.input)).fold(None::<f64>, |a, b| Some(a.map_or(b, |a| a.min(b))));
                let all_free = f.items.iter().all(|(_, i)| i.is_some_and(ModelInfo::free));
                let key2 = key.clone();
                list = list.child(
                    div()
                        .id(("pick-family", hn))
                        .h(px(28.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(8.))
                        .rounded(px(radius::CONTROL))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .text_color(c(pal.fg2))
                        .aria_label(f.label.clone())
                        .aria_expanded(open)
                        .child(glyph(if open { "chevron-down" } else { "chevron-right" }, 12., fg3))
                        .child(div().flex_1().min_w_0().truncate().child(f.label.clone()))
                        .child(div().flex_none().text_size(px(11.)).text_color(fg3).child(format!("{} versions", f.items.len())))
                        .when(all_free, |d| d.child(div().flex_none().px(px(6.)).h(px(18.)).flex().items_center().rounded(px(radius::TAG)).bg(ca(pal.ok, 0.14)).text_size(px(11.)).text_color(c(pal.ok)).child("Free")))
                        .when_some(from.filter(|_| !all_free), |d, v| d.child(div().flex_none().font_family(crate::ui::SANS_MONO).text_size(px(11.)).text_color(fg3).child(format!("from {}", dollars(v)))))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            if !this.start.pick_open.remove(&key2) {
                                this.start.pick_open.insert(key2.clone());
                            }
                            cx.notify();
                        }))
                        .test_support(),
                );
                hn += 1;
                kids += 1;
                if open {
                    for (m, i) in f.items {
                        let label = m.as_deref().map_or_else(|| format!("{}'s default", s.name), |m| row_label(m, i, true));
                        list = list.child(self.pick_row(n, to, current, s, m, i, label, 20.0, pal, cx));
                        n += 1;
                        kids += 1;
                    }
                }
            }
        }
        if filtering && !any {
            list = list.child(div().id("pick-none-all").px(px(8.)).py(px(10.)).text_size(px(12.5)).text_color(fg3).aria_label("No model matches").child("No model matches. Clear the search or a filter.").test_support());
        }
        (list, sections)
    }

    /// One model row: the label, a Free or price tag, a check on the current one.
    #[allow(clippy::too_many_arguments)]
    fn pick_row(&self, n: usize, to: &PickFor, current: Option<&Route>, s: &CliStatus, m: Option<String>, info: Option<&ModelInfo>, label: String, indent: f32, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let sel = current.is_some_and(|r| r.provider == s.provider && r.model == m);
        let key = fav_key(&s.provider, m.as_deref());
        let named = info.is_some_and(|i| i.name.is_some());
        let price = price_tag(info);
        let free = info.is_some_and(ModelInfo::free);
        let (to2, prov, m2, key2) = (to.clone(), s.provider.clone(), m.clone(), key);
        let ready = s.ready();
        div()
            .id(("route-model", n))
            .h(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .pl(px(8. + indent))
            .pr(px(8.))
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .text_color(if sel { fg } else { fg2 })
            .when(sel, |d| d.bg(c(pal.selected)))
            .when(!sel, |d| d.hover(|d| d.bg(c(pal.hover))))
            .when(!ready, |d| d.opacity(0.6))
            .aria_label(label.clone())
            .aria_selected(sel)
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
            .test_support()
            .into_any_element()
    }
}

/// A model family in the list: its versions, and the upstream header to draw above it when
/// sorting by provider starts a new upstream.
struct Family<'a> {
    key: String,
    label: String,
    upstream_head: Option<String>,
    items: Vec<(Option<String>, Option<&'a ModelInfo>)>,
}

/// "claude-sonnet" -> "Claude Sonnet".
fn pretty(f: &str) -> String {
    f.split(['-', '_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut ch = w.chars();
            ch.next().map(|c| format!("{}{}", c.to_uppercase(), ch.as_str())).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Folds one CLI's models into families (the CLI's `family`, per upstream provider), sorted by
/// provider (0), lowest input price (1) or name (2). The CLI's default stays first.
fn families(rows: Vec<(Option<String>, Option<&ModelInfo>)>, sort: u8) -> Vec<Family<'_>> {
    let mut out: Vec<Family> = Vec::new();
    for (m, i) in rows {
        let up = m.as_deref().and_then(|m| m.split_once('/')).map_or(String::new(), |(p, _)| p.to_string());
        let key = match (i.and_then(|i| i.family.clone()), &m) {
            (Some(f), _) => format!("{up}/{f}"),
            (None, Some(m)) => m.clone(),
            (None, None) => String::new(),
        };
        let label = i.and_then(|i| i.family.as_deref()).map(pretty).unwrap_or_default();
        match out.iter_mut().find(|f| f.key == key) {
            Some(f) => f.items.push((m, i)),
            None => out.push(Family { key, label, upstream_head: None, items: vec![(m, i)] }),
        }
    }
    let price = |f: &Family| f.items.iter().filter_map(|(_, i)| i.and_then(|i| i.input)).fold(f64::INFINITY, f64::min);
    let up = |f: &Family| f.items[0].0.as_deref().and_then(|m| m.split_once('/')).map_or(String::new(), |(p, _)| p.to_string());
    let name = |f: &Family| if f.items.len() > 1 { f.label.to_lowercase() } else { f.items[0].0.clone().unwrap_or_default().to_lowercase() };
    let default_first = |f: &Family| f.items[0].0.is_some();
    match sort {
        1 => out.sort_by(|a, b| default_first(a).cmp(&default_first(b)).then(price(a).total_cmp(&price(b)))),
        2 => out.sort_by(|a, b| default_first(a).cmp(&default_first(b)).then(name(a).cmp(&name(b)))),
        _ => {
            // Stable: upstream groups in the order the CLI lists them.
            let order: Vec<String> = out.iter().map(up).fold(Vec::new(), |mut v, u| {
                if !v.contains(&u) {
                    v.push(u);
                }
                v
            });
            out.sort_by_key(|f| (default_first(f), order.iter().position(|u| *u == up(f)).unwrap_or(0)));
            let mut last = None::<String>;
            for f in &mut out {
                let u = up(f);
                if !u.is_empty() && last.as_ref() != Some(&u) {
                    f.upstream_head = Some(u.clone());
                }
                last = Some(u);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{dollars, families, price_tag, tokens_short, ModelInfo};

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

    #[test]
    fn versions_fold_into_families_and_sort_by_price() {
        let q = |id: &str, fam: &str, inp: f64| ModelInfo { id: id.into(), family: Some(fam.into()), input: Some(inp), output: Some(inp), ..Default::default() };
        let (a, b, c) = (q("opencode/qwen3.6-plus", "qwen", 0.0), q("opencode/qwen3.5", "qwen", 0.0), q("anthropic/claude-sonnet-4-5", "claude-sonnet", 3.0));
        let rows = vec![(None, None), (Some(a.id.clone()), Some(&a)), (Some(c.id.clone()), Some(&c)), (Some(b.id.clone()), Some(&b))];
        let f = families(rows.clone(), 0);
        assert_eq!(f.len(), 3, "default, qwen (2 versions), claude-sonnet");
        assert!(f[0].items[0].0.is_none(), "the CLI's default first");
        assert_eq!((f[1].label.as_str(), f[1].items.len()), ("Qwen", 2));
        assert_eq!(f[1].upstream_head.as_deref(), Some("opencode"));
        assert_eq!(f[2].label, "Claude Sonnet");
        let by_price = families(rows, 1);
        assert_eq!(by_price[1].label, "Qwen", "cheapest after the default");
    }
}
