# Phone companion

A Backspace phone app that keeps you in the loop away from the desk: read
and answer chats, react, save notes to Memory, and approve or reject a
project's tickets and merges. The desktop app is the server; the phone is a
thin client on the same network.

Status: **the desktop side is built** (Settings → Phone, `crates/core/src/companion.rs`).
The phone app is not started yet; this file is its contract and plan.

## Pairing

1. Settings → Phone → *Let my phone connect*. Backspace listens on
   `0.0.0.0:7421` and shows a QR code.
2. The QR holds `backspace://pair?v=1&url=<http://lan-ip:7421>&token=<token>&name=<machine>`.
3. The phone checks `GET <url>/v1/hello` (no token) for `kind: "companion"`
   and a `protocol` it speaks, then stores `url` and `token` in the keychain.
4. *Unpair all phones* issues a new token; every paired phone has to scan
   again.

Away from home, put both on a private network (Tailscale, WireGuard) and pair
with that address; there is no relay yet.

## API (protocol 1)

Every call but `/v1/hello` sends `Authorization: Bearer <token>`. Bodies are
JSON. Errors are `{ "error": "..." }` with 400, 401 or 404.

| Call | Does |
|---|---|
| `GET /v1/hello` | `{ app, kind: "companion", protocol, version, name }` |
| `GET /v1/chats` | Chat threads, newest first: `{ id, title, updated, pinned, route, preview, busy, unread, project }` (apps' threads are left out) |
| `GET /v1/chat?id=` | One thread with its messages (`role`, `text`, `status`, `reactions`, `reply_to`, `attachments`, `ad`...) |
| `POST /v1/chat/new` | `{ route?, project? }` → the new thread. No route: the user's default |
| `POST /v1/chat/send` | `{ id, text, reply_to? }`. The reply streams into the thread; poll `/v1/chat` |
| `POST /v1/chat/react` | `{ id, msg, emoji }` (one tapback per message; the same one again removes it) |
| `POST /v1/chat/stop` | `{ id }` |
| `GET /v1/routes` | Harnesses switched on that can chat: `{ id, name, kind, models }` |
| `GET /v1/memory` | Memory notes |
| `POST /v1/memory` | `{ text, project? }` → the note (source "phone") |
| `GET /v1/project` | `{ open: false }`, or `{ open: true, workspace, state }` with the project's agents, tickets and approvals |
| `POST /v1/approve` | `{ id }`: approve a pending approval |
| `POST /v1/reject` | `{ id, feedback }` |

## Plan

1. **Phone app v1** (React Native or SwiftUI + Kotlin; pick when it starts):
   pairing by QR, chat list and thread with the same tapbacks, reply-to and
   link previews, a composer, approvals inbox, Memory quick-add.
2. **Live updates**: replace polling with a server-sent events stream
   (`GET /v1/events`) that says which thread or project changed.
3. **Push notifications**: "a ticket waits on you", "a reply finished",
   through a small relay (APNs/FCM need one), opt-in per machine.
4. **Away from the LAN**: an end-to-end-encrypted relay so pairing works
   without Tailscale; the token becomes a key pair made at pairing.
5. **Attachments and voice**: photos from the phone into a chat; dictation.
6. **Several machines**: the phone keeps a list, like the desktop's
   Machines.

Open questions: whether Cloud plans should work from the phone without the
desktop running (that is the Cloud API, not this one), and how much of Code
(diffs, the diagram) is worth a small screen.
