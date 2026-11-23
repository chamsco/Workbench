//! What a stream makes is a link (Charles, 22 Nov): the URLs and files in its replies, the files
//! its steps touched, and its side streams. A click opens it in the support card: a page in the
//! in-app browser (`browser.rs`), a file as text. A right click offers "Open in app" or "Open in
//! browser" (the system's browser, or the file's own app).

use super::*;
use gpui_kit::prelude::FluentBuilder;
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Link {
    Url(String),
    File(PathBuf),
}

impl Link {
    fn label(&self) -> String {
        match self {
            Link::Url(u) => u.clone(),
            Link::File(p) => p.display().to_string(),
        }
    }
}

/// The URLs in `text`, and the code spans (`code`, byte ranges into `text`) that name a file that
/// exists, absolute or under `dir`. A URL ends at whitespace and drops trailing punctuation.
pub(crate) fn find(text: &str, code: &[Range<usize>], dir: Option<&Path>) -> Vec<(Range<usize>, Link)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = ["https://", "http://"].iter().filter_map(|p| text[at..].find(p)).min() {
        let start = at + i;
        let len = text[start..].find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`')).unwrap_or(text.len() - start);
        let url = text[start..start + len].trim_end_matches(|c: char| matches!(c, '.' | ',' | ')' | ';' | ':' | '!' | '?' | '\'' | ']'));
        if url.len() > "https://".len() {
            out.push((start..start + url.len(), Link::Url(url.to_string())));
        }
        at = start + len.max(1);
    }
    for r in code {
        let s = text[r.clone()].trim();
        if s.is_empty() || s.contains(char::is_whitespace) || out.iter().any(|(o, _)| o.start < r.end && r.start < o.end) {
            continue;
        }
        let p = Path::new(s);
        let full = if p.is_absolute() { Some(p.to_path_buf()) } else { dir.map(|d| d.join(p)) };
        if let Some(f) = full.filter(|f| f.is_file()) {
            out.push((r.clone(), Link::File(f)));
        }
    }
    out.sort_by_key(|(r, _)| r.start);
    out
}

/// A file's head as text for the preview: at most 2000 lines or 256 KB; None for a binary file.
fn preview(p: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(p).and_then(|f| f.take(256 * 1024).read_to_end(&mut buf)).map_err(|e| e.to_string())?;
    if buf.contains(&0) {
        return Err("A binary file: open it in its own app.".into());
    }
    Ok(String::from_utf8_lossy(&buf).lines().take(2000).collect::<Vec<_>>().join("\n"))
}

impl Cauldron {
    /// Opens `link` in the support card (`in_app`) or outside Cauldron.
    pub(super) fn open_link(&mut self, link: Link, in_app: bool, cx: &mut Context<Self>) {
        self.link_menu = None;
        if !in_app {
            match &link {
                Link::Url(u) => cx.open_url(u),
                Link::File(p) => cx.open_url(&format!("file://{}", p.display())),
            }
            cx.notify();
            return;
        }
        // A picture opens over everything, at full size (Charles, 22 Nov).
        if let Link::File(f) = &link {
            if super::imageview::is_image(f) {
                self.image_view = Some(f.clone());
                cx.notify();
                return;
            }
        }
        // A page opens in the app's browser, not outside or on a host (Charles, 22 Nov).
        let link = match link {
            Link::File(f) if f.extension().is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm")) => Link::Url(format!("file://{}", f.display())),
            l => l,
        };
        let p = if matches!(link, Link::Url(_)) { Panel::Browser } else { Panel::Files };
        self.link = Some(link);
        if self.drawer != Some(p) {
            self.toggle_panel(p);
        }
        cx.notify();
    }

    pub(super) fn open_link_menu(&mut self, link: Link, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.link_menu = Some((link, at));
        cx.notify();
    }

    /// The right-click menu on a link: "Open in app", "Open in browser".
    pub(super) fn link_menu_el(&self, pal: &'static Palette, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (link, at) = self.link_menu.clone()?;
        let vp = window.viewport_size();
        let w = 220.0;
        let x = f32::from(at.x).min(f32::from(vp.width) - w - 8.0).max(8.0);
        let y = f32::from(at.y).min(f32::from(vp.height) - 110.0).max(8.0);
        let item = |id: &'static str, icon: &'static str, label: &'static str, in_app: bool, link: Link, cx: &mut Context<Self>| {
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
                .text_color(c(pal.fg2))
                .aria_label(label)
                .child(glyph(icon, 14., c(pal.fg3)))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.open_link(link.clone(), in_app, cx)))
                .test_support()
        };
        let outside = if matches!(link, Link::Url(_)) { "Open in browser" } else { "Open in its app" };
        Some(
            div()
                .id("link-menu")
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .p(px(4.))
                .flex()
                .flex_col()
                .gap(px(1.))
                .rounded(px(radius::PANEL))
                .bg(c(pal.raised))
                .border_1()
                .border_color(c(pal.line))
                .shadow(vec![drop(tokens::shadow::MENU)])
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.link_menu = None;
                    cx.notify();
                }))
                .child(div().px(px(8.)).py(px(4.)).text_size(px(11.)).text_color(c(pal.fg3)).truncate().child(link.label()))
                .child(item("link-in-app", "panel-right", "Open in app", true, link.clone(), cx))
                .child(item("link-outside", "external-link", outside, false, link, cx))
                .test_support()
                .into_any_element(),
        )
    }

    /// The support card's body for an opened link, when `p` is the panel it opened in.
    pub(super) fn link_body(&self, p: Panel, pal: &'static Palette, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let link = self.link.clone()?;
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let load_progress = (p == Panel::Browser).then(|| self.browser.load_progress()).flatten();
        if load_progress.is_some_and(|(_, animate)| animate) {
            window.request_animation_frame();
        }
        let head = |label: String, outside: &'static str, link: Link, cx: &mut Context<Self>| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(30.))
                .flex_none()
                .text_size(px(12.))
                .child(div().min_w_0().flex_1().truncate().font_family(MONO).text_size(px(11.5)).text_color(fg).child(label))
                .child(
                    div()
                        .id("link-outside-btn")
                        .flex_none()
                        .h(px(24.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded(px(radius::CONTROL))
                        .border_1()
                        .border_color(c(pal.line))
                        .text_color(c(pal.fg2))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label(outside)
                        .child(glyph("external-link", 12., fg3))
                        .child(outside)
                        .on_click(cx.listener(move |this, _, _, cx| this.open_link(link.clone(), false, cx)))
                        .test_support(),
                )
                .child(
                    div()
                        .id("link-close")
                        .flex_none()
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::CONTROL))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label("Close the link")
                        .child(glyph("x", 13., fg3))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.link = None;
                            cx.notify();
                        }))
                        .test_support(),
                )
        };
        let browser_head = |label: String, link: Link, cx: &mut Context<Self>| {
            let utility = |id: &'static str, icon: &'static str, label: &'static str| {
                div()
                    .id(id)
                    .size(px(24.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(label)
                    .child(glyph(icon, 13., fg3))
            };
            let progress = load_progress.map(|(width, _)| {
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h(px(2.))
                    .w(relative(width))
                    .rounded(px(radius::CONTROL))
                    .bg(ca(pal.fg, 0.18))
            });
            div()
                .id("browser-head")
                .relative()
                .flex()
                .items_center()
                .gap(px(3.))
                .h(px(30.))
                .flex_none()
                .px(px(3.))
                .rounded(px(radius::CONTROL))
                .bg(c(pal.inset))
                .border_1()
                .border_color(c(pal.line))
                .children(progress)
                .child(utility("browser-back", "chevron-left", "Go back").on_click(cx.listener(|this, _, _, cx| {
                    this.browser.go_back();
                    cx.notify();
                })))
                .child(utility("browser-forward", "chevron-right", "Go forward").on_click(cx.listener(|this, _, _, cx| {
                    this.browser.go_forward();
                    cx.notify();
                })))
                .child(utility("browser-reload", "refresh-cw", "Reload page").on_click(cx.listener(|this, _, _, cx| {
                    this.browser.reload();
                    cx.notify();
                })))
                .child(div().relative().min_w_0().flex_1().truncate().font_family(MONO).text_size(px(11.5)).text_color(fg).child(label))
                .child(
                    div()
                        .id("link-outside-btn")
                        .flex_none()
                        .h(px(24.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded(px(radius::CONTROL))
                        .border_1()
                        .border_color(c(pal.line))
                        .text_color(c(pal.fg2))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label("Open in browser")
                        .child(glyph("external-link", 12., fg3))
                        .child("Open in browser")
                        .on_click(cx.listener(move |this, _, _, cx| this.open_link(link.clone(), false, cx)))
                        .test_support(),
                )
                .child(
                    div()
                        .id("link-close")
                        .flex_none()
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::CONTROL))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label("Close the link")
                        .child(glyph("x", 13., fg3))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.link = None;
                            cx.notify();
                        }))
                        .test_support(),
                )
                .test_support()
        };
        let col = div().id("link-body").size_full().flex().flex_col().gap(px(6.)).px(px(14.)).pt(px(10.)).pb(px(12.));
        Some(match (&link, p) {
            (Link::Url(u), Panel::Browser) => {
                // The page itself is a native view over this box (`browser.rs`), where there is one.
                let slot = self.browser_slot.clone();
                let page = div()
                    .id("browser-page")
                    .flex_1()
                    .min_h_0()
                    .rounded(px(radius::INNER))
                    .bg(c(pal.inset))
                    .relative()
                    .child(canvas(move |b, _, _| slot.set(Some(b)), |_, _, _, _| {}).absolute().inset_0())
                    .when_some(self.browser.error.clone(), |d, e| d.p(px(10.)).text_size(px(12.)).text_color(c(pal.bad)).child(format!("The browser could not start: {e}")))
                    .when(!super::browser::AVAILABLE, |d| {
                        d.flex().flex_col().items_center().justify_center().gap(px(4.)).text_size(px(12.)).text_color(fg3).child("The in-app browser runs on Windows and macOS.").child("On this system, open the page in your browser.")
                    })
                    .test_support();
                col.child(browser_head(u.clone(), link.clone(), cx)).child(page).into_any_element()
            }
            (Link::File(f), Panel::Files) => {
                let body = match preview(f) {
                    Ok(t) => div().id("link-file").flex_1().min_h_0().overflow_y_scroll().p(px(10.)).rounded(px(radius::INNER)).bg(c(pal.inset)).font_family(MONO).text_size(px(11.5)).line_height(px(17.)).text_color(c(pal.fg2)).child(t).test_support(),
                    Err(e) => div().id("link-file").p(px(10.)).text_size(px(12.)).text_color(fg3).child(e).test_support(),
                };
                col.child(head(f.display().to_string(), "Open in its app", link.clone(), cx)).child(body).into_any_element()
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{find, Link};

    #[test]
    fn urls_and_existing_files_are_links() {
        let d = std::env::temp_dir().join(format!("bs-links-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("index.html"), "<p>").unwrap();
        let t = "See https://example.com/a?b=1. Then open index.html or missing.rs (http://x.io).";
        let code = [t.find("index.html").map(|i| i..i + 10).unwrap(), t.find("missing.rs").map(|i| i..i + 10).unwrap()];
        let l = find(t, &code, Some(&d));
        assert_eq!(l.len(), 3, "{l:?}");
        assert_eq!(l[0].1, Link::Url("https://example.com/a?b=1".into()));
        assert_eq!(l[1].1, Link::File(d.join("index.html")));
        assert_eq!(l[2].1, Link::Url("http://x.io".into()));
        assert_eq!(&t[l[2].0.clone()], "http://x.io");
        let _ = std::fs::remove_dir_all(&d);
    }
}

impl Cauldron {
    /// Preview cards for the first three web links in a finished reply (Charles, 23 Nov): the
    /// site, the page's title and a line of what it is, fetched once per URL in the background.
    /// A click opens the link as its text does; a page that could not be read shows its site.
    pub(super) fn unfurl_cards(&self, i: usize, text: &str, pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mut urls: Vec<String> = Vec::new();
        for (_, l) in find(text, &[], None) {
            if let Link::Url(u) = l {
                let h = backspace_core::unfurl::host(&u);
                // Local servers are the user's own work in progress, not pages to preview.
                if !urls.contains(&u) && !h.starts_with("localhost") && !h.starts_with("127.") && !h.starts_with("0.0.0.0") {
                    urls.push(u);
                }
            }
            if urls.len() == 3 {
                break;
            }
        }
        if urls.is_empty() {
            return None;
        }
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut col = div().id(("unfurls", i)).flex().flex_col().gap(px(6.)).max_w(px(520.));
        for (k, u) in urls.into_iter().enumerate() {
            let known = self.unfurls.borrow().get(&u).cloned();
            // Tests stay off the network: their previews stay loading.
            if known.is_none() && !cfg!(test) {
                self.unfurls.borrow_mut().insert(u.clone(), None);
                let (cache, key, url) = (self.unfurls.clone(), u.clone(), u.clone());
                cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let got = cx.background_executor().spawn(async move { backspace_core::unfurl::fetch(&url) }).await;
                    let _ = this.update(cx, |_, cx| {
                        cache.borrow_mut().insert(key, Some(got));
                        cx.notify();
                    });
                })
                .detach();
            }
            let card = known.flatten();
            let site = card.as_ref().map(|c| c.site.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| backspace_core::unfurl::host(&u));
            let title = card.as_ref().and_then(|c| c.title.clone());
            let desc = card.as_ref().and_then(|c| c.description.clone());
            let loading = card.is_none();
            let open = u.clone();
            col = col.child(
                super::surface::block(pal)
                    .id(("unfurl", i * 10 + k))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(9.))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.raised)))
                    .aria_label(format!("Link preview: {}", title.clone().unwrap_or_else(|| u.clone())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().truncate().text_size(px(11.)).text_color(fg3).child(site))
                            .child(div().truncate().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(if loading { fg3 } else { fg }).child(title.unwrap_or_else(|| if loading { "Loading preview…".into() } else { u.clone() })))
                            .when_some(desc, |d, t| d.child(div().text_size(px(12.)).line_height(px(17.)).max_h(px(34.)).overflow_hidden().text_color(fg2).child(t))),
                    )
                    .child(div().flex_none().child(glyph("external-link", 13., fg3)))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_link(Link::Url(open.clone()), true, cx)))
                    .test_support(),
            );
        }
        Some(col.into_any_element())
    }
}
