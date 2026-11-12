# Two branches to bring together (12 Nov 2026)

`claude/project-thread-gg3ogp` has top bar A (81698a0; mockup in `design/cauldron-top-bar-a.html`), the motion clock for the drawer, panes, Send/Stop, transcript and sidebar indicator (`crates/chrome/src/motion.rs`, `crates/app/src/motion.rs`).

`claude/project-thread-sa3a1p` (Windows session) has step lines under replies, tooltip fade, the + menu pop with a sliding hover highlight, and Segoe UI Variable Text. It forked at 490c847 and does not have the top bar.

A dry-run merge of sa3a1p into gg3ogp conflicts only in `crates/app/src/shell.rs`, four hunks: the `use` lines, the `Cauldron` fields, `Cauldron::new()`, and the + menu.

Both branches animate the + menu. Keep the sa3a1p version (checked on a display) and drop `menu_since` and `menu_shown` and the `k` argument of `tab_menu_el`. Keep every other motion field from gg3ogp. Merge where there is a display to check the result.
