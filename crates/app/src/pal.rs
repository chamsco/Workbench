//! The design tokens of design/workbench.html, value for value, so the GPUI
//! and Tauri builds draw the same pixels from the same numbers.

use gpui_kit::{rgb, rgba, Hsla};

#[derive(Clone, Copy)]
pub struct Pal {
    pub desk_base: Hsla,
    pub blobs: [Hsla; 3],
    pub glass: Hsla,
    pub glass_side: Hsla,
    pub glass_main: Hsla,
    pub pane: Hsla,
    pub pane_edge: Hsla,
    pub pane_edge_on: Hsla,
    pub fg: Hsla,
    pub fg2: Hsla,
    pub fg3: Hsla,
    pub fg4: Hsla,
    pub hover: Hsla,
    pub sel: Hsla,
    pub pill: Hsla,
    pub pill_on: Hsla,
    pub pill_edge: Hsla,
    pub accent: Hsla,
    pub blue: Hsla,
    pub green: Hsla,
    pub amber: Hsla,
    pub red: Hsla,
    pub violet: Hsla,
    pub term_fg: Hsla,
    pub term_dim: Hsla,
    pub prompt_bg: Hsla,
    pub prompt_fg: Hsla,
    pub node: Hsla,
    pub grid_dot: Hsla,
    pub page_bg: Hsla,
    pub page_fg: Hsla,
    pub page_dim: Hsla,
    pub page_line: Hsla,
    pub page_accent: Hsla,
    pub page_desk: Hsla,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// `rgba(r, g, b, a)` from the stylesheet.
fn ca(r: u8, g: u8, b: u8, a: f32) -> Hsla {
    let a = (a * 255.).round() as u32;
    rgba((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | a).into()
}

impl Pal {
    pub fn light() -> Self {
        Self {
            desk_base: c(0xefe3ec),
            blobs: [c(0xff9ec2), c(0xffb27d), c(0xa99bff)],
            glass: ca(252, 249, 252, 0.56),
            glass_side: ca(255, 255, 255, 0.26),
            glass_main: ca(246, 243, 247, 0.5),
            pane: ca(255, 255, 255, 0.84),
            pane_edge: ca(24, 18, 30, 0.10),
            pane_edge_on: ca(24, 18, 30, 0.30),
            fg: c(0x1c1a1f),
            fg2: c(0x4e4955),
            fg3: c(0x7d7785),
            fg4: c(0xa8a2ae),
            hover: ca(30, 20, 40, 0.05),
            sel: ca(30, 20, 40, 0.085),
            pill: ca(255, 255, 255, 0.5),
            pill_on: ca(255, 255, 255, 0.95),
            pill_edge: ca(24, 18, 30, 0.12),
            accent: c(0xcf5a24),
            blue: c(0x2a66d9),
            green: c(0x187f45),
            amber: c(0x9a5c00),
            red: c(0xbf3a2f),
            violet: c(0x6a4fd6),
            term_fg: c(0x2a2630),
            term_dim: c(0x78727f),
            prompt_bg: c(0xe8e5eb),
            prompt_fg: c(0x1c1a1f),
            node: ca(255, 255, 255, 0.95),
            grid_dot: ca(30, 20, 40, 0.13),
            page_bg: c(0xffffff),
            page_fg: c(0x1b1a1f),
            page_dim: c(0x6f6a76),
            page_line: c(0xebe8ee),
            page_accent: c(0x5b47d6),
            page_desk: ca(236, 233, 239, 0.75),
        }
    }

    pub fn dark() -> Self {
        Self {
            desk_base: c(0x160f17),
            blobs: [c(0x8a2f5e), c(0x8c4524), c(0x3b2f8f)],
            glass: ca(36, 30, 38, 0.58),
            glass_side: ca(70, 58, 72, 0.2),
            glass_main: ca(20, 17, 22, 0.48),
            pane: ca(17, 15, 19, 0.8),
            pane_edge: ca(255, 255, 255, 0.08),
            pane_edge_on: ca(255, 255, 255, 0.24),
            fg: c(0xece8ef),
            fg2: c(0xbdb6c3),
            fg3: c(0x8a8390),
            fg4: c(0x5f5965),
            hover: ca(255, 255, 255, 0.05),
            sel: ca(255, 255, 255, 0.09),
            pill: ca(255, 255, 255, 0.06),
            pill_on: ca(255, 255, 255, 0.16),
            pill_edge: ca(255, 255, 255, 0.12),
            accent: c(0xf08a5d),
            blue: c(0x7aa8ff),
            green: c(0x56d18f),
            amber: c(0xf2b24c),
            red: c(0xff7a6e),
            violet: c(0xa99bff),
            term_fg: c(0xe6e2ea),
            term_dim: c(0x8f8896),
            prompt_bg: c(0xe8e5ea),
            prompt_fg: c(0x141216),
            node: ca(28, 25, 31, 0.95),
            grid_dot: ca(255, 255, 255, 0.08),
            page_bg: c(0x17161b),
            page_fg: c(0xecebf0),
            page_dim: c(0x9a96a3),
            page_line: c(0x2a2830),
            page_accent: c(0x9d8cff),
            page_desk: ca(10, 9, 12, 0.5),
        }
    }
}
