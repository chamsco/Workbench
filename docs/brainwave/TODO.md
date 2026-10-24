# To do

This is the working list for Brainwave. Each section says what it is for, what
is done, and what is left. The order is the order we work in. Tick a box in the
same PR that does the work. The why is in [PLAN.md](PLAN.md) and the how in
[ARCHITECTURE.md](ARCHITECTURE.md).

Status on 2026-10-24.

## 1. The frame (PR #29)

The window after the owner's target design: sidebar, app tiles, project tree,
the page card, and Home with its chat column.

- [x] Sidebar row, app tiles, tree, machine switcher
- [x] Page card 8px in with rounded corners; full width except on Home
- [x] Window material: Frosted (the default), See-through, Solid
- [x] Room for the window buttons on Windows, Linux and macOS
- [x] Header separator; Code with a darker page and no chat column
- [ ] Merge order: #22 → #24 → #29, and close or rebase #28 (it overlaps).
      **Owner's call is still open.**
- [ ] See it on a real Mac, a real Windows box and a real Linux box: glass,
      see-through, the window buttons
- [ ] Get the first macOS CI build green

## 2. Agents (PR #29)

Make an agent something you can see, talk to and tune in one place.

- [x] The brain mascot from the owner's drawing: looks at the cursor, blinks on
      its own, default pink
- [x] Agent panel: name, brain, then Settings / Context / Traces / Routines
- [x] Settings: change the model, instructions, personality, memory, rules
- [x] Context map: vertical treemap with tiles by kind, Free, and a hover card
- [x] Traces: the last 20 replies, each opening its waterfall
- [x] Routines: the agent's routines, with run, edit and add
- [x] Voice mode from a mic next to Send; equalizer eyes and a swaying head
- [x] Organigram of agents, projects and groups, drawn by Pele
- [ ] Count tool definitions and tool results in the context map
- [ ] Use the trace's real input tokens as the total when one exists
- [ ] Voice on real hardware: macOS (WebKit) and Windows (WebView2)
- [ ] Voice on Linux: core-side speech-to-text (Moonshine or whisper.cpp)
- [ ] Routines run when the app is closed (a background runner)

## 3. Home and memory

One memory, shown on Home, written by every app.

- [x] Memory folders on Home (Everywhere, per project, per agent) and recent
      activity
- [x] The Memory tile removed; folders open as Home › Folder
- [x] Memory on by default; new notes go to the folder you are in
- [ ] Every app proposes through `memory::propose` with its own source tag
      (`code`, `design`, `clip`)
- [ ] Pending notes: inferred notes wait on Home for one tap
- [ ] Rank by relevance: local embeddings through the model gateway
- [ ] A folder card per app source, if the owner wants "by app" as well as
      "by project"

## 4. Design system

One look everywhere. See [DESIGN.md](DESIGN.md).

- [ ] `ui/tokens.css`: one token set imported by the frame and the React face
- [ ] Replace inline hex values (context map, folder cards, toggles) with tokens
- [ ] Six type steps; map the old sizes
- [ ] Four radii
- [ ] One Toggle, Pill, Tabs, Button, Input and Card in
      `web/src/components/ui/`, used by both stacks

## 5. Architecture debts

The ones that bite first. See [ARCHITECTURE.md](ARCHITECTURE.md#debts).

- [ ] Secure the phone link: short-lived pairing, bind only while pairing, TLS
      or Tailscale
- [ ] Code workers on `backspace-runner` (traces and Carry on)
- [ ] Split `chat.rs`; move logic out of `main.rs` into core
- [ ] Freeze the GPUI shell
- [ ] Decide: React for every new surface

## 6. Design app, v0

- [ ] Brief → two to six live HTML directions through `show_options`
- [ ] Pick, mix, "more like this"
- [ ] Picks and brand tokens to memory (`design`)
- [ ] Export as HTML and PNG

## 7. Clip Studio, v0

- [ ] The crew: Director, Voice, Visuals, Scripts as four agents in a group, with
      the cloud mascots as their faces
- [ ] Script → voice-over (system voice, then local TTS) → stills → MP4 with
      ffmpeg in core
- [ ] The timeline as the thread's artifact
- [ ] Choices to memory (`clip`)

## 8. Docs

- [x] PLAN.md, DESIGN.md, RESEARCH.md, ARCHITECTURE.md, TODO.md
- [ ] Get the owner to sign off on the *Decisions to agree on* in
      ARCHITECTURE.md
- [ ] Rewrite the README around agents, memory and apps once milestone 2 ships
