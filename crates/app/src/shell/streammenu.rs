//! The right-click menu on a stream (board round 6, pick R2; Charles 21 Nov 04:59Z): a header with
//! the stream's name and state, then Open in split, Rename, Mark as read, Pin, Archive and Delete.
//! Delete asks in place ("Delete this stream for good?") and only then deletes.

use super::*;

pub(crate) struct StreamMenu {
    pub id: String,
    pub at: Point<Pixels>,
    /// Delete is waiting for its yes.
    pub confirm: bool,
    /// Rename is open: the field, and what ends it.
    pub rename: Option<(Entity<InputState>, Subscription)>,
}

const MENU_W: f32 = 232.0;

impl Cauldron {
    pub(super) fn open_stream_menu(&mut self, id: String, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.stream_menu = Some(StreamMenu { id, at, confirm: false, rename: None });
        cx.notify();
    }

    fn close_stream_menu(&mut self, cx: &mut Context<Self>) {
        self.stream_menu = None;
        cx.notify();
    }

    fn info_of(&self, id: &str) -> Option<ThreadInfo> {
        match &self.threads {
            Threads::Ready(v) => v.iter().find(|t| t.id == id).cloned(),
            _ => None,
        }
    }

    /// Runs one of the menu's actions on its stream, then closes the menu.
    fn stream_action(&mut self, act: StreamAct, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.stream_menu.take() else { return };
        let id = menu.id;
        let chats = self.core.as_ref().map(|(_, c)| c.clone());
        match act {
            StreamAct::Split => {
                self.selected = Some(id.clone());
                self.open_thread(id);
                if self.panes.shape.count() == 1 {
                    self.set_shape(self.panes.shape.next());
                }
            }
            StreamAct::Rename(title) => {
                if let Some(c) = &chats {
                    let _ = c.rename(&id, &title);
                }
            }
            StreamAct::Read => {
                if let Some(c) = &chats {
                    c.mark_read(&id);
                }
            }
            StreamAct::Pin(on) => {
                if let Some(c) = &chats {
                    let _ = c.pin(&id, on);
                }
            }
            StreamAct::Archive(on) => {
                if let Some(c) = &chats {
                    let _ = c.archive(&id, on);
                }
                if on && self.open.as_deref() == Some(id.as_str()) {
                    self.close_open_stream();
                }
            }
            StreamAct::Delete => {
                if let Some(c) = &chats {
                    // Its side streams go with it.
                    let sides: Vec<String> = c.list().into_iter().filter(|t| t.side_of.as_deref() == Some(id.as_str())).map(|t| t.id).collect();
                    for s in sides {
                        let _ = c.delete(&s);
                    }
                    let _ = c.delete(&id);
                }
                if self.open.as_deref() == Some(id.as_str()) {
                    self.close_open_stream();
                }
            }
        }
        self.refresh();
        let _ = window;
        cx.notify();
    }

    /// The open stream is gone from view (archived or deleted): back to the New stream screen.
    fn close_open_stream(&mut self) {
        self.open = None;
        self.thread = None;
        self.selected = None;
        self.sides.clear();
        self.side_root = None;
    }

    /// The menu, as an overlay in window coordinates; None when it is closed.
    pub(super) fn stream_menu_el(&self, pal: &'static Palette, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.stream_menu.as_ref()?;
        let info = self.info_of(&menu.id);
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let pinned = info.as_ref().is_some_and(|t| t.pinned);
        let archived = info.as_ref().is_some_and(|t| t.archived);
        let title = info.as_ref().map_or_else(|| "Stream".to_string(), |t| t.title.clone());
        let state = match &info {
            Some(t) if t.busy => "Working",
            Some(t) if t.failed.is_some() => "Failed",
            Some(t) if t.unread => "Needs you",
            _ => "Done",
        };
        let vp = window.viewport_size();
        let h_guess = if menu.confirm { 150.0 } else { 244.0 };
        let x = f32::from(menu.at.x).min(f32::from(vp.width) - MENU_W - 8.0).max(8.0);
        let y = f32::from(menu.at.y).min(f32::from(vp.height) - h_guess - 8.0).max(8.0);
        let item = |id: &'static str, icon: &'static str, label: &'static str| {
            div()
                .id(id)
                .h(px(28.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .text_size(px(12.5))
                .text_color(fg2)
                .aria_label(label)
                .child(glyph(icon, 14., fg3))
                .child(label)
        };
        let mut col = div()
            .id("stream-menu")
            .w(px(MENU_W))
            .p(px(4.))
            .flex()
            .flex_col()
            .gap(px(1.))
            .rounded(px(radius::PANEL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow(vec![drop(tokens::shadow::MENU)])
            .font_family(SANS)
            .font_features(sans_features())
            .font_weight(FontWeight::MEDIUM)
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_stream_menu(cx)));
        // The head: the name and where the stream stands, or the rename field.
        col = col.child(match &menu.rename {
            Some((st, _)) => div()
                .id("stream-menu-rename")
                .h(px(30.))
                .mx(px(2.))
                .mb(px(3.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(c(pal.accent))
                .child(Input::new(st).appearance(false).text_size(px(12.5)).p_0())
                .test_support()
                .into_any_element(),
            None => div()
                .id("stream-menu-head")
                .px(px(8.))
                .pt(px(4.))
                .pb(px(6.))
                .mb(px(3.))
                .border_b_1()
                .border_color(c(pal.line))
                .flex()
                .flex_col()
                .child(div().truncate().text_size(px(12.5)).text_color(fg).child(title.clone()))
                .child(div().text_size(px(11.)).text_color(fg3).child(state))
                .test_support()
                .into_any_element(),
        });
        if menu.confirm {
            col = col
                .child(div().px(px(8.)).py(px(6.)).text_size(px(12.5)).line_height(px(17.)).text_color(fg).child("Delete this stream for good?"))
                .child(div().px(px(8.)).pb(px(6.)).text_size(px(11.5)).line_height(px(15.)).text_color(fg3).child("Its side streams go with it. This can't be undone."))
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .px(px(4.))
                        .pb(px(2.))
                        .child(
                            div()
                                .id("menu-delete-yes")
                                .flex_1()
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(radius::CONTROL))
                                .cursor_pointer()
                                .bg(c(pal.bad))
                                .text_size(px(12.5))
                                .text_color(c(pal.raised))
                                .aria_label("Delete")
                                .child("Delete")
                                .on_click(cx.listener(|this, _, window, cx| this.stream_action(StreamAct::Delete, window, cx)))
                                .test_support(),
                        )
                        .child(
                            div()
                                .id("menu-delete-keep")
                                .flex_1()
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(radius::CONTROL))
                                .cursor_pointer()
                                .border_1()
                                .border_color(c(pal.line_strong))
                                .hover(|d| d.bg(c(pal.hover)))
                                .text_size(px(12.5))
                                .text_color(fg)
                                .aria_label("Keep")
                                .child("Keep")
                                .on_click(cx.listener(|this, _, _, cx| this.close_stream_menu(cx)))
                                .test_support(),
                        ),
                );
        } else {
            col = col
                .child(item("menu-split", "columns-2", "Open in split").on_click(cx.listener(|this, _, window, cx| this.stream_action(StreamAct::Split, window, cx))).test_support())
                .child(
                    item("menu-rename", "pencil", "Rename")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let current = this.stream_menu.as_ref().and_then(|m| this.info_of(&m.id)).map(|t| t.title).unwrap_or_default();
                            let st = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
                            st.update(cx, |s, cx| s.set_value(current, window, cx));
                            let sub = cx.subscribe_in(&st, window, |this, st, ev: &InputEvent, window, cx| match ev {
                                InputEvent::PressEnter { .. } => {
                                    let text = st.read(cx).value().to_string();
                                    if text.trim().is_empty() {
                                        this.close_stream_menu(cx);
                                    } else {
                                        this.stream_action(StreamAct::Rename(text), window, cx);
                                    }
                                }
                                _ => {}
                            });
                            st.read(cx).focus_handle(cx).focus(window, cx);
                            if let Some(m) = &mut this.stream_menu {
                                m.rename = Some((st, sub));
                            }
                            cx.notify();
                        }))
                        .test_support(),
                )
                .child(item("menu-read", "check-check", "Mark as read").on_click(cx.listener(|this, _, window, cx| this.stream_action(StreamAct::Read, window, cx))).test_support())
                .child(
                    item("menu-pin", if pinned { "pin-off" } else { "pin" }, if pinned { "Unpin" } else { "Pin to the top" })
                        .on_click(cx.listener(move |this, _, window, cx| this.stream_action(StreamAct::Pin(!pinned), window, cx)))
                        .test_support(),
                )
                .child(
                    item("menu-archive", if archived { "archive-restore" } else { "archive" }, if archived { "Unarchive" } else { "Archive" })
                        .on_click(cx.listener(move |this, _, window, cx| this.stream_action(StreamAct::Archive(!archived), window, cx)))
                        .test_support(),
                )
                .child(
                    item("menu-delete", "trash", "Delete")
                        .text_color(c(pal.bad))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(m) = &mut this.stream_menu {
                                m.confirm = true;
                                m.rename = None;
                            }
                            cx.notify();
                        }))
                        .test_support(),
                );
        }
        Some(deferred(div().absolute().left(px(x)).top(px(y)).child(col)).with_priority(5).into_any_element())
    }
}

pub(crate) enum StreamAct {
    Split,
    Rename(String),
    Read,
    Pin(bool),
    Archive(bool),
    Delete,
}

#[cfg(test)]
mod tests {
    use super::super::thread::tests::open;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    /// Right-click on a row: pin and unpin, archive and bring back, delete only after the in-place
    /// yes, and Keep leaves the stream alone.
    #[gpui_kit::test]
    fn the_stream_menu_pins_archives_and_deletes_after_a_yes(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.right_click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find("menu-pin").label(), Some("Pin to the top"));
            window.click("menu-pin", cx);
        })
        .unwrap();
        assert_eq!(chats.list().iter().filter(|t| t.pinned).count(), 1, "Pin saves");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("stream-menu").is_none(), "an action closes the menu");
            window.right_click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find("menu-pin").label(), Some("Unpin"), "a pinned stream offers Unpin");
            window.click("menu-archive", cx);
        })
        .unwrap();
        let rows = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| match &c.threads { super::Threads::Ready(v) => v.iter().filter(|t| !t.archived).count(), _ => 0 }).unwrap();
        assert_eq!(chats.list().iter().filter(|t| t.archived).count(), 1, "Archive saves, nothing is deleted");
        assert_eq!(chats.list().len(), 2);
        assert_eq!(rows(cx), 1, "and the row leaves the list");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.right_click(("thread", 0usize), cx);
            window.render_frame(cx);
            window.click("menu-delete", cx);
            window.render_frame(cx);
            assert!(window.try_find("menu-delete-yes").is_some(), "Delete asks first");
            window.click("menu-delete-keep", cx);
        })
        .unwrap();
        assert_eq!(chats.list().len(), 2, "Keep deletes nothing");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.right_click(("thread", 0usize), cx);
            window.render_frame(cx);
            window.click("menu-delete", cx);
            window.render_frame(cx);
            window.click("menu-delete-yes", cx);
        })
        .unwrap();
        assert_eq!(chats.list().len(), 1, "the yes deletes it");
    }
}
