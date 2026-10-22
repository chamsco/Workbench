# Backspace for iPhone

A SwiftUI client for the desktop app's phone API (`docs/companion.md`,
protocol 1). The Mac does the work; the phone pairs with it on the local
network and reads chats, sends messages, and approves what agents deliver.

## Build

```sh
brew install xcodegen
cd mobile/ios
xcodegen generate          # writes Backspace.xcodeproj and Backspace/Info.plist
open Backspace.xcodeproj   # pick your team under Signing to run on a phone
```

iOS 17 or later. CI (`.github/workflows/ios.yml`) builds it, runs the unit
tests on a simulator and uploads screenshots of the demo as `ios-screenshots`.

## Pairing

On the Mac: Settings → Phone → *Reachable from my network*, then *Let my
phone connect*. Scan the QR code (or paste the `backspace://pair` link, or
type the address and token). The phone checks `/v1/hello`, then keeps the
address and token in the keychain. *Try a demo* runs the whole app against
canned data without a Mac.

## Layout

The frame follows the phone mockups: menu, orb and machine across the top,
a floating **Chat / Tasks / Make** bar at the bottom, and the chats in a
drawer from the left.

| Where | Shows | API |
|---|---|---|
| Chat | The open thread, tapbacks and Save to Memory on long-press, a composer that turns into Stop | `/v1/chat`, `/v1/chat/send`, `/react`, `/stop`, `POST /v1/memory` |
| Tasks → Active | The open project's pending approvals. Swipe right approves, left sends back with feedback | `/v1/project`, `/v1/approve`, `/v1/reject` |
| Tasks → Up next | The project's open tickets, work in motion first | `/v1/project` |
| Make | A prompt box, templates that fill it, earlier chats | `/v1/chat/new` |
| Drawer | Pinned chats, then the rest, and New chat (pick the model) | `/v1/chats`, `/v1/routes` |

It polls (every 2.5 s, 0.9 s while a reply streams) until the server-sent
events in the companion plan exist.

## Launch arguments

For the simulator and the CI screenshots: `-demo YES`, `-startTab tasks|make`,
`-startChat <id>`, `-startDrawer YES`.
