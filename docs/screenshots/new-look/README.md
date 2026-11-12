# New look

The app moves to the design-system tokens (design/cauldron-design-system/tokens.json at 45d2432 on
`claude/project-thread-92mzld`) and the v2 board's layout (design/cauldron-new-look.html at 2096c8f on
`claude/project-thread-gg3ogp`), gate by gate. Shots are the real window at 1200x720 with the
sample data folder.

## NL1: tokens

`nl1-01-new-thread.png`, `nl1-02-thread.png` (dark), `nl1-03-thread-light.png` (light; the theme follows the OS, so that shot came from a build forced to light, reverted and not committed): the current layout on the new tokens. The layout changes from NL2 on;
`board-frame1.png` is the board's frame 1 for comparison.

- Palette: Tuff surfaces (bg #111111, card #1d1d1d, panel #151515, raised #282828 in dark), purple accent
  #a08cff dark / #6236d6 light. `inset`, `hover`, `selected`, `line` and `line-strong` are overlays (`Rgba`)
  that GPUI composites over whatever is beneath. `run`, `info`, `design` and `mem` are gone (the Code and project chips and the
  code glyphs are fg-2; your message bubble is `raised`).
- Radii: SHELL 22, PANEL 16, INSET 12, CONTROL 10, INNER 8, TAG 6, replacing 6/4/3. For now the old uses map
  R to PANEL, R1 to CONTROL and R2 to INNER; each screen takes its own radii as it is ported.
- Type: Geist and Geist Mono are bundled and registered on every OS. The shell sets ss01 and tnum (tabular figures) on its root. The bundled TTFs have no cv11 (GSUB checked with fontTools), so that set is not on.
- Found by the tests: light fg-3 on `selected` over a card is 4.37:1, so selected fills go on bg or a panel, never on a card. Nothing uses `selected` yet; selected rows still use `raised` until NL2 moves the sidebar onto bg.
- Not changed: motion stays Cauldron's (200ms cap); the design system's 240 and 300ms durations break that rule. The
  shadows keep their dark values in both themes, and the 1px rings from `shadow-menu`/`shadow-tip` are not added yet.
- Runs: `docs/cauldron/NL1-tokens-red.txt` (the new tests against the old tokens: no `Rgba`, no new radii), `NL1-tokens-green.txt`.
