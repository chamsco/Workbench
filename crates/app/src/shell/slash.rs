//! Slash commands in the reply box (Charles, 22 Nov): typing `/` opens a list over the box,
//! filtered as you type, each command with what it does right under its name. Up and Down move,
//! Tab or Enter completes, Escape closes; Enter on a complete command runs it. The project's
//! skills (core skills.rs) are listed after the built-in commands.

use super::*;
use gpui_kit::prelude::FluentBuilder;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cmd {
    pub(crate) name: String,
    /// What it takes after the name, shown dimmed; empty when it takes nothing.
    pub(crate) args: &'static str,
    pub(crate) what: String,
    /// A skill: Enter sends "Use the `name` skill." with what follows.
    pub(crate) skill: bool,
}

const BUILT_IN: [(&str, &str, &str); 10] = [
    ("goal", "<objective> | pause | resume | clear", "Keep working until an objective is met. The stream carries on by itself, up to 8 turns in a row, and stops when its reply says the goal is met."),
    ("ask", "", "Switch to Ask: it answers from what it reads, with no edits and no commands."),
    ("plan", "", "Switch to Plan: it proposes a plan first and edits nothing."),
    ("act", "", "Switch to Act: it can edit files and run commands."),
    ("side", "<question>", "Ask in a side thread beside this stream, without interrupting it."),
    ("review", "[what to look at]", "Review the uncommitted changes in this project for bugs, risks and missing tests."),
    ("init", "", "Write an AGENTS.md that explains this project to agents: layout, commands, rules."),
    ("model", "", "Pick the model this stream runs on."),
    ("copy", "", "Copy the last reply as Markdown."),
    ("new", "", "Start a new stream."),
];

/// Every command: the built-in ones, then the skills found for `project`.
pub(crate) fn all(project: Option<&str>) -> Vec<Cmd> {
    let mut v: Vec<Cmd> = BUILT_IN.iter().map(|(n, a, w)| Cmd { name: n.to_string(), args: a, what: w.to_string(), skill: false }).collect();
    let dir = project.map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    for s in backspace_core::skills::Skills::load(&dir).found(&dir) {
        if v.iter().any(|c| c.name == s.name) {
            continue;
        }
        let what = s.description.lines().next().unwrap_or("").trim().to_string();
        v.push(Cmd { name: s.name, args: "[task]", what: if what.is_empty() { "A skill.".into() } else { format!("Skill. {what}") }, skill: true });
    }
    v
}

/// The commands matching what is typed: only while the text is `/word` with no space yet. Names
/// starting with the word come first, then names containing it.
pub(crate) fn matches(text: &str, cmds: &[Cmd]) -> Vec<Cmd> {
    let Some(word) = text.strip_prefix('/') else { return Vec::new() };
    if word.contains(char::is_whitespace) {
        return Vec::new();
    }
    let w = word.to_lowercase();
    let mut out: Vec<Cmd> = cmds.iter().filter(|c| c.name.starts_with(&w)).cloned().collect();
    out.extend(cmds.iter().filter(|c| !c.name.starts_with(&w) && c.name.contains(&w)).cloned());
    out
}

/// A typed command and its rest: `/goal ship it` is ("goal", "ship it").
pub(crate) fn split(text: &str) -> Option<(&str, &str)> {
    let t = text.strip_prefix('/')?;
    let (name, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    (!name.is_empty()).then_some((name, rest.trim()))
}

impl Cauldron {
    /// `/side`: a side thread gets the question; this stream carries on undisturbed.
    fn side_ask(&mut self, q: String, cx: &mut Context<Self>) {
        let (Some(id), Some((_, chats))) = (self.open.clone(), self.core.as_ref()) else { return };
        let chats = chats.clone();
        match chats.side(&id, &q, self.side_ctx).and_then(|t| chats.send(&t.id, &q, Vec::new(), None, self.mode_ctx())) {
            Ok(()) => self.refresh(),
            Err(e) => self.send_error = Some(e.to_string()),
        }
        cx.notify();
    }

    fn composer_text(&self, cx: &App) -> String {
        self.composer.as_ref().map(|(st, _)| st.read(cx).value().to_string()).unwrap_or_default()
    }

    /// The list under the pointer of the reply box, as typed now.
    pub(super) fn slash_list(&self, cx: &App) -> Vec<Cmd> {
        if self.slash_closed {
            return Vec::new();
        }
        let text = self.composer_text(cx);
        if !text.starts_with('/') {
            return Vec::new();
        }
        let project = self.thread.as_ref().and_then(|t| t.project.clone());
        let mut cache = self.slash_cmds.borrow_mut();
        if cache.as_ref().is_none_or(|(p, _)| *p != project) {
            *cache = Some((project.clone(), all(project.as_deref())));
        }
        matches(&text, &cache.as_ref().unwrap().1)
    }

    /// Up, Down, Tab, Escape while the list is open. True when the key was the list's.
    pub(super) fn slash_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let list = self.slash_list(cx);
        if list.is_empty() {
            return false;
        }
        match key {
            "up" => self.slash_sel = (self.slash_sel + list.len() - 1) % list.len(),
            "down" => self.slash_sel = (self.slash_sel + 1) % list.len(),
            "tab" => self.slash_complete(&list, window, cx),
            "escape" => self.slash_closed = true,
            _ => return false,
        }
        cx.notify();
        true
    }

    /// Puts the picked command in the box, ready for what it takes.
    fn slash_complete(&mut self, list: &[Cmd], window: &mut Window, cx: &mut Context<Self>) {
        let Some(cmd) = list.get(self.slash_sel.min(list.len().saturating_sub(1))) else { return };
        let text = format!("/{} ", cmd.name);
        if let Some((st, _)) = &self.composer {
            st.update(cx, |s, cx| s.set_value(text, window, cx));
        }
        self.slash_sel = 0;
    }

    /// Enter with the list open: a command that takes nothing runs, one that takes something is
    /// completed so you can type it.
    pub(super) fn slash_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let list = self.slash_list(cx);
        let Some(cmd) = list.get(self.slash_sel.min(list.len().saturating_sub(1))).cloned() else { return false };
        if cmd.args.is_empty() {
            if let Some((st, _)) = &self.composer {
                st.update(cx, |s, cx| s.set_value(format!("/{}", cmd.name), window, cx));
            }
            return false; // the send runs it
        }
        self.slash_complete(&list, window, cx);
        cx.notify();
        true
    }

    /// Runs `/name rest` from the reply box. None when the text is not a command (it is sent as
    /// is); Some(text to send instead, if any) when it is.
    pub(super) fn run_slash(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> Option<Option<String>> {
        let (name, rest) = split(text)?;
        let project = self.thread.as_ref().and_then(|t| t.project.clone());
        let id = self.open.clone()?;
        let chats = self.core.as_ref().map(|(_, c)| c.clone())?;
        let out = match name {
            "goal" => match rest {
                "" => {
                    self.send_error = Some(self.thread.as_ref().and_then(|t| t.goal.clone()).map_or_else(|| "No goal yet: /goal <objective>".into(), |g| format!("Goal: {g}")));
                    None
                }
                "pause" | "resume" => {
                    let _ = chats.set_goal(&id, None, rest == "pause");
                    None
                }
                "clear" => {
                    let _ = chats.clear_goal(&id);
                    None
                }
                g => {
                    let _ = chats.set_goal(&id, Some(g), false);
                    Some(format!("Goal: {g}"))
                }
            },
            "ask" | "plan" | "act" => {
                self.ask = name == "ask";
                self.set_plan(name == "plan", window, cx);
                None
            }
            "side" if !rest.is_empty() => {
                self.side_ask(rest.to_string(), cx);
                None
            }
            "review" => Some(format!(
                "Review the uncommitted changes in this project{}. Read the diff, then list what is wrong or risky (bugs, missing tests, unclear names), most serious first, each with the file and line. Change nothing.",
                if rest.is_empty() { String::new() } else { format!(", looking at {rest}") }
            )),
            "init" => Some("Write an AGENTS.md at the root of this project for agents that will work in it: what the project is, its layout, how to build, test and run it, and the rules to follow. Read the project first; keep it short and true.".into()),
            "model" => {
                self.start.route_menu = Some(super::subs::PickFor::Thread(id.clone()));
                self.load_providers(cx);
                None
            }
            "copy" => {
                if let Some(m) = self.thread.as_ref().and_then(|t| t.messages.iter().rev().find(|m| m.role == backspace_core::chat::Role::Assistant && !m.text.trim().is_empty())) {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(m.text.clone()));
                }
                None
            }
            "new" => {
                self.new_thread(window, cx);
                None
            }
            other => {
                let cmds = all(project.as_deref());
                let c = cmds.iter().find(|c| c.name == other && c.skill)?;
                Some(format!("Use the `{}` skill.{}", c.name, if rest.is_empty() { String::new() } else { format!(" {rest}") }))
            }
        };
        Some(out)
    }

    /// The list over the reply box.
    pub(super) fn slash_el(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let list = self.slash_list(cx);
        if list.is_empty() {
            return None;
        }
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let sel = self.slash_sel.min(list.len() - 1);
        let mut col = div()
            .id("slash-list")
            .max_h(px(320.))
            .overflow_y_scroll()
            .p(px(4.))
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .border_1()
            .border_color(c(pal.line))
            .bg(c(pal.raised))
            .shadow(super::lift(super::Lift::Card, pal))
            .occlude()
            .aria_label("Commands");
        for (i, cmd) in list.into_iter().enumerate() {
            let on = i == sel;
            let name = cmd.name.clone();
            col = col.child(
                div()
                    .id(("slash", i))
                    .px(px(10.))
                    .py(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when(on, |d| d.bg(c(pal.selected)))
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(format!("/{}", cmd.name))
                    .aria_selected(on)
                    .aria_description(cmd.what.clone())
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .text_size(px(13.))
                            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(format!("/{}", cmd.name)))
                            .when(!cmd.args.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(fg3).font_family(MONO).child(cmd.args)))
                            .when(cmd.skill, |d| d.child(div().ml_auto().text_size(px(11.)).text_color(fg3).child("skill"))),
                    )
                    .child(div().text_size(px(12.)).line_height(px(17.)).text_color(fg3).child(cmd.what.clone()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            let list = this.slash_list(cx);
                            this.slash_sel = list.iter().position(|c| c.name == name).unwrap_or(0);
                            if !this.slash_enter(window, cx) {
                                this.send_composer(window, cx);
                            }
                            if let Some((st, _)) = &this.composer {
                                let f = st.read(cx).focus_handle(cx);
                                window.focus(&f, cx);
                            }
                        }),
                    )
                    .test_support(),
            );
        }
        Some(col.test_support().into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn slash_lists_by_prefix_then_contains_and_closes_on_a_space() {
        let cmds = all(None);
        let m = matches("/go", &cmds);
        assert_eq!(m[0].name, "goal");
        assert!(m[0].what.starts_with("Keep working until an objective is met"));
        assert!(matches("/", &cmds).len() >= BUILT_IN.len(), "a bare / lists everything");
        assert!(matches("/goal ship", &cmds).is_empty(), "typing its argument closes the list");
        assert!(matches("hello", &cmds).is_empty());
        assert_eq!(split("/goal ship it"), Some(("goal", "ship it")));
        assert_eq!(split("/act"), Some(("act", "")));
        assert!(all(None).iter().any(|c| c.skill), "the skills are listed too");
    }
}
