# The notch, kept for later

Top bar A (81698a0, merged here on 12 Nov 2026) replaced the notch with one bar across the window.
Charles asked to keep the notch design, since it may be used somewhere else.

- The design: `design/cauldron-master-layout.html` still draws the notch (the sidebar card's cut
  corner holding the traffic lights and the sidebar button). The merge kept that file.
- The code: `notch-top-bar.patch` here is commit 81698a0 as a diff. Its `-` lines are the notch as
  it ran in the app: `metric::NOTCH_W` 126, `NOTCH_W_NO_LIGHTS` 62, `NOTCH_H` 52, `Shell.notch`
  and its pane, the solver's notch rect, the notch drawing and drag area in `shell.rs`, and the
  test that the sidebar button stays inside the notch. `git apply -R` on that commit's files
  brings it back (expect conflicts from later work).
