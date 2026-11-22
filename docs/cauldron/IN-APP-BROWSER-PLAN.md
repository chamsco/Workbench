# In-app browser plan

This plan covers the native browser opened from a Cauldron stream link. The first Windows smoke
test on PR #75 showed that the initial implementation could terminate the app while creating
WebView2, so native browser work is gated on process safety before feature polish.

## This PR: make native creation safe

- [x] Keep layout pure: rendering records the desired URL and rectangle but does not call WebView2
      or WKWebView APIs.
- [x] Create, resize, navigate and show the native view from a delayed UI-loop task after GPUI has
      completed the frame and returned from its native event dispatch.
- [x] Disable GPUI DirectComposition on Windows, the compatibility path required for a WebView2
      child surface in this renderer.
- [x] Give the webview an app-owned data directory instead of sharing an implicit WebView2 profile.
- [x] Avoid retrying a failed native creation every frame; closing the panel clears the failure so a
      later open can retry.
- [x] Add lifecycle tests for request coalescing, URL/rectangle changes and failure reset.
- [x] Run the Windows link smoke test: free OpenCode reply → click URL → Example Domain rendered in
      the embedded pane, the process remained alive, and close/reopen worked.

## Next: platform smoke coverage

- Add a Windows desktop smoke job that builds the release app and exercises WebView2 with a local
  deterministic page. It must fail on process exit, not only on a returned Rust error.
- Add a macOS smoke job for WKWebView creation, resize, navigation and close.
- Record WebView2/WKWebView availability as a visible capability error; never silently terminate or
  fall back without telling the user.
- Add navigation policy tests for `http`/`https`, external links, invalid URLs and page load errors.

## Linux: WebKitGTK implementation

### X11 first

1. Add the Linux-only WebKitGTK dependency and initialize GTK on the thread that owns the native
   webview.
2. Host the webview in a GTK native child surface and keep GPUI responsible for the surrounding
   panel, bounds and visibility.
3. Pump GTK's main context alongside GPUI without blocking the GPUI event loop; send navigation,
   bounds and visibility commands to the GTK-owning thread.
4. Support X11 first because native child embedding is the smallest path to a reliable Linux
   result. Keep the system-browser action available while the native view is unavailable.

### Wayland

1. Verify whether the GPUI backend and WebKitGTK can share a child surface on the supported Wayland
   compositors.
2. If child embedding is unavailable, use a positioned GTK overlay only as an interim compatibility
   layer; otherwise keep the system browser fallback.
3. Do not advertise full Linux support until X11 and Wayland have launch, navigation, resize and
   close smoke coverage.

## Browser polish after stability

- Back, forward and reload controls.
- Address field with explicit navigation.
- Loading, error and offline states inside the support card.
- A bounded user-data directory and cleanup policy.
- External navigation policy and an explicit “open in system browser” action.

## Exit criteria

The feature is ready to call working when a release build passes the targeted Rust tests, the
Windows and macOS native smoke jobs, Linux X11 and Wayland smoke jobs, and the manual flow keeps
Cauldron alive while opening, resizing, closing and reopening a real page.
