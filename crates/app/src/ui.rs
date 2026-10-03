//! Drawing primitives shared by the workbench and its canvases: fonts,
//! hairlines, icons, dots, buttons, the mascot, the desk, transcript lines,
//! and the dev-server text snapshot.

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use backspace_core::{AgentRecord, AgentStatus, LogKind, ProjectState, MAIN};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::icons;
use crate::pal::Pal;

/// gpui takes one family name, not a CSS stack: the system faces on macOS
/// (SF Pro, Menlo) and Windows (Segoe UI); elsewhere the bundled Inter (the
/// closest open match to SF Pro) and JetBrains Mono, which the Tauri shell
/// bundles too.
pub(crate) const MONO: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "JetBrains Mono"
};
pub(crate) const UI: &str = if cfg!(target_os = "macos") {
    ".SystemUIFont"
} else if cfg!(target_os = "windows") {
    "Segoe UI"
} else {
    "Inter"
};

/// The faces registered at startup (shared with crates/tauri-app/ui/fonts).
pub(crate) fn bundled_fonts() -> Vec<std::borrow::Cow<'static, [u8]>> {
    macro_rules! font {
        ($f:literal) => {
            std::borrow::Cow::Borrowed(
                &include_bytes!(concat!("../../tauri-app/ui/fonts/", $f, ".ttf"))[..],
            )
        };
    }
    let mut v = vec![
        font!("JetBrainsMono-Regular"),
        font!("JetBrainsMono-Medium"),
        font!("JetBrainsMono-Bold"),
        font!("JetBrainsMono-Italic"),
    ];
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        v.extend([
            font!("Inter-Regular"),
            font!("Inter-Medium"),
            font!("Inter-SemiBold"),
            font!("Inter-Bold"),
            font!("Inter-Italic"),
        ]);
    }
    v
}

/// Width of two monospace cells at 12px, for hanging indents.
pub(crate) const CH2: f32 = 14.4;

/// macOS blurs what is behind a translucent window; elsewhere the replica's
/// desk (base colour plus soft blobs) is painted behind the glass instead.
pub(crate) const GLASS: bool = cfg!(target_os = "macos");

pub(crate) const SPIN: [&str; 10] = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];

/// What a dev-server URL answered, for the Browser environment.
pub(crate) struct Page {
    pub(crate) status: String,
    pub(crate) title: String,
    pub(crate) text: Vec<String>,
    pub(crate) ms: u128,
}

// ------------------------------------------------------------------ primitives

/// The replica's 0.5px hairline: one device pixel.
pub(crate) fn hair(window: &Window) -> Pixels {
    px(1. / window.scale_factor().max(1.))
}

pub(crate) trait Hair: Styled + Sized {
    fn hair_all(mut self, w: Pixels, c: Hsla) -> Self {
        let s = self.style();
        s.border_widths.top = Some(w.into());
        s.border_widths.right = Some(w.into());
        s.border_widths.bottom = Some(w.into());
        s.border_widths.left = Some(w.into());
        self.border_color(c)
    }
    fn hair_b(mut self, w: Pixels, c: Hsla) -> Self {
        self.style().border_widths.bottom = Some(w.into());
        self.border_color(c)
    }
    fn hair_t(mut self, w: Pixels, c: Hsla) -> Self {
        self.style().border_widths.top = Some(w.into());
        self.border_color(c)
    }
}
impl<T: Styled> Hair for T {}

pub(crate) fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(icons::path(name))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// `.ib`: a 24px square button around `content`.
pub(crate) fn ib_with(
    id: impl Into<ElementId>,
    content: impl IntoElement,
    p: &Pal,
    on: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .text_color(p.fg3)
        .when(on, |d| d.bg(p.sel))
        .hover(|d| d.bg(p.hover))
        .child(content)
}

/// `.ib` with one of the line icons.
pub(crate) fn ib(id: impl Into<ElementId>, name: &str, p: &Pal, on: bool) -> Stateful<Div> {
    ib_with(id, icon(name, 14., if on { p.fg } else { p.fg3 }), p, on)
}

/// `.dot`, with the CSS pulse (opacity 1 → 0.35 → 1 over 1.4s) for running.
pub(crate) fn dot(c: Hsla, pulse: Option<f32>) -> Div {
    let a = pulse.map_or(1., |t| {
        let ph = (t % 1.4) / 1.4;
        1. - 0.65 * (1. - (2. * ph - 1.).abs())
    });
    div()
        .size(px(6.))
        .flex_none()
        .rounded_full()
        .bg(c.opacity(c.a * a))
}

pub(crate) fn status_dot(p: &Pal, s: AgentStatus, t: f32) -> Div {
    match s {
        AgentStatus::Running => dot(p.accent, Some(t)),
        AgentStatus::AwaitingApproval => dot(p.amber, None),
        AgentStatus::Approved => dot(p.green, None),
        AgentStatus::Failed => dot(p.red, None),
        _ => dot(p.fg4, None),
    }
}

/// Text with per-span colours (StyledText highlights).
pub(crate) fn rich(parts: &[(&str, Option<Hsla>)]) -> StyledText {
    let mut s = String::new();
    let mut hl = vec![];
    for (t, c) in parts {
        let start = s.len();
        s.push_str(t);
        if let Some(c) = c {
            hl.push((
                start..s.len(),
                HighlightStyle {
                    color: Some(*c),
                    ..Default::default()
                },
            ));
        }
    }
    StyledText::new(s).with_highlights(hl)
}

pub(crate) fn route(a: &AgentRecord) -> String {
    let now = a.decision.as_ref().map_or("not routed".to_string(), |d| {
        format!("{} @ {}", d.model, d.effort)
    });
    match a.escalations.first().and_then(|e| e.split(" → ").next()) {
        Some(start) => format!("{start} → {now} ↑{}", a.escalations.len()),
        None => now,
    }
}

pub(crate) fn btn(id: impl Into<ElementId>, label: &str, p: &Pal, w: Pixels) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(14.))
        .flex()
        .items_center()
        .rounded(px(7.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .bg(p.pill_on)
        .hair_all(w, p.pane_edge)
        .child(label.to_string())
}

/// The pixel mascot: 12x7 cells of 4px, accent coloured.
pub(crate) fn mascot(c: Hsla) -> impl IntoElement {
    const ROWS: [&str; 7] = [
        "...XXXXXXXXX",
        "..XXXXXXXXXX",
        ".XXXX.XX.XXX",
        "XXXXXXXXXXXX",
        ".XXXXXXXXXXX",
        "..XX.XXXX.XX",
        "...XXXXXXXXX",
    ];
    canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            for (y, row) in ROWS.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    if ch == 'X' {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(
                                    b.origin.x + px(x as f32 * 4.),
                                    b.origin.y + px(y as f32 * 4.),
                                ),
                                size(px(4.), px(4.)),
                            ),
                            c,
                        ));
                    }
                }
            }
        },
    )
    .w(px(48.))
    .h(px(28.))
    .flex_none()
}

/// The desk behind the glass: the replica's three `blur(70px)` blobs over
/// the base colour, rendered once per window size at quarter resolution
/// (blurred content loses nothing) and drawn as a single image, so frames
/// do not re-rasterise it.
pub(crate) fn desk_image(p: &Pal, w: f32, h: f32) -> Arc<Image> {
    const SCALE: f32 = 4.;
    let (iw, ih) = (
        (w / SCALE).ceil().max(1.) as usize,
        (h / SCALE).ceil().max(1.) as usize,
    );
    let vw = w / 100.;
    let rgb = |c: Hsla| {
        let c = c.to_rgb();
        [c.r, c.g, c.b]
    };
    // (diameter, left, top) in vw, from .blob.a/.b/.c; c hangs off the bottom.
    let blobs = [
        (52., -10., Some(-14.), rgb(p.blobs[0])),
        (46., 100. - 12. - 46., Some(-6.), rgb(p.blobs[1])),
        (60., 26., None, rgb(p.blobs[2])),
    ];
    let base = rgb(p.desk_base);
    let sigma = 70.;
    // 24-bit BMP, bottom-up rows padded to 4 bytes.
    let row = (iw * 3 + 3) & !3;
    let mut bmp = Vec::with_capacity(54 + row * ih);
    let size = (54 + row * ih) as u32;
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&size.to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0]);
    bmp.extend_from_slice(&(iw as i32).to_le_bytes());
    bmp.extend_from_slice(&(ih as i32).to_le_bytes());
    bmp.extend_from_slice(&[1, 0, 24, 0]);
    bmp.extend_from_slice(&[0; 24]);
    for y in (0..ih).rev() {
        let py = (y as f32 + 0.5) * SCALE;
        let mut line = Vec::with_capacity(row);
        for x in 0..iw {
            let px_ = (x as f32 + 0.5) * SCALE;
            let mut c = base;
            for (d, l, t, bc) in &blobs {
                let r = d * vw / 2.;
                let cx0 = l * vw + r;
                let cy0 = match t {
                    Some(t) => t * vw + r,
                    None => h + 34. * vw - r,
                };
                let dist = ((px_ - cx0).powi(2) + (py - cy0).powi(2)).sqrt();
                // A Gaussian-blurred disc's edge: ~ 0.5 * erfc((d - r) / (σ√2)).
                let a = 0.85 * 0.5 * (1. - ((dist - r) / sigma * 1.13).tanh());
                for i in 0..3 {
                    c[i] = c[i] * (1. - a) + bc[i] * a;
                }
            }
            for i in [2, 1, 0] {
                line.push((c[i].clamp(0., 1.) * 255.).round() as u8);
            }
        }
        line.resize(row, 0);
        bmp.extend_from_slice(&line);
    }
    Arc::new(Image::from_bytes(ImageFormat::Bmp, bmp))
}

pub(crate) fn chip_f(p: &Pal, w: Pixels, title: bool) -> Div {
    div()
        .text_size(px(11.5))
        .px(px(9.))
        .py(px(3.))
        .rounded_full()
        .bg(p.glass)
        .hair_all(w, p.pane_edge)
        .when(title, |d| d.font_weight(FontWeight::SEMIBOLD))
}

pub(crate) fn nothing(p: &Pal, s: &str) -> AnyElement {
    div()
        .p(px(14.))
        .text_size(px(12.5))
        .text_color(p.fg3)
        .child(s.to_string())
        .into_any_element()
}

pub(crate) fn empty_mark(p: &Pal, ic: &str, s: &str) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(26.))
        .child(
            div()
                .size(px(54.))
                .rounded(px(14.))
                .bg(p.hover)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(ic, 26., p.fg3)),
        )
        .child(nothing(p, s))
}

pub(crate) fn h1(p: &Pal, s: &str) -> AnyElement {
    div()
        .text_size(px(26.))
        .line_height(relative(1.2))
        .font_weight(FontWeight::BOLD)
        .mb(px(4.))
        .text_color(p.fg)
        .child(s.to_string())
        .into_any_element()
}

pub(crate) fn para(p: &Pal, s: &str) -> AnyElement {
    div()
        .my(px(10.))
        .text_color(p.fg)
        .child(s.to_string())
        .into_any_element()
}

pub(crate) fn note(p: &Pal, s: &str) -> AnyElement {
    hang("※ ", div().italic().child(s.to_string()), p.term_dim).into_any_element()
}

/// A line with a two-cell glyph gutter (CSS `padding-left: 2ch; text-indent: -2ch`).
pub(crate) fn hang(glyph: &str, body: impl IntoElement, c: Hsla) -> Div {
    div()
        .flex()
        .items_start()
        .text_color(c)
        .child(
            div()
                .w(px(CH2))
                .flex_none()
                .whitespace_nowrap()
                .child(glyph.trim_end().to_string()),
        )
        .child(div().flex_1().min_w_0().child(body))
}

pub(crate) fn log_el(kind: LogKind, text: &str, p: &Pal) -> AnyElement {
    // `white-space: pre-wrap` drops a block's final newline; gpui would draw it.
    let text = text.strip_suffix('\n').unwrap_or(text);
    match kind {
        LogKind::User => div()
            .px(px(6.))
            .rounded(px(2.))
            .bg(p.prompt_bg)
            .text_color(p.prompt_fg)
            .font_weight(FontWeight::MEDIUM)
            .child(format!("❯ {text}"))
            .into_any_element(),
        LogKind::Assistant => hang("● ", text.to_string(), p.term_fg).into_any_element(),
        LogKind::ToolCall => {
            let (name, rest) = text.split_once(' ').unwrap_or((text, ""));
            hang(
                "⎿ ",
                rich(&[(name, Some(p.term_fg)), (&format!(" {rest}"), None)]),
                p.term_dim,
            )
            .into_any_element()
        }
        LogKind::ToolResult => {
            let lines: Vec<&str> = text.lines().collect();
            let body = if lines.len() > 6 {
                format!(
                    "{}\n… {} more lines",
                    lines[..6].join("\n"),
                    lines.len() - 6
                )
            } else {
                text.to_string()
            };
            if text.starts_with("CHECK FAILED") || text.starts_with("REJECTED") {
                hang("✗ ", body, p.red).into_any_element()
            } else if text.starts_with("Approved")
                || text.starts_with("Plan APPROVED")
                || text.starts_with("Merged")
            {
                hang("✓ ", body, p.green).into_any_element()
            } else {
                div()
                    .pl(px(CH2))
                    .text_color(p.term_dim)
                    .child(body)
                    .into_any_element()
            }
        }
        LogKind::System if text.starts_with("escalated") => {
            hang("↑ ", text.to_string(), p.amber).into_any_element()
        }
        LogKind::System => note(p, text),
        LogKind::Error => hang("✗ ", text.to_string(), p.red).into_any_element(),
    }
}

/// (margin-top, margin-bottom) of each transcript line, from the stylesheet.
pub(crate) fn log_margins(kind: LogKind, text: &str) -> (f32, f32) {
    match kind {
        LogKind::User => (8., 8.),
        LogKind::Assistant => (8., 0.),
        LogKind::ToolCall => (6., 0.),
        LogKind::ToolResult
            if text.starts_with("Approved")
                || text.starts_with("Plan APPROVED")
                || text.starts_with("Merged") =>
        {
            (6., 0.)
        }
        LogKind::System if text.starts_with("escalated") => (6., 0.),
        LogKind::System => (8., 0.),
        _ => (0., 0.),
    }
}

/// CSS `white-space: pre-line`: keep line breaks, collapse runs of spaces.
pub(crate) fn pre_line(s: &str) -> String {
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Depth-first, so each agent sits directly under its manager.
pub(crate) fn tree_order(s: &ProjectState) -> Vec<usize> {
    fn walk(s: &ProjectState, id: usize, out: &mut Vec<usize>) {
        out.push(id);
        for child in s.agents.iter().filter(|a| a.parent == Some(id)) {
            walk(s, child.id, out);
        }
    }
    let mut out = Vec::new();
    if !s.agents.is_empty() {
        walk(s, MAIN, &mut out);
    }
    out
}

/// A plain-HTTP GET of a dev server, reduced to its title and visible text.
pub(crate) fn probe(url: &str) -> Result<Page, String> {
    let start = Instant::now();
    let rest = url.strip_prefix("http://").ok_or(
        "Only http:// dev servers can be previewed here; open https pages in your browser.",
    )?;
    let (host, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_string()), |(h, p)| (h, format!("/{p}")));
    let addr = if host.contains(':') {
        host.to_string()
    } else {
        format!("{host}:80")
    };
    let mut s = std::net::TcpStream::connect(&addr).map_err(|e| format!("{addr}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    write!(
        s,
        "GET {path} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: backspace\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    s.take(2 << 20)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&buf).into_owned();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_once(' '))
        .map_or(String::new(), |(_, s)| format!("HTTP {s}"));
    // Lowercasing ASCII keeps byte offsets aligned with `body`.
    let lower = body.to_ascii_lowercase();
    let title = lower
        .find("<title>")
        .and_then(|i| {
            lower[i..]
                .find("</title>")
                .map(|j| body[i + 7..i + j].trim().to_string())
        })
        .or_else(|| {
            let i = lower.find("<h1")?;
            let gt = lower[i..].find('>')? + i + 1;
            let end = lower[gt..].find("</h1>")? + gt;
            Some(strip_tags(&body[gt..end]))
        })
        .unwrap_or_else(|| url.to_string());
    // Visible text: drop scripts, styles and the title; break on block tags.
    let mut text = String::new();
    let mut i = 0;
    while i < body.len() {
        if body.as_bytes()[i] == b'<' {
            let end = lower[i..].find('>').map_or(body.len(), |j| i + j + 1);
            let tag = &lower[i..end];
            let mut next = end;
            for skip in ["script", "style", "title"] {
                if tag.starts_with(&format!("<{skip}")) {
                    let close = format!("</{skip}>");
                    next = lower[end..]
                        .find(&close)
                        .map_or(body.len(), |j| end + j + close.len());
                }
            }
            let block = [
                "<p", "<li", "<div", "<h", "<br", "<tr", "</p", "</li", "</div", "</h", "<ul",
                "</ul",
            ];
            if block.iter().any(|t| tag.starts_with(t)) {
                text.push('\n');
            }
            i = next;
        } else {
            let next = lower[i..].find('<').map_or(body.len(), |j| i + j);
            text.push_str(&body[i..next]);
            i = next;
        }
    }
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|l| {
            l.replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&nbsp;", " ")
        })
        .filter(|l| !l.is_empty() && *l != title)
        .take(200)
        .collect();
    Ok(Page {
        status,
        title,
        text: lines,
        ms: start.elapsed().as_millis(),
    })
}

pub(crate) fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}
