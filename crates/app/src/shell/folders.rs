//! Folders, glass (the destinations board, section 3; Charles, 13 Nov 18:49Z: "we had a glass like
//! effect on ours, I would like it again"). A: every folder as the old app's Memory folder
//! (`legacy/tauri-app/ui/dash.css`, `.fold-art`) redrawn in the new tokens: a coloured back with
//! its tab, two sheets that lift on hover, a frosted front with the Updated pill. B: a card opens
//! the folder's detail, the same glass folder as its header, then its streams and notes.
//!
//! The front's frost is the sheets blurred under the folder's colour, drawn into one SVG per colour
//! (and hover); GPUI keeps the picture, so each is made once. GPUI has no backdrop blur
//! (GLASS-FEASIBILITY.md).
//!
//! Real: the folders streams are in, their streams, what runs or needs you, the notes memory holds
//! for each. Not yet in core, so marked: agents per folder, the Librarian's proposals (Example);
//! New folder and filing (Demo: not built yet) until core's folder field lands.

use super::pages::{tag, AppPage};
use super::*;
use std::collections::HashMap;

fn hex(v: u32) -> String {
    format!("#{v:06x}")
}

/// `color-mix(in srgb, a p, b)`.
fn mix(a: u32, b: u32, p: f32) -> String {
    let ch = |v: u32, s: u32| ((v >> s) & 255) as f32;
    let m = |s: u32| (ch(a, s) * p + ch(b, s) * (1.0 - p)).round() as u32;
    hex((m(16) << 16) | (m(8) << 8) | m(0))
}

/// How a folder is drawn: a folder, a Librarian proposal (outlined, see-through), or the New
/// folder tile.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Art {
    Folder,
    Proposal,
    New,
}

/// The folder art, 148x108 as on the board (`.gf-art`), plus a margin for the front's shadow.
/// `lift` is the hover: the sheets tip further and rise.
pub(super) fn art_svg(fc: u32, art: Art, lift: bool, dark: bool) -> String {
    let back = mix(fc, 0x000000, 0.62);
    let top = mix(fc, 0xffffff, 0.55);
    let col = hex(fc);
    let sheet = |x: f32, deg: f32, dy: f32, op: f32| {
        let (cx, cy) = (x + 29., 35. + dy);
        format!(
            r##"<g opacity="{op}" transform="translate(0 {dy}) rotate({deg} {cx} {cy})"><rect x="{x}" y="2" width="58" height="66" rx="6" fill="#f4f4f2"/><rect x="{a}" y="12" width="42" height="3" rx="1.5" fill="#d4d4d9"/><rect x="{a}" y="20" width="33.6" height="3" rx="1.5" fill="#d4d4d9"/><rect x="{a}" y="28" width="25.2" height="3" rx="1.5" fill="#d4d4d9"/></g>"##,
            a = x + 8.
        )
    };
    let (d1, d2) = if lift { (-5., -6.) } else { (0., 0.) };
    let (r1, r2) = if lift { (-12., 10.) } else { (-8., 6.) };
    let line = if dark { "#ffffff" } else { "#000000" };
    let body = match art {
        Art::Folder => {
            let sheets = sheet(20., r1, d1, 1.0) + &sheet(70., r2, d2, 1.0);
            format!(
                r##"<rect x="4" y="3" width="52" height="20" rx="9" fill="{back}"/>
<rect x="4" y="12" width="140" height="90" rx="12" fill="{back}"/>
<g>{sheets}</g>
<rect x="6" y="48" width="136" height="64" rx="13" fill="#000" opacity="0.26" filter="url(#drop)"/>
<g clip-path="url(#front)"><rect x="0" y="38" width="148" height="70" fill="{back}"/><g filter="url(#frost)">{sheets}</g></g>
<rect x="0" y="38" width="148" height="70" rx="13" fill="url(#glass)"/>
<rect x="0.5" y="38.5" width="147" height="69" rx="12.5" fill="none" stroke="#fff" stroke-opacity="0.1"/>
<path d="M13 38.5 H135" stroke="#fff" stroke-opacity="0.4"/>"##
            )
        }
        Art::Proposal => {
            let sheets = sheet(20., r1, d1, 0.5) + &sheet(70., r2, d2, 0.5);
            format!(
                r##"<rect x="4.75" y="12.75" width="138.5" height="88.5" rx="11.5" fill="none" stroke="{col}" stroke-opacity="0.75" stroke-width="1.5"/>
<path d="M4.75 22 V12.5 Q4.75 3.75 13 3.75 H47 Q55 3.75 56 12.75" fill="none" stroke="{col}" stroke-opacity="0.75" stroke-width="1.5"/>
<g>{sheets}</g>
<g opacity="0.55"><g clip-path="url(#front)"><g filter="url(#frost)">{sheets}</g></g><rect x="0" y="38" width="148" height="70" rx="13" fill="url(#glass)"/></g>
<rect x="0.75" y="38.75" width="146.5" height="68.5" rx="12.25" fill="none" stroke="{col}" stroke-opacity="0.75" stroke-width="1.5"/>"##
            )
        }
        Art::New => format!(
            r##"<rect x="4.75" y="12.75" width="138.5" height="88.5" rx="11.5" fill="none" stroke="{line}" stroke-opacity="0.18" stroke-width="1.5"/>
<path d="M4.75 22 V12.5 Q4.75 3.75 13 3.75 H47 Q55 3.75 56 12.75" fill="none" stroke="{line}" stroke-opacity="0.18" stroke-width="1.5"/>
<rect x="0.5" y="38.5" width="147" height="69" rx="12.5" fill="{line}" fill-opacity="0.05" stroke="{line}" stroke-opacity="0.16"/>
<path d="M74 63 V83 M64 73 H84" stroke="{line}" stroke-opacity="0.45" stroke-width="1.6" stroke-linecap="round"/>"##
        ),
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="164" height="124" viewBox="-8 -4 164 124">
<defs>
<filter id="frost" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="4"/></filter>
<filter id="drop" x="-30%" y="-30%" width="160%" height="180%"><feGaussianBlur stdDeviation="7"/></filter>
<clipPath id="front"><rect x="0" y="38" width="148" height="70" rx="13"/></clipPath>
<linearGradient id="glass" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}" stop-opacity="0.8"/><stop offset="1" stop-color="{col}" stop-opacity="0.86"/></linearGradient>
</defs>
{body}
</svg>"##
    )
}

fn art_img(fc: u32, art: Art, lift: bool, dark: bool, scale: f32) -> Img {
    img(Arc::new(Image::from_bytes(ImageFormat::Svg, art_svg(fc, art, lift, dark).into_bytes()))).w(px(164. * scale)).h(px(124. * scale)).flex_none()
}

/// A folder as the page shows it, from core's streams and memory's notes.
pub(super) struct Folder {
    pub name: String,
    pub streams: Vec<backspace_core::chat::ThreadInfo>,
    pub notes: usize,
}

impl Folder {
    fn running(&self) -> usize {
        self.streams.iter().filter(|t| t.busy).count()
    }
    fn waiting(&self) -> usize {
        self.streams.iter().filter(|t| threads::needs_you(t)).count()
    }
    fn updated(&self) -> u64 {
        self.streams.iter().map(|t| t.updated).max().unwrap_or(0)
    }
    fn unfiled(&self) -> bool {
        self.name == threads::NO_FOLDER
    }
}

/// The folders, in the sidebar's order, with their notes counted by place.
pub(super) fn folders(list: &[backspace_core::chat::ThreadInfo], notes: &[(String, String)], filing: &threads::Filing) -> Vec<Folder> {
    threads::by_folder(list, "", filing)
        .into_iter()
        .map(|(name, ts)| Folder { notes: notes.iter().filter(|(_, p)| *p == name).count(), streams: ts.into_iter().cloned().collect(), name })
        .collect()
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

impl Cauldron {
    fn folder_list(&self) -> Vec<Folder> {
        match &self.threads {
            Threads::Ready(v) => folders(v, &self.side_data.notes, &self.filing),
            _ => Vec::new(),
        }
    }

    /// The open folder (B), or None for All folders (A).
    fn folder_open_sel(&self) -> Option<usize> {
        self.side_sel[AppPage::Folders as usize].and_then(|k| (1000..2000).contains(&k).then_some(k - 1000))
    }

    pub(super) fn folders_page(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let list = self.folder_list();
        let body = match self.folder_open_sel().and_then(|i| list.get(i).map(|f| (i, f))) {
            Some((i, f)) => self.folder_detail(i, f, pal, cx),
            None => self.folders_overview(&list, pal, cx),
        };
        div().id("page-body").size_full().overflow_y_scroll().child(body).test_support().into_any_element()
    }

    /// One row of the dots: running, needs you, notes.
    fn folder_counts(&self, f: &Folder, pal: &'static Palette) -> Div {
        let dot = |col: Hsla, hollow: bool| div().size(px(7.)).flex_none().rounded_full().when_live(hollow, |d| d.border_1().border_color(col)).when_live(!hollow, |d| d.bg(col));
        let item = |lead: AnyElement, n: usize| div().flex().items_center().gap(px(5.)).child(lead).child(n.to_string());
        div()
            .flex()
            .gap(px(10.))
            .mt(px(3.))
            .text_size(px(11.5))
            .text_color(c(pal.fg3))
            .child(item(dot(c(pal.accent), true).into_any_element(), f.running()))
            .child(item(dot(c(pal.accent), false).into_any_element(), f.waiting()))
            .child(item(glyph("database", 12., c(pal.fg3)).into_any_element(), f.notes))
    }

    /// A: the glass folders, the Librarian's proposal and New folder.
    fn folders_overview(&self, list: &[Folder], pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let dark = pal.panel.0 < 128;
        let now = now_ms();
        let mut grid = div().id("folder-grid").flex().flex_wrap().gap(px(8.));
        for (i, f) in list.iter().enumerate() {
            let lift = self.folder_hover == Some(i);
            let fc = self.filing.colour(&f.name);
            let mut shade = gpui_kit::black();
            shade.a = 0.24;
            let pill = |t: String| div().h(px(18.)).px(px(7.)).flex().items_center().rounded(px(9.)).bg(shade).text_size(px(10.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::white()).child(t);
            // On the glass, as the old faces and "Updated": how recent, and a dot when one needs you.
            let front = div()
                .absolute()
                .left(px(8. + 8.))
                .right(px(8. + 8.))
                .bottom(px(8. + 8.))
                .flex()
                .items_center()
                .justify_between()
                .child(div().when_live(f.waiting() > 0, |d| d.child(div().size(px(9.)).rounded_full().bg(c(pal.accent)).border_1().border_color(gpui_kit::white()))))
                .when_live(!f.unfiled() && f.updated() > 0, |d| d.child(pill(format!("Updated {} ago", threads::age(f.updated(), now)))));
            let sub = if f.unfiled() { format!("Not filed yet · {}", plural(f.streams.len(), "stream")) } else { plural(f.streams.len(), "stream") };
            grid = grid.child(
                div()
                    .id(("folder-card", i))
                    .w(px(176.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.))
                    .pt(px(6.))
                    .pb(px(12.))
                    .px(px(6.))
                    .rounded(px(16.))
                    .cursor_pointer()
                    .when_live(lift, |d| d.bg(c(pal.hover)).border_1().border_color(c(pal.line)))
                    .when_live(!lift, |d| d.border_1().border_color(gpui_kit::transparent_black()))
                    .aria_label(format!("{}, {}", f.name, sub))
                    .child(div().relative().child(art_img(fc, Art::Folder, lift, dark, 1.0)).child(front))
                    .child(div().mt(px(4.)).max_w_full().truncate().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(f.name.clone()))
                    .child(div().max_w_full().truncate().text_size(px(12.)).text_color(fg3).child(sub))
                    .when_live(!f.unfiled(), |d| d.child(self.folder_counts(f, pal)))
                    .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                        let next = if *on { Some(i) } else if this.folder_hover == Some(i) { None } else { this.folder_hover };
                        if next != this.folder_hover {
                            this.folder_hover = next;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.side_sel[AppPage::Folders as usize] = Some(1000 + i);
                        this.folder_hover = None;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        // The Librarian's proposal: outlined and see-through, an example until core has proposals.
        grid = grid.child(
            div()
                .id("folder-proposal")
                .w(px(176.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(2.))
                .pt(px(6.))
                .pb(px(12.))
                .aria_label("Site copy, proposed by the Librarian (example)")
                .child(art_img(0x5b8cf0, Art::Proposal, false, dark, 1.0))
                .child(div().mt(px(4.)).text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Site copy"))
                .child(div().text_size(px(12.)).text_color(fg3).child("3 streams proposed"))
                .child(div().mt(px(4.)).flex().gap(px(4.)).child(div().px(px(6.)).rounded(px(6.)).border_1().border_color(c(pal.accent)).text_size(px(10.5)).text_color(c(pal.accent)).child("Proposed by the Librarian")).child(tag("proposal-example", "Example", pal).test_support()))
                .test_support(),
        );
        let new_tile = div()
            .id("folder-new")
            .w(px(176.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .pt(px(6.))
            .pb(px(12.))
            .rounded(px(16.))
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .aria_label("New folder")
            .child(art_img(0, Art::New, false, dark, 1.0))
            .child(div().mt(px(4.)).text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg3).child("New folder"))
            .child(div().text_size(px(12.)).text_color(fg3).child("Name it yourself"))
            .on_click(cx.listener(|this, _, window, cx| this.ask_folder_name(None, false, window, cx)));
        grid = grid.child(new_tile.test_support());
        let stripe = div()
            .id("librarian-stripe")
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(14.))
            .bg(c(pal.inset))
            .child(pages::figure(3, cauldron_chrome::avatar::Mood::Reading, 22., false))
            .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(c(pal.fg2)).child("The Librarian proposes filing 3 streams and creating one folder, Site copy."))
            .child(tag("stripe-example", "Example", pal).test_support())
            .child(self.tipped("stripe-review", div().id("stripe-review").h(px(26.)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line)).text_size(px(12.)).text_color(c(pal.fg4)).aria_label("Review (demo: not built yet)").child("Review"), "Demo: not built yet", Side::Below, pal, cx).test_support());
        div()
            .w_full()
            .max_w(px(820.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(14.))
            .px(px(24.))
            .py(px(18.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(12.))
                    .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.)).child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Folders")).child(div().text_size(px(13.)).text_color(fg3).child("Where streams are filed. Each folder holds streams, notes and agents.")))
                    .child(self.workdir_button("file-by-workdir", pal, cx))
                    .child(self.page_button("folders-new", "New folder", true, pal).on_click(cx.listener(|this, _, window, cx| this.ask_folder_name(None, false, window, cx))).test_support()),
            )
            .child(stripe.test_support())
            .when_live(self.filing.folders.is_empty(), |d| {
                d.child(
                    div()
                        .id("folders-empty")
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .p(px(14.))
                        .rounded(px(14.))
                        .border_1()
                        .border_color(c(pal.line))
                        .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(c(pal.fg2)).child("No folders yet: every stream is Unfiled. Name a folder, or make one per working directory and file each stream in its own."))
                        .child(self.workdir_button("file-by-workdir-empty", pal, cx))
                        .test_support(),
                )
            })
            .child(grid.test_support())
            .child(div().text_size(px(11.5)).text_color(fg3).child("A folder files streams; it never changes where a stream runs. Right-click a stream to file it. The dots count what runs, what needs you, and the notes memory holds there."))
    }

    fn page_button(&self, id: &'static str, label: &'static str, primary: bool, pal: &'static Palette) -> Stateful<Div> {
        div()
            .id(id)
            .flex_none()
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .text_size(px(12.5))
            .font_weight(FontWeight::MEDIUM)
            .when_live(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)).hover(|d| d.opacity(0.9)))
            .when_live(!primary, |d| d.border_1().border_color(c(pal.line_strong)).text_color(c(pal.fg2)).hover(|d| d.bg(c(pal.hover))))
            .aria_label(label)
            .child(label)
    }

    /// "File by working directory": one explicit action, named for what it does.
    fn workdir_button(&self, id: &'static str, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let el = self.page_button(id, "File by working directory", false, pal).on_click(cx.listener(|this, _, _, cx| this.file_by_workdir(cx)));
        self.tipped((id, 1usize), el, "Makes a folder for each working directory and files each stream in its own. Runs only when you click it.", Side::Below, pal, cx).test_support().into_any_element()
    }

    /// B: one folder, its glass header, its streams, its agents (example) and its notes.
    fn folder_detail(&self, _i: usize, f: &Folder, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let dark = pal.panel.0 < 128;
        let now = now_ms();
        let head = |t: &'static str| div().pt(px(14.)).pb(px(4.)).text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(t);
        let mut streams = div().id("folder-streams").flex().flex_col();
        for (k, t) in f.streams.iter().enumerate() {
            let id = t.id.clone();
            let state = if t.busy { c(pal.accent) } else if threads::needs_you(t) { c(pal.accent) } else { c(pal.fg4) };
            streams = streams.child(
                div()
                    .id(("folder-stream", k))
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(8.))
                    .rounded(px(radius::TAG))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(t.title.clone())
                    .child(div().size(px(7.)).flex_none().rounded_full().when_live(t.busy, |d| d.border_1().border_color(state)).when_live(!t.busy, |d| d.bg(state)))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(fg).child(t.title.clone()))
                    .child(div().flex_none().text_size(px(11.5)).text_color(fg3).child(threads::age(t.updated, now)))
                    .child({
                        let id = t.id.clone();
                        div()
                            .id(("folder-stream-file", k))
                            .h(px(22.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .rounded(px(radius::TAG))
                            .border_1()
                            .border_color(c(pal.line))
                            .text_size(px(11.5))
                            .text_color(fg2)
                            .hover(|d| d.bg(c(pal.hover)))
                            .aria_label("File in…")
                            .child("File in…")
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                this.file_menu = Some((id.clone(), (f32::from(ev.position.x) - 200.0, f32::from(ev.position.y) + 12.0)));
                                cx.stop_propagation();
                                cx.notify();
                            }))
                            .test_support()
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_page(AppPage::Streams);
                        this.selected = Some(id.clone());
                        this.open_thread(id.clone());
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let meta = format!("{} · {} · a stream's working directory is set on the stream, not on the folder", plural(f.streams.len(), "stream"), plural(f.notes, "note"));
        // Unfiled is not a folder: nothing to rename or delete.
        let fid = self.filing.folders.iter().find(|x| x.name == f.name).map(|x| x.id.clone());
        let mut agents = div().flex().items_center().gap(px(6.));
        for d in [0usize, 1, 2] {
            agents = agents.child(pages::figure(d, cauldron_chrome::avatar::Mood::Idle, 22., false));
        }
        div()
            .w_full()
            .max_w(px(760.))
            .mx_auto()
            .flex()
            .flex_col()
            .px(px(24.))
            .py(px(18.))
            .child(
                div()
                    .id("folder-head")
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(art_img(self.filing.colour(&f.name), Art::Folder, false, dark, 0.72))
                    .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.)).child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(f.name.clone())).child(div().text_size(px(12.)).text_color(fg3).child(meta)))
                    .when_opt(fid.clone(), |d, id| {
                        let (id2, confirm) = (id.clone(), self.folder_delete.as_deref() == Some(id.as_str()));
                        d.child(self.page_button("folder-rename", "Rename", false, pal).on_click(cx.listener(move |this, _, window, cx| this.ask_folder_name(Some(id.clone()), false, window, cx))).test_support())
                            .child(
                                self.page_button("folder-delete", if confirm { "Delete: its streams become Unfiled" } else { "Delete" }, false, pal)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.folder_delete.as_deref() == Some(id2.as_str()) {
                                            this.delete_folder(&id2, cx);
                                        } else {
                                            this.folder_delete = Some(id2.clone());
                                            cx.notify();
                                        }
                                    }))
                                    .test_support(),
                            )
                    })
                    .test_support(),
            )
            .child(
                div()
                    .id("folder-proposal-stripe")
                    .mt(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(14.))
                    .bg(c(pal.inset))
                    .child(pages::figure(3, cauldron_chrome::avatar::Mood::Reading, 20., false))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(12.5)).text_color(fg2).child("Proposed for this folder: one unfiled stream"))
                    .child(tag("folder-proposal-example", "Example", pal).test_support())
                    .test_support(),
            )
            .child(head("Streams"))
            .child(streams.test_support())
            .child(head("Agents"))
            .child(div().flex().items_center().gap(px(8.)).child(agents).child(div().text_size(px(12.)).text_color(fg3).child("Agents per folder arrive with the Librarian (milestone 10)")).child(tag("folder-agents-example", "Example", pal).test_support()))
            .child(head("Library"))
            .child(
                div()
                    .id("folder-notes")
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(8.))
                    .rounded(px(radius::TAG))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(format!("{} for this folder", plural(f.notes, "note")))
                    .child(glyph("database", 14., fg2))
                    .child(div().flex_1().text_size(px(13.)).text_color(fg).child(format!("{} for this folder", plural(f.notes, "note"))))
                    .child(div().text_size(px(11.5)).text_color(fg3).child("Open in Library"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_page(AppPage::Library);
                        cx.notify();
                    }))
                    .test_support(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{art_svg, folders, Art};
    use crate::threads;

    /// Folders on core's API: everything starts Unfiled; "File by working directory" makes one per
    /// working directory and files each stream there; the menu files a stream elsewhere; rename
    /// and delete work, and a deleted folder's streams are Unfiled. Where a stream runs never moves.
    #[gpui_kit::test]
    fn folders_file_streams_without_moving_them(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, chats) = super::super::thread::tests::open(cx);
        let projects = |chats: &backspace_core::chat::Chats| {
            let mut v: Vec<(String, Option<String>)> = chats.list().into_iter().map(|t| (t.id.clone(), chats.peek(&t.id).unwrap().project)).collect();
            v.sort();
            v
        };
        let before = projects(&chats);
        handle.update(cx, |c, _, _| c.open_page(super::AppPage::Folders)).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("folders-empty").is_some(), "no folders: the empty state");
            window.click("file-by-workdir", cx);
            window.render_frame(cx);
        })
        .unwrap();
        let (names, filed) = handle.update(cx, |c, _, _| (c.filing.folders.iter().map(|f| f.name.clone()).collect::<Vec<_>>(), c.filing.of.len())).unwrap();
        assert!(names.contains(&"Cauldron".to_string()), "a folder per working directory: {names:?}");
        assert!(filed > 0);
        assert_eq!(projects(&chats), before, "filing never touches where a stream runs");
        // File one stream in another folder through the menu.
        let (sid, other) = handle
            .update(cx, |c, _, cx| {
                let t = match &c.threads {
                    super::Threads::Ready(v) => v[0].id.clone(),
                    _ => unreachable!(),
                };
                c.folders_api().unwrap().create("Launch").unwrap();
                c.filing_changed(cx);
                (t, c.filing.folders.iter().position(|f| f.name == "Launch").unwrap())
            })
            .unwrap();
        handle.update(cx, |c, _, _| c.file_menu = Some((sid.clone(), (100.0, 100.0)))).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("file-opt", other), cx);
            window.render_frame(cx);
        })
        .unwrap();
        let launch = handle.update(cx, |c, _, _| c.filing.folders[other].id.clone()).unwrap();
        assert_eq!(chats.peek(&sid).unwrap().folder.as_deref(), Some(launch.as_str()));
        assert_eq!(projects(&chats), before);
        // Rename, then delete: its stream is Unfiled again.
        handle
            .update(cx, |c, _, cx| {
                c.folders_api().unwrap().rename(&launch, "Launch week").unwrap();
                c.filing_changed(cx);
                assert!(c.filing.folders.iter().any(|f| f.name == "Launch week"));
                c.delete_folder(&launch, cx);
                assert!(!c.filing.folders.iter().any(|f| f.id == launch));
                let t = match &c.threads {
                    super::Threads::Ready(v) => v.iter().find(|t| t.id == sid).cloned().unwrap(),
                    _ => unreachable!(),
                };
                assert!(c.filing.folder_of(&t).is_none(), "a deleted folder's stream is Unfiled");
            })
            .unwrap();
        assert_eq!(projects(&chats), before);
    }

    #[test]
    fn folders_count_streams_and_notes_by_folder() {
        let list = threads::fixture(1_700_000_000_000);
        let notes = vec![("a".to_string(), "Cauldron".to_string()), ("b".into(), "Cauldron".into()), ("c".into(), "Global".into())];
        let fs = folders(&list, &notes, &threads::Filing::by_workdir(&list));
        assert!(!fs.is_empty());
        let total: usize = fs.iter().map(|f| f.streams.len()).sum();
        assert_eq!(total, list.len(), "every stream in one folder");
        if let Some(c) = fs.iter().find(|f| f.name == "Cauldron") {
            assert_eq!(c.notes, 2);
        }
        if let Some(at) = fs.iter().position(|f| f.unfiled()) {
            assert_eq!(at, fs.len() - 1, "Unfiled last");
        }
    }

    /// Each kind of art is one picture; its glass and colour are in it.
    #[test]
    fn the_art_is_a_glass_folder() {
        let a = art_svg(0x8b5cf6, Art::Folder, false, true);
        assert!(a.contains("feGaussianBlur") && a.contains("#8b5cf6") && a.contains("url(#glass)"), "a frosted front over the sheets");
        assert_ne!(a, art_svg(0x8b5cf6, Art::Folder, true, true), "the sheets lift on hover");
        assert!(art_svg(0x5b8cf0, Art::Proposal, false, true).contains("stroke=\"#5b8cf0\""), "a proposal is outlined");
    }
}

/// The small dialog that names a folder: a new one (for the chip too), or a rename.
pub(crate) struct FolderPrompt {
    pub input: Entity<InputState>,
    _sub: Subscription,
    /// None: a new folder; Some(id): renaming that one.
    pub rename: Option<String>,
    /// A new folder made from the New stream chip is chosen there.
    pub for_chip: bool,
    pub error: Option<String>,
}

impl Cauldron {
    /// Core's folders, on the data folder core was opened on; None before core opens.
    pub(super) fn folders_api(&self) -> Option<backspace_core::folders::Folders> {
        let (_, chats) = self.core.as_ref()?;
        Some(backspace_core::folders::Folders::open(self.data_dir(), chats.clone()))
    }

    /// The data folder core reads: the one it was opened on, else core's own.
    pub(super) fn data_dir(&self) -> std::path::PathBuf {
        self.data_dir.clone().unwrap_or_else(Prefs::data_dir)
    }

    /// Reads where each stream is filed again when the streams or the folders changed. Without
    /// core (the screenshot fixtures) the filing stays as it was set.
    pub(super) fn refresh_filing(&mut self) {
        let Threads::Ready(v) = &self.threads else { return };
        let key = (v.len(), v.iter().map(|t| t.updated).fold(0u64, |a, u| a.wrapping_add(u)), self.filing_rev);
        if self.filing_key == Some(key) {
            return;
        }
        self.filing_key = Some(key);
        let Some(api) = self.folders_api() else { return };
        let mut f = threads::Filing::default();
        for x in api.list() {
            for t in api.threads_in(&x.id) {
                f.of.insert(t.id, x.id.clone());
            }
            let colour = u32::from_str_radix(x.color.trim_start_matches('#'), 16).unwrap_or(0x8b5cf6);
            f.folders.push(threads::FolderRef { id: x.id, name: x.name, colour });
        }
        self.filing = f;
    }

    fn filing_changed(&mut self, cx: &mut Context<Self>) {
        self.filing_rev += 1;
        self.refresh_filing();
        cx.notify();
    }

    /// Opens the name dialog.
    pub(super) fn ask_folder_name(&mut self, rename: Option<String>, for_chip: bool, window: &mut Window, cx: &mut Context<Self>) {
        let current = rename.as_ref().and_then(|id| self.filing.folders.iter().find(|f| &f.id == id)).map(|f| f.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Folder name"));
        st.update(cx, |s, cx| s.set_value(current, window, cx));
        let sub = cx.subscribe(&st, |this, _, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.folder_prompt_ok = true;
                cx.notify();
            }
        });
        st.read(cx).focus_handle(cx).focus(window, cx);
        self.folder_prompt = Some(FolderPrompt { input: st, _sub: sub, rename, for_chip, error: None });
        cx.notify();
    }

    /// Creates or renames from the dialog; a bad name keeps it open with core's reason.
    pub(super) fn submit_folder_name(&mut self, cx: &mut Context<Self>) {
        let Some(p) = &self.folder_prompt else { return };
        let name = p.input.read(cx).value().to_string();
        let Some(api) = self.folders_api() else {
            if let Some(p) = self.folder_prompt.as_mut() {
                p.error = Some("Core is not open yet.".into());
            }
            return;
        };
        let done = match &p.rename {
            Some(id) => api.rename(id, &name).map(|f| f.id),
            None => api.create(&name).map(|f| f.id),
        };
        match done {
            Ok(id) => {
                if p.for_chip {
                    self.start.folder = Some(Some(id));
                }
                self.folder_prompt = None;
                self.filing_changed(cx);
            }
            Err(e) => {
                if let Some(p) = self.folder_prompt.as_mut() {
                    p.error = Some(e.to_string());
                }
                cx.notify();
            }
        }
    }

    /// Deletes a folder; its streams become Unfiled (core takes them out).
    pub(super) fn delete_folder(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(api) = self.folders_api() {
            let _ = api.delete(id);
            self.side_sel[AppPage::Folders as usize] = None;
            self.folder_delete = None;
            self.filing_changed(cx);
        }
    }

    /// Files a stream in a folder, or takes it out with None. Where it runs is not touched.
    pub(super) fn file_stream(&mut self, thread: &str, folder: Option<&str>, cx: &mut Context<Self>) {
        if let Some(api) = self.folders_api() {
            if let Err(e) = api.set_thread_folder(thread, folder) {
                self.send_error = Some(format!("Couldn't file it: {e}"));
            }
            self.file_menu = None;
            self.filing_changed(cx);
        }
    }

    /// "File by working directory": a folder per working-directory name (made if missing), each
    /// stream with one filed there. Only when asked; it is what it says, not a suggestion.
    pub(super) fn file_by_workdir(&mut self, cx: &mut Context<Self>) {
        let (Some(api), Threads::Ready(v)) = (self.folders_api(), &self.threads) else { return };
        let plan = threads::Filing::by_workdir(v);
        let mut ids: HashMap<String, String> = api.list().into_iter().map(|f| (f.name.to_lowercase(), f.id)).collect();
        for f in &plan.folders {
            if !ids.contains_key(&f.name.to_lowercase()) {
                if let Ok(made) = api.create(&f.name) {
                    ids.insert(f.name.to_lowercase(), made.id);
                }
            }
        }
        for t in v {
            if let Some(name) = plan.name_of(t) {
                if let Some(id) = ids.get(&name.to_lowercase()) {
                    let _ = api.set_thread_folder(&t.id, Some(id));
                }
            }
        }
        self.filing_changed(cx);
    }

    /// The name dialog over a scrim.
    pub(super) fn folder_prompt_overlay(&self, w: f32, pal: &'static Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let p = self.folder_prompt.as_ref()?;
        let title = if p.rename.is_some() { "Rename folder" } else { "New folder" };
        let button = |id: &'static str, t: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(28.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .when_live(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)))
                .when_live(!primary, |d| d.border_1().border_color(c(pal.line_strong)).text_color(c(pal.fg2)))
                .aria_label(t)
                .child(t)
        };
        let bw = 380f32.min(w - 32.0);
        let mut scrim = gpui_kit::black();
        scrim.a = 0.3;
        Some(
            div()
                .id("folder-prompt-scrim")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(scrim)
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.folder_prompt = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("folder-prompt")
                        .absolute()
                        .left(px((w - bw) / 2.0))
                        .top(px(140.))
                        .w(px(bw))
                        .p(px(16.))
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .rounded(px(14.))
                        .bg(c(pal.raised))
                        .border_1()
                        .border_color(c(pal.line_strong))
                        .shadow_lg()
                        .aria_label(title)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                            if ev.keystroke.key == "escape" {
                                this.folder_prompt = None;
                                cx.notify();
                            }
                        }))
                        .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg)).child(title))
                        .child(div().h(px(32.)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong)).child(Input::new(&p.input).appearance(false).text_size(px(13.)).p_0()))
                        .when_opt(p.error.clone(), |d, e| d.child(div().id("folder-prompt-error").text_size(px(12.)).text_color(c(pal.bad)).child(e).test_support()))
                        .child(div().text_size(px(11.5)).text_color(c(pal.fg3)).child("A folder files streams. It never changes where a stream runs."))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(button("folder-prompt-cancel", "Cancel", false).on_click(cx.listener(|this, _, _, cx| {
                                    this.folder_prompt = None;
                                    cx.notify();
                                })).test_support())
                                .child(button("folder-prompt-ok", if p.rename.is_some() { "Rename" } else { "Create" }, true).on_click(cx.listener(|this, _, _, cx| this.submit_folder_name(cx))).test_support()),
                        )
                        .test_support(),
                ),
        )
    }

    /// The filing menu for one stream: each folder, Unfiled, New folder.
    pub(super) fn file_menu_overlay(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let (thread, (x, y)) = self.file_menu.clone()?;
        let current = match &self.threads {
            Threads::Ready(v) => v.iter().find(|t| t.id == thread).and_then(|t| self.filing.folder_of(t)).map(|f| f.id.clone()),
            _ => None,
        };
        let item = |id: ElementId, label: String| div().id(id).h(px(28.)).flex().items_center().gap(px(8.)).px(px(8.)).rounded(px(radius::INNER)).cursor_pointer().hover(|d| d.bg(c(pal.hover))).text_size(px(12.5)).text_color(c(pal.fg2)).aria_label(label.clone());
        let mut menu = div()
            .id("file-menu")
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(220.))
            .p(px(4.))
            .flex()
            .flex_col()
            .rounded(px(radius::CONTROL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.file_menu = None;
                cx.notify();
            }))
            .child(div().px(px(8.)).py(px(4.)).text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg3)).child("File in"));
        for (i, f) in self.filing.folders.iter().enumerate() {
            let (t, fid) = (thread.clone(), f.id.clone());
            let on = current.as_deref() == Some(f.id.as_str());
            menu = menu.child(
                item(("file-opt", i).into(), f.name.clone())
                    .when_live(on, |d| d.bg(c(pal.selected)))
                    .child(glyph("folder", 13., c(Rgb::hex(f.colour))))
                    .child(div().flex_1().min_w_0().truncate().child(f.name.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| this.file_stream(&t, Some(&fid), cx)))
                    .test_support(),
            );
        }
        let t2 = thread.clone();
        menu = menu
            .child(
                item("file-none".into(), threads::NO_FOLDER.into())
                    .when_live(current.is_none(), |d| d.bg(c(pal.selected)))
                    .child(glyph("x", 13., c(pal.fg3)))
                    .child(threads::NO_FOLDER)
                    .on_click(cx.listener(move |this, _, _, cx| this.file_stream(&t2, None, cx)))
                    .test_support(),
            )
            .child(
                item("file-new".into(), "New folder".into())
                    .child(glyph("plus", 13., c(pal.fg3)))
                    .child("New folder")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.file_menu = None;
                        this.ask_folder_name(None, false, window, cx);
                    }))
                    .test_support(),
            );
        Some(menu)
    }
}
