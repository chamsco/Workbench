# gpui-app

I opened every cited line in `crates/app/src/{main,canvas,ui,arrange,pal}.rs` and read the surrounding 15 lines, then re-checked the structural claims with greps (all `-----` banners in the slice, every `fn`/`impl` boundary in `main.rs` and `canvas.rs`, and every `GLASS` use). Every line number and quote matched the file, so nothing was rejected and no count was dropped; the only corrections were to two rationales (the `bg(p.glass)` on `main.rs:994` sits a few lines *below* the comment, not above it, and the twelve `main.rs` dividers are not byte-identical — they use two dash lengths), neither of which changes a verdict. The three keeps are platform (`ui.rs:63`, macOS blur vs the desk fallback, with `GLASS` feeding `WindowBackgroundAppearance::Blurred` at 2910 and the fallback at 1029) and upstream semantics (`ui.rs:530`, Unicode lowercasing changes byte lengths while `lower`'s indices slice `body`); `arrange.rs:26` survives because the hit-test tree cannot be the geometry the same drag is reflowing. `canvas.rs:720` is the one genuine refactor: the comment admits arithmetic fused into an element style for a CSS box-model replica.

## Delete (18)

| file:line | kind | the comment | why it goes |
| --- | --- | --- | --- |
| crates/app/src/canvas.rs:384 | section-banner | `// -------------------------------------------------------------- browser` | Browser code starts at 1246; this divider sits between two blank lines and the diagram banner, labelling nothing. |
| crates/app/src/canvas.rs:386 | section-banner | `// -------------------------------------------------------------- diagram` | Directly above `fn diagram_pane`, whose name already says it; two stacked dividers with no code between them. |
| crates/app/src/canvas.rs:1244 | section-banner | `// -------------------------------------------------------------- browser` | Second "browser" divider 860 lines after the first; still redundant with `fn browser_pane` at 1246. |
| crates/app/src/canvas.rs:844 | section-banner | `// -------------------------------------------------------------- docs + new` | Divides `fn docs_pane`; the function name is precise, while "new" names a section that lives in the chooser region instead. |
| crates/app/src/canvas.rs:1102 | section-banner | `// -------------------------------------------------------------- picker` | Above `fn chooser` at 1104; the banner's word disagrees with the function name it labels. |
| crates/app/src/main.rs:48 | section-banner | `// ------------------------------------------------------------------ model` | Followed by self-naming enums and structs; one of twelve dividers of the same shape in the file (612, 651, 870, 915, 1150, 1605, 1843, 2084 and the rest). |
| crates/app/src/main.rs:405 | section-banner | `// -------------------------------------------------------------- data` | Inside `impl Workbench` above `backend`; the methods below (backend, machine, refresh, tree, fetch) describe themselves. |
| crates/app/src/main.rs:948 | section-banner | `// ------------------------------------------------------------------ render` | Immediately above `impl Render for Workbench`, which announces the same thing more precisely. |
| crates/app/src/main.rs:2497 | section-banner | `// -------------------------------------------------------------- settings` | Above `fn settings_page`, the only item in that region; the heading partitions nothing. |
| crates/app/src/arrange.rs:508 | section-banner | `// -------------------------------------------------------------- tabs` | Above press_tab/move_tab/drop_tab/tab_strip, all self-naming; same divider pattern as main.rs. |
| crates/app/src/ui.rs:77 | section-banner | `// ------------------------------------------------------------------ primitives` | The module doc at line 1 already lists fonts, hairlines, icons, dots, buttons and the rest that follow. |
| crates/app/src/main.rs:286 | restates-code | `// Spinner and pulsing dots, only while something is working.` | "only while something is working" is the `busy` match at 293-298; the `SPIN` indexing at 300 shows it drives the spinner. |
| crates/app/src/main.rs:422 | restates-code | `// A new review is queued up front, the way a notification would be.` | Narrates the `pending.last()` assignment taken when the count grew; "the way a notification would be" is mood, not a rule. |
| crates/app/src/main.rs:674 | restates-code | `/// Per-slot state follows the tab: reset it and load browser addresses.` | Both halves are the body verbatim: the reset loop at 677-682 and the url/fetch loop at 683-688. |
| crates/app/src/main.rs:994 | obvious | `// Sidebar and main column share one sheet of glass: no divider.` | `bg(p.glass)` is set on the shared container at 999; a missing divider is visible as a missing element. |
| crates/app/src/ui.rs:163 | restates-code | `/// Text with per-span colours (StyledText highlights).` | Signature `parts: &[(&str, Option<Hsla>)] -> StyledText` plus the `HighlightStyle` body already say it. |
| crates/app/src/pal.rs:49 | restates-code | ``/// `rgba(r, g, b, a)` from the stylesheet.`` | Signature is `fn ca(r: u8, g: u8, b: u8, a: f32)`; the module doc at line 1 already claims stylesheet parity. |
| crates/app/src/main.rs:832 | restates-code | `/// Show something in this tab: focus a canvas already showing it, else` | Summarises the match at 840-849 and omits its only subtlety: `same` compares agent ids for "agent"/"files" only. |

## Refactor first (1)

| file:line | the comment | what the code should become |
| --- | --- | --- |
| crates/app/src/canvas.rs:720 | `// CSS width is content-box: 360 + 2x14 padding + hairlines.` | Extract the 360px content width and the padding/hairline sum into one named card-width constant (with the box-model note as its doc), so the replica arithmetic at 721 lives in one place instead of inline `360. + 28. + 2. * f32::from(w)`. |

## Keep (3)

| file:line | the comment | what cannot change |
| --- | --- | --- |
| crates/app/src/ui.rs:63 | ``/// macOS blurs what is behind a translucent window; elsewhere the replica's`` … | Platform capability: `GLASS = cfg!(target_os = "macos")` drives `WindowBackgroundAppearance::Blurred` (main.rs:2910) and the desk fallback (main.rs:1029); deleting it invites removing the fallback. |
| crates/app/src/ui.rs:530 | ``// Lowercasing ASCII keeps byte offsets aligned with `body`.`` | Upstream Unicode semantics: `to_lowercase()` can change byte length while `lower`'s indices slice `body` at 537 and 543. |
| crates/app/src/arrange.rs:26 | `/// The arrangement when the drag began. Hit-testing uses it, so the` … | The hit-test tree cannot be the geometry the same drag is reflowing; `base` must stay the pre-drag snapshot or the canvas under the pointer moves away. |
