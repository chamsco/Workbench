# Design survey: affectUI, Tuff UI, Palermo (Profound)

Surveyed 2026-11-12 for Cauldron (Rust/GPUI desktop app for running coding agents).
Purpose: patterns, measurements and licences. Web/React code can't be used directly in GPUI. Port the ideas and numbers, not the code.

Method: public pages via WebFetch (direct curl was blocked by the egress proxy for all three hosts). Every number below comes from a public page, a public docs page, or a free/MIT registry file or repo file. I did not log in, open any Pro source, or download Pro assets. Tuff's `/r/*.json` registry files are `robots_disallowed`, so I didn't fetch them. I read Tuff's tokens from its public MIT GitHub repo instead. GitHub rate-limited some fetches, so the Tuff `clause-playbook.tsx` source was **not** read. Its anatomy below comes from the public docs page only.

---

## 0. Licence verdicts (read first)

| Library | Free part | Paid part | What we may do |
|---|---|---|---|
| **Tuff UI** (tuffui.com) | Everything. "MIT licensed, free forever." | None. README: "No Pro tier, no signup, nothing locked." | **Copy, adapt, port and redistribute, including commercially.** Keep the MIT notice. README: "MIT. Free for personal and commercial use; keep the license line at the top of each file." If we port a component's code or token file closely, put the MIT copyright line in the ported Rust file or in `THIRD_PARTY_NOTICES`. Ideas and measurements alone need no attribution, but crediting the source is good practice. |
| **affectUI** (affectui.com) | A subset is MIT. The home page says "46 items are free and open source (MIT)". The compare page says "40 items, MIT". The components page says 29 of 98 components are free. Free item pages say "Free and open source (MIT)" (e.g. Button, Tool Call Card). | **affectUI Pro**, a one-time purchase. Individual is $249 ("every Pro item for one person, lifetime updates, unlimited commercial projects"). Team is $699 ("the same for a team"). Both include "a commercial license for unlimited projects, for you and your clients." | **Free/MIT items:** same as Tuff (copy, adapt, port, keep the notice). **Pro items** (e.g. Agent Checkpoints, Cost Waterfall, Approval Card, Context Window, Mission Control): **study only from public descriptions** unless we buy. I found no separate licence or terms page, and the pricing page doesn't cover redistribution or seat counting. If we buy, ask the vendor about porting Pro code into a distributed desktop app before doing so. |
| **Palermo** (palermo.tryprofound.com) | None. "It lives inside our codebase and isn't published as a package." "The only way to install Palermo is to work at Profound." | n/a (internal and proprietary) | **Study only.** No licence is granted and no source is published. Token values shown publicly on the docs pages are useful *reference measurements*, but we should derive our own palette rather than reproduce Profound's token set wholesale. Don't copy their names, icons or brand. |

Source URLs for licence quotes:
- Tuff: https://tuffui.com/components/clause-playbook, https://tuffui.com/docs/installation ("MIT licensed, free forever."), https://github.com/yurigdesigner/tuffui (README and the `LICENSE` file, MIT). I couldn't read the copyright-holder line of LICENSE because of the rate limit. Fetch it before shipping any ported code.
- affectUI: https://www.affectui.com/ ("46 items are free and open source (MIT)." / "The others are part of affectUI Pro – a one-time purchase from $249."), https://www.affectui.com/pricing, https://www.affectui.com/compare ("A set of components, screens and blocks is free under MIT." / "Pro unlocks the rest with a one-time payment, including every item added later."), https://www.affectui.com/components/button ("Free and open source (MIT)."), Pro pages ("Part of affectUI Pro").
- Palermo: https://palermo.tryprofound.com/docs/get-started (quotes above). The site has no licence or copyright text.

---

## 1. Tuff UI

React 19 + Tailwind v4 + `motion`, distributed as a shadcn registry. There are about 82 components in these groups: Foundations (24), Screens (7), Conversation (13), Tools & Approvals (5), Interactive Answers (19), Planning & Execution (2), Observability (8), Settings & Usage (4). Index: https://tuffui.com/components. Icons are Phosphor (filled by default, bold for line glyphs).

### 1.1 Tokens (from `app/globals.css` in the public MIT repo, plus https://tuffui.com/docs/installation)

**Light (`:root`)**
| Token | Value |
|---|---|
| `--background` | `oklch(0.975 0.0015 286)` (about #F7F7F8) |
| `--foreground` | `oklch(0.2 0.004 286)` (about #1C1C1F) |
| `--surface` | `oklch(0.948 0.002 286)` (about #EEEEF0) |
| `--surface-inset` | `oklch(1 0 0)` (#FFFFFF) |
| `--surface-raised` | `oklch(0.912 0.003 286)` (about #E4E4E7) |
| `--border` | `oklch(0.2 0.004 286 / 0.07)` (foreground at 7%) |
| `--muted-foreground` | `oklch(0.5 0.008 286)` (about #6B6B73) |
| `--ring` | `color-mix(in oklab, var(--brand) 55%, transparent)` |
| `--brand` | `#0071e3` |
| `--positive` / `--negative` / `--warning` | `#248a3d` / `#d70015` / `#c93400` |
| `--button` | `linear-gradient(0deg, #fff 0%, #e6e6e6 100%)` (hover end `#eeeeee`) |
| `--button-foreground` | `#111111` |
| `--button-edge` | `rgba(0,0,0,0.1)` |
| `--button-shadow` | none (`0 0 #0000`) |
| `--button-secondary` | `linear-gradient(180deg, #e2e2e2 0%, #ececec 100%)` |

**Dark (`.dark`)**
| Token | Value |
|---|---|
| `--background` | `oklch(0.155 0 0)` (about #111111) |
| `--foreground` | `oklch(0.97 0 0)` (about #F5F5F5) |
| `--surface` | `oklch(0.212 0 0)` (about #1D1D1D) |
| `--surface-inset` | `oklch(0.172 0 0)` (about #151515) |
| `--surface-raised` | `oklch(0.265 0 0)` (about #282828) |
| `--border` | `oklch(1 0 0 / 0.06)` (white at 6%) |
| `--muted-foreground` | `oklch(0.68 0.004 286)` (about #9E9EA3) |
| `--ring` | brand at 60% |
| `--brand` | `#0a84ff` |
| `--positive` / `--negative` / `--warning` | `#30d158` / `#ff453a` / `#ff9f0a` (Apple system colours) |
| `--button` | `linear-gradient(0deg, #fff 0%, #ababab 100%)` (a silver primary even in dark mode) |
| `--button-edge` | `rgba(255,255,255,0.5)` |
| `--button-shadow` | `0 2px 2px 0 rgba(0,0,0,0.1)` |
| `--button-secondary` | `linear-gradient(180deg, #1d1d1d 0%, #262626 100%)` |

(Hex conversions of oklch values are approximate. I computed them, the source doesn't state them.)

**Radii:** `--radius: 0.75rem` (12px). sm = 8px, md = 10px, lg = 12px, xl = 16px. Surface layering uses Shell `rounded-[1.375rem]` (22px), Panel `rounded-2xl` (16px) and Inset `rounded-xl` (12px). Shell padding is `p-1.5` (6px), so the radii nest: 22 − 6 = 16.

**Type:** Geist Sans for UI, Geist Mono for data, Orbitron only for the logo. `font-feature-settings: "cv11", "ss01"` (alternate glyph forms such as single-storey a and open digits). Antialiased smoothing. Examples use `font-mono text-xs` for insets and `font-mono text-2xl` for big numbers.

**Focus:** `:focus-visible` is a 2px solid `--ring` outline with 2px offset. `::selection` is brand at 30%.

**Motion** (`registry/motion-tokens/motion-tokens.ts`)
- `EASE_OUT = cubic-bezier(0.23, 1, 0.32, 1)`: strong ease-out for things that enter or leave
- `EASE_IN_OUT = cubic-bezier(0.77, 0, 0.175, 1)`: for movement on screen
- `spring = { type: "spring", duration: 0.3, bounce: 0 }`: bounce-free, for layout and morphs
- `enter = 200ms EASE_OUT`, `exit = 140ms EASE_OUT` (exits faster than entrances)
- `instant = 0`: "for keyboard-initiated changes and reduced motion"
- Every component uses `MotionConfig reducedMotion="user"`. Under reduced motion, transforms and layout stop but **opacity still animates** "so state stays visible".
- Timings: tooltip first-hover delay 300ms. Toast 4000ms (errors 6000ms). Undo window 5000ms. Rolling-number stagger 24ms (right to left).

**Control sizes (density):** Action Button sm 32 / md 36. Segmented Control 32 / 36. Status Pill 20 / 24. Progress Bar 6 / 8 / 12. Agent Avatar 20 / 28 / 36 / 48. IconTile 36 square. Terminal max-height 280. Layouts use container queries, previewed at 1280, 768 and 390.

### 1.2 Surface foundation (https://tuffui.com/components/surface)
- Borderless layers: **Shell** (`bg-surface`, 22px radius, 6px padding) > **Panel** (`bg-surface-inset`, 16px, usually `p-4`) > **Inset** (`bg-foreground/[0.04]`, 12px, `px-3.5 py-3 font-mono text-xs`).
- Depth comes from tone steps, not outlines or shadows: "Main content, one step lighter or darker than the shell", "Inset — code, payloads and details, one level deeper".
- Headers and footers sit *in the shell's padding*, outside the panel. "Footer text lives in the shell too." ShellHeader has an optional IconTile 12px from the shell edge, a title (h3), a subtitle, and an aside on the right.
- In light mode the panel is *whiter* than the shell (white card on a grey tray). In dark mode the panel is *darker* than the shell (recessed well). This is worth copying as is.

### 1.3 Clause Playbook (https://tuffui.com/components/clause-playbook)
"An agent's negotiation playbook for one clause." The example is Liability cap § 9.2, Corvane MSA.

Anatomy, top to bottom:
1. **Header:** a section number "§ 9.2" set apart from the title "Liability cap". Subtitle "Negotiation playbook · Corvane MSA" uses a middot separator. A state badge on the right reads "No edits". The default icon is Phosphor *Scales*.
2. **Agent note:** one line of agent narration, "Comparing your playbook with 2,140 signed MSAs…". Changes are announced (aria-live) when the note changes.
3. **Market chart (dot histogram):** labelled "Market" with the scale key "1 dot ≈ 9 MSAs". Each value bucket is a stack of dots. Colour encodes the zone: dots **below the fallback handle are dimmed**, the **fallback zone is neutral**, and the **preferred zone is the accent colour**. A percentile readout shows "p45 of market" (`marketPercentile(distribution, value)`).
4. **Threshold track:** "Liability cap thresholds". A two-handle range on domain [0, 24] with ticks at 0, 6, 12, 18 and 24. The unit "mo" appears **only on the last tick**. Handles sit at 6 and 12, with a `minGap` between them.
5. **Zone legend:** Walk-away (<6) · Fallback (6–12) · Preferred (12+).
6. **Settings rows:** one per row, each a label plus a one-line description plus a control on the right:
   - Data-breach super-cap: stepper "2×" ("multiple of the general cap")
   - Escalate to: select "Finance" ("Who the agent pings before it concedes")
   - Auto-accept within fallback: toggle ("Agent signs off 6–12 mo on its own"). The description restates the live threshold values.
   - Suggest market wording: "From 2,140 signed MSAs"
   - Block below walk-away: "Needs General counsel to unblock"
7. **Footer:** a "Live in the playbook" status on the left, with **Discard** and **Apply** on the right.

States and behaviour: Apply stays disabled until something differs from the baseline. Apply then resets the baseline and Discard restores it. Handles move with arrows (one step), Shift+arrow or PageUp/PageDown (large step), and Home/End. Agent-proposed values **glide into place** instead of jumping.

Why it looks professional: one domain-specific visual (the dot market) ties three readouts together (handles, zone colours, percentile). The numbers are real and specific (2,140, p45, 2×). Units appear once. Every setting has a plain-language consequence line. There's an explicit dirty/clean model with Discard and Apply. Colour is used only for the one zone that matters.

### 1.4 Renewal Rings (https://tuffui.com/components/renewal-rings)
"Contract renewals on a 12-month clock: one ring per contract, a hatched notice window, the next deadline highlighted."

Anatomy:
1. **Header:** title "Renewals", subtitle "Next 12 months · from today", and a pill "5 in 90 d" (count plus a compact unit).
2. **Chart heading:** "Term left", with a "7 of 11 shown" note. It says honestly when data is truncated (`maxRings` defaults to 6 and keeps the soonest).
3. **Clock face:** 12 month initials (N D J F M A M J J A S O) around the edge. A **Today** marker sits at 12 o'clock. The radial scale runs 0–12 with an "in 90 days" reference sector.
4. **Rings:** concentric, one per contract, with the **soonest end on the outer ring**. Each ring has a *term arc* plus a **hatched notice window** (diagonal hatching distinguishes notice from term without a second hue).
5. **Legend:** Term · Notice · 90 days.
6. **Details panel:** below the chart on narrow cards and beside it from 32rem (container query). Shows end date, days left, ACV, auto-renew and notice date. A passed notice date is labelled "passed".
7. **Footer stats:** three mini-meters. "Notice due" on a 30-day scale, "Auto-renew" with count 5, and "Up for renewal" with ACV on a $1M scale labelled "MACV".
8. **Actions:** small buttons under the details ("Draft notice", "Remind me").

States: hover or focus previews a ring. Click selects it, and clicking again or Esc clears. Arrow keys, Home and End move between rings. On mount, the arcs **sweep in from Today one after another** (staggered). This is disabled by `intro=false` and under reduced motion. `noticeSoonDays` (default 30) decides what counts as "due".

Why it looks professional: a time metaphor the reader already knows (a clock). Texture (hatching) instead of extra colours. Numbers have compact units ("90 d", "$480K"). "N of M shown" honesty. Selection drives a details panel instead of tooltips.

### 1.5 Other Tuff pieces worth porting as patterns
- **Status Pill** (https://tuffui.com/components/status-pill): tones are neutral, brand, positive, negative and warning, each a tinted background plus tinted text. Guidance: "Always pair with text." Companion **Tag** is a *mono* label for models, branches and transports (`<Tag>opus-5.5</Tag>`). Directly useful for Cauldron model and branch chips.
- **Action Button** (https://tuffui.com/components/action-button): variants are primary (silver gradient, "use one per view"), secondary (default), quiet and danger. Heights are 32 (default) and 36. Loading swaps the icon for a spinner. Icon-only buttons need a label and tooltip.
- **Rolling Number** (https://tuffui.com/components/rolling-number): odometer digits. Only digits roll, and units and separators stay fixed. Right-to-left 24ms stagger. Tabular figures are always on.
- **Dot Histogram** (https://tuffui.com/components/dot-histogram): 48 bins by default. Footer shows count, median, p95 and flagged count with % plus a scale note "1 dot = 2 runs". Samples over the threshold are red. Tall columns re-bin to keep the height fixed.
- **Tick Gauge** (https://tuffui.com/components/tick-gauge): 44 ticks on an arc. Ticks between the value and the target turn red. "Ticks fill in order on mount; later values recolor them in place." `role="meter"`.
- Agent-relevant ones to review next: tool-call-block, file-changes, terminal-output, context-window-meter, run-trace, checkpoint-timeline, background-agents, tool-approval-queue, rate-limits.

---

## 2. affectUI

178 items (components, screens, blocks, illustrations, motion), each a **single self-contained `.tsx` with injected CSS and no Tailwind**. Dark mode turns on via an ancestor `.dark` or `[data-theme="dark"]`. Site theme colour is `#0c0c0d`. Index: https://www.affectui.com/components.

### 2.1 Tokens (from the free MIT registry files `https://www.affectui.com/r/button.json` and `/r/tool-call-card.json`)
Global hooks follow the pattern `--aui-*` with per-component fallbacks.

| Hook | Light | Dark |
|---|---|---|
| `--aui-ink` (text) | `#171717` | `#ededed` |
| `--aui-muted` | `#737373` (button), `#6b6b6b` / `#8a8a8a` (tool card text/muted) | `#8f8f8f`, `#a1a1a1` / `#7c7c7c` |
| `--aui-surface` | `#ffffff` | `#232326` (button), `#161618` (card) |
| `--aui-subtle` (hover) | `#fafafa` | `rgba(255,255,255,0.035)` |
| `--aui-border` | `#e5e5e5` / `#e7e7e7` | `rgba(255,255,255,0.1)` |
| tile / code bg | `#f5f5f5` / `#fafafa` | `#1d1d20` |
| `--aui-danger` | `#c4320a` (red base `#f04438`) | `#ff8a7a` (red base `#ff6b5f`) |
| danger bg / line | red at 6% / 16% | red at 8% / 20% |
| `--aui-warning` | `#b25e00` | `#f2a15c` |
| syntax | key `#171717`, string `#0b8576`, literal `#8a35d6` | key `#ededed`, string `#4fc3b4`, literal `#cfaef5` |
| `--aui-focus` | `rgba(0,96,226,0.7)` | `rgba(77,141,255,0.8)` |
| `--aui-ease` | `cubic-bezier(0.22, 1, 0.36, 1)` | same |
| `--aui-radius` | 12px (button), 10px (tool card) | |
| `--aui-radius-lg` / `-sm` | 16px / 7px | |
| `--aui-shadow-card` | `inset 0 0 0 1px var(--line), 0 1px 2px rgba(0,0,0,0.04)` | `inset 0 0 0 1px var(--line)` (no drop shadow in dark) |

**Type:** `"Inter", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif`. Mono is `ui-monospace, "SF Mono", Menlo, monospace`.
- Button: weight 500, letter-spacing −0.01em, 13 / 14 / 15px for sm / md / lg.
- Tool Call Card base: **13.5px / 1.4, letter-spacing −0.006em**. Tool name in mono 12.5px/500. Status 12.5px **tabular-nums**. Code 12px mono / 1.6. Section label 12px/500 muted. Error 13px / 1.45.

**Button sizes:** sm 32h / 12px pad-x / 6 gap / 16 icon / radius 10. md 40 / 16 / 8 / 18 / 12. lg 48 / 20 / 10 / 20 / 14. The radius steps with size (±2px).
- Primary is a dark vertical gradient `#323137 → #201e25` with a 3-stop edge (`#4b4951 / #31303a / #18171b`). Its shadow is `0 0 0 1px ring, 0 2px 4px rgba(0,0,0,.1)`, plus a "shine" highlight `rgba(255,255,255,.82)`. In dark mode the primary inverts to a light gradient `#f4f4f5 → #e4e4e7`.
- Secondary is a white surface with a 1px `--line` border and `0 1px 2px rgba(0,0,0,.06)`.
- Pressed is `scale(0.97)`. Focus is a `2px solid` focus colour with 2px offset.

**Motion:** colour and shadow transitions take 160ms `ease`. Transform takes 160ms on the ease curve. Label entrance 480ms and icon entrance 520ms `cubic-bezier(0.25,1,0.5,1)`. Spinner 800–900ms linear. Shimmer sweep 1.04–1.8s linear. Expand/collapse 380ms (grid rows 0fr→1fr) with opacity 240ms. Chevron 360ms. Check-draw 360ms after an 80ms delay. Under reduced motion all durations become 1ms and the shimmer is hidden.

### 2.2 Tool Call Card (free, MIT) (https://www.affectui.com/components/tool-call-card)
Highly relevant to Cauldron.
- **Collapsed row:** min-height 44px, padding `8 10 8 9`, gap 10. A **26×26 icon tile** (radius 7, `#f5f5f5`, 1px inset line, 14px glyph at stroke 1.4). Then the **tool name in mono**, a truncating summary in muted text, and status on the right that never wraps.
- **Status:** running is a spinner (track at 22% opacity, ink arc) plus a **live timer** (tabular). Success is a check (stroke 1.8, ink, *not green*) plus duration. Error is red "Failed" text plus an icon (never colour alone).
- **Expanded:** sections with a 1px top divider and 12px padding and gap. Arguments and result are pretty-printed and highlighted JSON. A code block has padding `10 12`, is capped at 220px with a fade and "Show all", and has Copy buttons that change to "Copied". A Retry button sits next to the error. A running call with no result shows "Waiting for result…".
- **Group header:** min-height 36. "Used N tools" when idle, a shimmer on "Using tools…" while running, and the total time. Stacked 24px icon tiles overlap by 5px with a 2px ring in the surface colour.
- The card radius is 12px. Its border is an **inset box-shadow hairline**, not a CSS border.

### 2.3 Button (free, MIT) (https://www.affectui.com/components/button)
Variants are primary (dark fill), secondary (white plus outline), ghost and destructive. Loading turns the left icon into a spinner, shimmers the label, sets `aria-busy`, and keeps the button focusable (not disabled). It becomes square when it has no children.

### 2.4 Pro showcase pieces (public descriptions only, not studied as code)
- **Agent Checkpoints** (Pro): top bar with the total cost ("$0.42") and the heading "Checkpoints". Fixed-height rows show a title, optional icon, a time like "0:42 / 1:05" and a file count. On hover or focus, a **Rewind** button replaces the file count and time. Status: running shimmers, failed shows a badge, stopped is labelled. Branches sit on their own lanes. The **path to HEAD is blue and everything else fades back**. Over 14 rows it scrolls internally and keeps the current row in view. At narrow widths the task text and counts hide first.
- **Cost Waterfall** (Pro): "Run cost" heading with the task muted beside it. A cost/tokens toggle. One row per LLM, tool or sub-agent call in run order, with sub-agents folding their children. Bar length is the share of the total, and the bar is tinted by kind (agent, LLM, tool). Running bars shimmer and failed bars are red. Wide layout puts steps left, bars middle and numbers right, with In/Out token columns. Costs look like "$0.042" and durations are in ms. Breakpoints: below 720px the In/Out columns hide, and below 520px rows stack.
- **Approval Card** (Pro): a grey "tray" label above a white card. A 32px icon tile (shield). Risk badge is low (grey), medium (amber) or high (red). Preview plus a key-value details panel. Approve (primary) and Deny (quiet) carry **shortcut hints ⌘↵ / Esc**. "Always allow this action" checkbox. A quiet "Expires in 0:42" countdown turns amber in the last 10s and pauses on hover. After a decision the card morphs into a one-line resolved row with a time.
- **Context Window** (Pro): a 216-cell grid (36×6). Parts fill in order, with at most about 6 colours. The % turns amber at 0.8 and counts as full at 0.95. Hovering a part lights its cells. Compaction shimmers, then clears.
- **Mission Control** (Pro, block): a summary curve "since you last looked". A radio filter (all / running / waiting / failed, each with a count, plus "Needs you" = waiting + failed). A fleet of agent cards with nested sub-agents. A side inbox of waiting and failed agents with Approve / Reject / Retry. The inbox moves under the fleet below 860px.
- Other names (Pro or unknown): Agent Graph, Agent Lanes, Agent Tree, Trace Waterfall, Diff Timeline, Conflict Radar, Review Center (single-key approve, edit, reject, skip), Running Now, Usage Meter, Model Picker, Decision Card.

---

## 3. Palermo (Profound's design system)

React + TypeScript, internal to Profound. Docs pages are at `/docs/<slug>` (e.g. `/docs/button`, `/docs/colors`). Components: Active Bar, Alert, Audio Player, Avatar, Badge, Brand icons, Button, Card, Checkbox, Collapsible, Commandbar, Composer Question Tool, Dialog, Drawer, Dropdown Menu, Empty & Error States, Highlights, Filter Builder, Input, Multi-step Form, Pagination, Pill / Tag, Popover, Progress Bar, Radio Group, Scroll Area, Section Header, Segmented Tabs, Separator, Skeleton, Spinner, Statistic, Switch, Table, Tabs, Textarea, Toast, Toggle Group, Tooltip, Video Player.

### 3.1 Colour tokens (https://palermo.tryprofound.com/docs/colors). Reference only.
**Backgrounds (light → dark):** base `#ffffff → #1e1e1e`, primary `#ffffff → #252525`, secondary `#fafafa → #272727`, tertiary `#f5f5f5 → #343434`, quaternary `#e8e8e8 → #414141`. Semantic tints sit at about **10% alpha** (`#376cff1a` blue, `#ff5f571a` red, `#15b4621a` green, `#f4a4621a` orange), with hover at about 16% (`…2a`). Header is `rgba(255,255,255,.7)` / `rgba(30,30,30,.7)` (translucent). Tooltip is `rgba(24,24,26,.9)` in both themes.

**Fills:** primary `#18181a → #ffffff` (hover `#2c2c2e`), secondary `#969696 → #5a5a5a`, tertiary `#d4d4d8 → #464646`, quaternary `#e8e8e8 → #323232`, quinary `#f0f0f0 → #2a2a2a`. Accents are green `#15b462`, red `#ff5f57`, blue `#376cff` and orange `#f4a462`.

**Controls:** bg `#ffffff → #292929`, hover `#fafafa → #333333`, selected `#f1f1f1 → #393939`, primary `#18181a → #ffffff`.

**Text:** primary `#18181a → #ffffff`, secondary `#787878`, tertiary `#969696`, quaternary `#b4b4b4 → #5a5a5a`. The 4-step text ramp is identical across themes for secondary and tertiary.

**Shadows:** ring-1 `rgba(0,0,0,.02)`, ring-2 `rgba(0,0,0,.05)` → dark `rgba(255,255,255,.1)`. shadow-1/2 `.02/.05` → dark `0/.1`. `shadow-stroke` `#ebebeb → #292929`. Destructive shadows are red at 8%, 12% and 20%.
Shadow scale: `shadow-low | medium | high`, each with `-hover`, `-focus` and `-borderless`, plus `shadow-focus`. Guidance (verbatim):
- "Each regular shadow includes its border. Do not add a separate neutral border or ring."
- "Low and medium support hover and focus: hover adds a solid edge, while focus retains the depth and adds a 3px halo."
- "High has no interaction states."
- "Borderless keeps only the depth."

### 3.2 Typography (https://palermo.tryprofound.com/docs/typography)
Named roles only (values aren't shown publicly): `title-regular`, `title-small-regular/semibold`, `title-mini-regular/semibold`, `body-regular(-medium)`, `body-small(-medium)`, `body-mini(-medium)`, `body-micro-medium` ("labels and metadata"). Lesson: there are **few sizes, each with exactly a regular and a medium/semibold variant**, and the micro size exists *only* in medium.

### 3.3 Components (public docs)
- **Button** (`/docs/button`): Primary, Secondary (default), Tertiary, Ghost, Subtle, Destructive and Dashed. Sizes are micro, small, regular and large. Subtle and tertiary pair as a **view switcher** (off and on) with `aria-pressed`. Loading on a text-only button overlays a centred spinner. On an icon button it swaps the icon and keeps the label. Recommends a fixed width to avoid reflow.
- **Table** (`/docs/table`): `density` default or comfortable. Comfortable means a **40px header and 56px body rows**. `headerWeight` medium, regular or mini. `fixedHeader`. Expandable rows. Per-column `align`. `sortDirection` sets `aria-sort`, and the consumer owns sorting. Loading shows skeleton cells. `disableCardStyles` drops the card frame.
- **Statistic** (`/docs/statistic`): label, value, description and trend (up green with an arrow, down red, neutral "–"). `decimals` and `fixedDecimals`. Compact formatting ("+1.2k", "+12.3%"). `emptyText` for null. `changeMode` value or rank. `animated`.
- **Badge** (`/docs/badge`): 10 hues (default, red, blue, green, amber, yellow, purple, orange, gray, cyan). Sizes micro, small, default and large. Medium or regular weight. Default style is rounded and bordered. The **agent-builder** variant is "Borderless squared".
- **Card**: background primary, secondary, tertiary or quaternary. Padding none, sm or md.
- **Active Bar**: a sliding highlight behind the active vertical-nav item, positioned by `top`, with `--item-height` on the list. Animates between items.
- **Segmented Tabs**: pill tabs where "the active trigger fills with a control surface instead of showing an underline". Use `SegmentedToggleGroup` when there are no panels (a11y note).
- **Section Header**: title, subtitle, badge and right-aligned actions, with an info-tooltip or dotted-underline title tooltip. Title and subtitle have skeleton loading states.
- **Progress**: linear sm, md and lg, plus circular with a named or pixel size and `strokeWidth`. Indeterminate linear.
- **Icons**: in-house SVG set in Outline and Filled, with kebab-case names.

---

## 4. Cross-cutting patterns: "professional, not AI-generated"

1. **Tabular numerals on anything that changes.** Status timers and durations use `font-variant-numeric: tabular-nums` (affectUI Tool Call Card). Rolling Number always applies tabular figures and rolls only digits, leaving units still (Tuff). *GPUI: enable `tnum` on the numeric font, or use the mono face for live counters.*
2. **Depth from tone steps, not borders or shadows.** Tuff Surface is "borderless", with shell, panel and inset one tone step apart. Palermo has a 5-step background ramp (base, primary, secondary, tertiary, quaternary).
3. **Hairlines at very low alpha.** Tuff border is foreground at 7% (light) and white at 6% (dark). affectUI is `rgba(255,255,255,.1)` dark and `#e5e5e5` light, drawn as an *inset 1px shadow* (no layout shift). Palermo ring-2 is 5%.
4. **Shadow includes its border; dark mode drops drop-shadows.** Palermo says "Each regular shadow includes its border. Do not add a separate neutral border". affectUI's dark card shadow is only the 1px inset line. Shadows stay tiny (`0 1px 2px rgba(0,0,0,.04)`).
5. **Restrained colour: neutrals plus one accent plus semantic tints at about 10%.** Palermo tints are `…1a` (10%) with hover `…2a`. affectUI danger bg is 6–8%. In Tuff's Clause Playbook only the preferred zone gets the accent. Success checks are drawn in **ink, not green** (affectUI).
6. **Mono for identifiers, sans for prose.** Tool names (affectUI), model, branch and transport Tags (Tuff), and insets and payloads (Tuff `font-mono text-xs`) are mono. Palermo's agent-builder badges are squared and borderless to look "system".
7. **Small, deliberate type scale with slight negative tracking.** affectUI UI text is 13.5px / 1.4 at −0.006em, buttons 13–15px at 500 and −0.01em, labels 12px/500, code 12px / 1.6. Palermo has few sizes, each with regular and medium only, and micro only in medium for labels and metadata.
8. **Nested radii that add up.** Tuff shell 22 = panel 16 + 6 padding. affectUI button radius steps ±2px per size (10/12/14) and its card is `radius-lg − 4`.
9. **Status is never colour alone, and it's always paired with text.** Tuff Status Pill: "Always pair with text." affectUI: error shows icon plus "Failed" text. Badge risk levels name the level. Statistic neutral shows "–".
10. **Specific numbers, compact units, honest truncation.** "5 in 90 d", "7 of 11 shown", "1 dot ≈ 9 MSAs", "p45 of market" (Tuff). "$0.042", "0:42 / 1:05", "Used 4 tools" (affectUI). Units appear once (Tuff "mo" only on the last tick).
11. **Explicit dirty/clean and resolved states.** Clause Playbook keeps Apply disabled until edits exist and Discard restores the baseline. affectUI Approval Card morphs into a one-line resolved row. Answered decisions collapse.
12. **Keyboard parity with visible hints.** ⌘↵ / Esc hints on Approve and Deny (affectUI, hidden on touch). Arrows, Shift+arrow, PageUp/Down and Home/End on sliders and rings (Tuff). Single-key review (affectUI Review Center).
13. **Fast, damped motion; exits faster than entrances; nothing bounces.** Tuff enter 200ms and exit 140ms with `cubic-bezier(.23,1,.32,1)` and a spring with bounce 0. affectUI colour 160ms, expand 380ms, ease `cubic-bezier(.22,1,.36,1)`, press scale 0.97. Reduced motion keeps opacity changes (Tuff) or snaps to 1ms (affectUI).
14. **Shimmer and live timers mean "running"; fade means "not on the current path".** Running rows shimmer and failed ones turn red (affectUI Cost Waterfall and Checkpoints). The HEAD path is blue and other branches fade back (Agent Checkpoints). Rings sweep in with a stagger (Tuff).
15. **Hover reveals secondary actions in place of metadata.** affectUI Checkpoints: Rewind replaces the file count and time on hover or focus. Tuff Renewal Rings: hover previews, click selects into a details panel, and there are no floating tooltips for core data.
16. **Responsive by container, and less important columns drop first.** affectUI Cost Waterfall hides In/Out below 720 and stacks below 520. Checkpoints hide task text first. Tuff uses container queries (details panel beside the chart from 32rem).

---

## 5. Suggested starting values for Cauldron (derived, not copied)
- Neutrals: a 5-step surface ramp. Dark: about #111 background, #151515 inset, #1D1D1D surface, #282828 raised. Light: #F7F7F8, #FFF, #EEEEF0, #E4E4E7. Text ramp primary, secondary, tertiary, quaternary.
- Hairline: foreground at 6–8%, drawn inside the bounds. No drop shadows in dark mode except on overlays.
- One accent (blue in the #0A84FF / #376CFF family). Semantic positive, negative and warning used mainly as 8–12% tints, with full strength reserved for small glyphs and text.
- Type: Inter or Geist at 13–13.5px UI / 1.4, 12px labels at 500, 12px mono / 1.6 for code and IDs, tabular numerals globally on numeric runs. Tracking −0.006 to −0.01em.
- Radii: 6 / 8 / 10 / 12 / 16, nesting outer = inner + padding.
- Control heights: 24 (pill), 28, 32 (default button and segmented), 36, 40. Rows 36 (group), 44 (tool row), 40/56 (table header and comfortable row).
- Motion: enter 200ms, exit 140ms, hover and colour 160ms, expand 360–380ms. Curve about `cubic-bezier(0.23,1,0.32,1)`. No bounce. Respect reduced motion by keeping opacity changes.
- Focus: 2px ring at 2px offset, accent at 55–70% alpha.

## 6. Gaps / not verified
- Tuff `clause-playbook.tsx` and `renewal-rings.tsx` source (exact dot sizes, stroke widths, hatch spacing): GitHub rate-limited my fetches, and the `/r/*.json` registry is robots-disallowed. Both components are MIT, so the source can be read later from the repo.
- Tuff LICENSE copyright-holder line: not captured. Read it before porting code.
- Palermo pixel sizes, type sizes and shadow composites: not exposed in public text. Values are probably in CSS, but the rendered docs didn't show them.
- affectUI's full licence or terms document: none found. Pro redistribution terms are unknown.
- affectUI free-item count is inconsistent across pages (46 / 40 / 29-of-98 components).
