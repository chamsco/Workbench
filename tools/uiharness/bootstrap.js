// uiharness/bootstrap.js — installed with
// Page.addScriptToEvaluateOnNewDocument, so it runs at document_start in every
// new document, before any of the page's own HTML is parsed.
//
// preload.js is inlined as the page's first <script> in <head> and therefore
// also runs before the app's scripts, but this one runs earlier still: it is
// the place for anything that must exist before the parser has done anything
// at all. Keep it small — it has no access to the DOM yet.
(function () {
  "use strict";

  // The app is a Tauri desktop window. A browser will not let it size itself,
  // so record what the page asked for rather than throwing it away silently.
  window.__BOOTSTRAP = {
    at: Date.now(),
    viewport: { width: window.innerWidth, height: window.innerHeight },
    devicePixelRatio: window.devicePixelRatio,
    userAgent: navigator.userAgent,
  };

  // Nothing in the UI should try to be a real desktop window.
  window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};

  // A page loaded from a file:// or unknown origin would be a harness bug;
  // say so now, with the URL, instead of failing 20s later on a missing flag.
  if (!/^https?:$/.test(location.protocol)) {
    console.error(
      "[uiharness] the page was not loaded over http(s): " + location.href +
        " — run it through tools/uiharness/run.mjs, which serves the UI root.",
    );
  }
})();
