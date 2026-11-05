# open-bot-avatar (vendored)

Source: <https://github.com/DominikScholz/open-bot-avatar>
Package: `open-bot-avatar@0.1.2` (MIT)

The prototype uses the real renderer rather than a re-creation. Files:

- `package/` — the published npm tarball, extracted unchanged (`dist`, `src`, `README.md`, `LICENSE`).
- `gallery/brain.json`, `gallery/cat.json`, `gallery/star.json` — definitions copied verbatim from the repository's `gallery/` folder.
- `entry.js` — re-exports the package API plus the gallery definitions.
- `open-bot-avatar.js` — `entry.js` bundled by esbuild as a classic-script IIFE (`window.OpenBotAvatar`), so the prototype also works from `file://` where ES module imports are blocked.

Rebuild the bundle after changing the entry or vendored files:

```sh
npx esbuild design/vendor/open-bot-avatar/entry.js \
  --bundle --format=iife --global-name=OpenBotAvatar \
  --outfile=design/vendor/open-bot-avatar/open-bot-avatar.js
```
