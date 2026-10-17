# tauri-ui-css

I opened all four files at the 22 cited sites and read 15 lines around each, then chased the producer of every class the comments name (`index.html`, `app.js`, `shell.js`, `home.js`, `chat.js`, `shell.css`'s own hide rule) because most of these comments are claims about markup and JS rather than about CSS; rejected is 0 — every line and quote exists, and I only replaced the audit's line-joined renderings of multi-line blocks with verbatim fragments of the same block. Four reasons were repaired against the file, three of them making the delete stronger: `shell.css:210`'s advertised `.hc-panel-in-.hc` selector does not exist (the verdict holds because `home.js:59` nests `.hc-panel` in the `.hc` card), `chat.css:18` is not merely a restatement but stale — `shell.js:216–218` renders Chat/Agents and ADE/IDE, never "Chat | Coding" — `chat.css:386` describes a `.side-acts` block that has no markup anywhere in `crates/` and is force-hidden by `shell.css:133`, so the code under it is dead too, and `style.css:438` duplicates the module header at `app.js:10–17`, which is what a redesign log belongs in. The 18 deletes lose nothing a reader cannot rebuild from the selectors, names and JS; the three refactors keep a real why and only shed the "used to" sentence; the one keep survives because no rule of this repo can make an unregistered custom property interpolable.

## Delete (18)
| file:line | kind | the comment | why it goes |
| --- | --- | --- | --- |
| crates/tauri-app/ui/shell.css:324 | tombstone | `/* The chatbox carried a 1px gradient "silver" frame for a round.` | Nothing below draws a frame or a bevel, so the post-mortem and the handoff note it cites constrain no live rule. |
| crates/tauri-app/ui/chat.css:400 | history-narration | `/* The drawer now slides out from the sidebar. */` | "now" reports a migration; `left:6px`/`right:auto` and `slidein-l` already place the drawer and set its direction. |
| crates/tauri-app/ui/style.css:438 | history-narration | `Tabs on the left hold up to four canvases each; the sidebar and main column…` | Redesign changelog duplicating app.js:10–17; the split/stack structure and the shared glass are the selectors below. |
| crates/tauri-app/ui/shell.css:404 | self-praise | `/* The status line that says which mode the CLI is in, and how many agents are…` | Describes copy app.js:1105 writes (permission mode, agent count) and praises it; the rule below only dims `.dim`. |
| crates/tauri-app/ui/shell.css:314 | self-praise | `A 32px circle wearing the tools' translucent well when off, filling with ink…` | Restates `width`/`height`, `border-radius:50%`, `var(--chip)` and the `aria-pressed` ink fill; the last sentence is applause. |
| crates/tauri-app/ui/chat.css:18 | restates-code | `/* The Chat \| Coding switch: a pill with the active half filled dark. */` | Stale label (shell.js:216–218 renders Chat/Agents and ADE/IDE); pill radius and dark selected half are the rules below. |
| crates/tauri-app/ui/style.css:308 | obvious | `/* What the canvas decided to do with a public address, in the blank state. */` | Names an element whose copy app.js:1568 writes; the rule is only colour, size, line-height and max-width. |
| crates/tauri-app/ui/shell.css:210 | obvious | `/* The picker unfolds inside the composer, part of the same card. */` | home.js:59 already nests `.hc-panel` in the `.hc` card and `unfold` is the next rule; the cited `.hc-panel-in-.hc` selector does not exist. |
| crates/tauri-app/ui/chat.css:386 | section-banner | `/* Review + updates sit in the sidebar, between the spend card and the machines. */` | Narrates the placement of a block with no markup anywhere; shell.css:133 force-hides `.side-acts`, so nothing renders here. |
| crates/tauri-app/ui/style.css:546 | section-banner | `/* settings */` | Bare divider; the `.settings` selector on the next line repeats the word. |
| crates/tauri-app/ui/chat.css:367 | section-banner | `/* toast */` | Bare divider; `.toast` sits directly under it. |
| crates/tauri-app/ui/chat.css:203 | section-banner | `/* composer */` | Bare divider; `.composer` sits directly under it. |
| crates/tauri-app/ui/chat.css:315 | section-banner | `/* plans */` | Bare divider; `.plans` sits directly under it. |
| crates/tauri-app/ui/chat.css:371 | section-banner | `/* setup: what will you use Backspace for (tiles like a view picker) */` | Heading for `.uses`; the onboarding question lives in onboard.js and the parenthetical is visual narration. |
| crates/tauri-app/ui/chat.css:1 | section-banner | `/* Chat, setup, providers, plans and the empty states. Shares tokens with…` | File TOC for sections that each carry a heading; the `:root` block below shows the shared tokens. |
| crates/tauri-app/ui/home.css:1 | section-banner | `/* The model menu, the effort slider, the trace canvas and the…` | TOC plus a pointer to shell.css; the classes below say what this file owns and one grep finds the JS. |
| crates/tauri-app/ui/style.css:793 | section-banner | `/* ---------- status bar (after an agent control plane's footer) */` | Divider plus a credit; it constrains no rule in `.status` and names no external format to match. |
| crates/tauri-app/ui/style.css:13 | section-banner | `/* Layout: a macOS window floating on a desktop. Frosted sidebar…` | Layout essay listing environments and sub-features; the grid and tab rules show the structure, app.js/index.html the features. |

## Refactor first (3)
| file:line | the comment | what the code should become |
| --- | --- | --- |
| crates/tauri-app/ui/style.css:640 | `/* ---------- the chat surfaces sit on the painting, like Home ---------------…` | Trim to the scoping reason — "Still scoped by the window's mode class, so canvases, Apps and Settings keep the glass" — and drop the "used to paint one opaque sheet" sentence. |
| crates/tauri-app/ui/shell.css:330 | `What shows in the gap between starting and finding: the CLI scan is running,…` | Trim to the first sentence (the scan leaves the route list empty for a few seconds); drop "It used to show the connect panel in that gap". |
| crates/tauri-app/ui/shell.css:1 | `One window: the painting (art.js) runs behind the title bar, the sidebar…` | Keep only the cascade fact "Loaded last."; drop "as few lines as possible" and "chrome floats on the painting in soft chips and glass cards". |

## Keep (1)
| file:line | the comment | what cannot change |
| --- | --- | --- |
| crates/tauri-app/ui/style.css:180 | `Animated, not snapped. The width is set here rather than left to --side-w:…` | A plain, unregistered custom property is not interpolable — a CSS platform rule, and the app registers no `@property` anywhere in its own CSS. Only a class-level `width` can animate the rail; shell.css:408–412 restates the same limit. |
