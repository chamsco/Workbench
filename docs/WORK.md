# Work: what comes out of Workbench, and the Brainpowr stack around it

Work is the collaborative surface of Brainpowr: rooms where people talk to
people (h2h), people talk to agents (a2h) and agents talk to agents (a2a), in
any mix, under rules an org's admin sets. This note says which parts of
Workbench it reuses, what it adds, how it fits with Anarchy, and where the
plan is weakest.

The code is `crates/work` (`brainpowr-work`). The interface direction is
`design/work.html`.

## 1. What Workbench already gives us

| Piece | Where | Reused how |
|---|---|---|
| One turn on any model or agent, as one event stream: coding CLIs, Ollama, any OpenAI-compatible API (vLLM, OpenRouter, Mistral, xAI…), A2A agents | `crates/runner` (`backspace-runner`, MIT, knows nothing about chats) | **As is**, a dependency. Work never talks to a model any other way. |
| Token usage per turn (`Event::Usage`) | `crates/runner` | The meter books it against the payer. |
| Group turn-taking: @Name hands over, PASS leaves no message, a turn cap | `crates/core/src/chat.rs` (`reply_group`, `named_in`, `is_pass`, `group_transcript`) | **Ported** to `work::turns`, rewritten for several people (below). |
| Agents: name, job, avatar, model, rules on what they may not do, routines | `crates/core/src/agents.rs` | The shape is kept (`room::Who::Agent`); rules (`off`, `apps_off`) stay Workbench's until Work has its own tool layer. |
| The chat UI: bubbles grouped by sender, tapbacks, replies, link previews, attachments, route picker | `crates/tauri-app/ui/extensions/chat/` | Visual reference for `design/work.html`; not lifted, see §4. |
| Remote A2A agents brought in by card URL | `crates/core/src/fleet.rs`, `harnesses.rs` | Becomes a catalog entry (`Endpoint::A2a`). |

## 2. What Workbench cannot do, and what `crates/work` adds

**Workbench is single-user.** A message is `Role::User` or `Role::Assistant`.
There is no way to say *which* person wrote it, so it cannot hold h2h at all,
and every group assumes one human who is always the one being answered. That
is the main thing the extraction fixes.

| Module | What it is |
|---|---|
| `room` | A room has members; a member is a person or an agent. An agent has an `owner` (the person it works for) or none (a shared agent). `Room::mode()` reads Solo / H2h / A2h / A2a / Mixed from who is in it. |
| `turns` | `Floor`: who speaks next. Workbench's rules, plus: **with two or more people in a room, agents speak only when named**; **an agent that @names a person stops the chain** and waits for them; **a2a runs on a cap the caller sets**. Mentions match whole names only (`@Al` no longer wakes Alfred). The transcript an agent reads quotes others' messages as data, not instructions. |
| `governance` | The org's model **catalog** (each model with where it runs: External, OrgHosted = sovereign, OnDevice, and a credit weight), **grants** on the default, roles, teams, people and shared agents, an org **ceiling**, **invites** that carry role and teams, and `Governor::check` before every turn. |
| `meter` | `meter::turn`: check, run through the runner, book the usage (or an estimate, flagged, when the backend reports none). |

Rules as built (each has a test in `crates/work/tests`):

- Most specific grant wins: person → team → role → org default. Someone in
  several teams gets the most generous of those teams. The ceiling caps all.
- An agent working for a person spends **that person's** allowance and can
  use only that person's models. A shared agent spends its own pool; a shared
  agent with no grant gets nothing.
- A **Sealed** room (Anarchy's end-to-end state) only allows on-device models.
  External models need `external` allowed on the payer's grant.
- Allowances are in **credits** = tokens × the model's weight, per day, week
  or month (weeks start Monday UTC).

Example from the brief: Developer role, 1M credits/week, Claude + GPT +
sovereign; Accountant role, 50k/week, sovereign only, no external APIs.

## 3. The Brainpowr stack

```
 ┌──────────────────────────── Brainpowr Admin (governance console) ───────────────────────────┐
 │ org, workspaces, invites, roles, teams · model catalog · grants & quotas · usage · audit     │
 └───────────────▲───────────────────────────────────────────────────────▲────────────────────┘
                 │ policy (Governor)                                       │ ledger, audit
 ┌───────────────┴──────────────┐   ┌─────────────────────────────────────┴──────────────────┐
 │ Work (rooms, Floor, meter)    │──▶│ Model gateway: backspace-runner behind the org's keys  │
 │ h2h · a2h · a2a               │   │ Claude · ChatGPT · vLLM/Ollama (sovereign) · A2A agents │
 └───────────────▲──────────────┘   └────────────────────────────────────────────────────────┘
                 │ messages, members, presence, files
 ┌───────────────┴──────────────────────────────────────────────────────────────────────────┐
 │ Anarchy: identity (OIDC), orgs, spaces, invites, MLS end-to-end messaging, drive, calls,  │
 │ sidekicks (agents as members, `agent_of`), Company Brain + Company MCP (audience-scoped)   │
 └──────────────────────────────────────────────────────────────────────────────────────────┘
                                   "Cloud OS": hosting the three above per org
```

- **Anarchy owns people and transport.** It already has orgs, users, spaces,
  expiring invites (guests, hashed codes), sidekicks as accounts tied to a
  person (`agent_of`), and the Company Brain with audience scoping.
  Workbench has none of it, so rebuilding it from Workbench would be a step
  back. What Anarchy lacks is roles and teams: its schema has neither, and
  its invites carry no role. Those are the first migration Work needs there.
- **Work owns agent behaviour in a room**: who speaks, what they read, what
  it costs. It maps onto Anarchy one to one: a room is a channel, a member is
  a user (sidekick or not), `Who::Agent.owner` is `users.agent_of`, `Trust`
  is the channel's trust state.
- **The gateway is the only path to a model.** Keys live there (`key_env`),
  never in policy or on laptops. This is what makes quotas real (§5).
- **Admin** edits `Policy` and the catalog and reads the ledger. Invites are
  Anarchy's `invites` table plus a role and teams.

## 4. Interface direction (`design/work.html`)

From the references: one glass window on a soft sky (define, Glass); the
sidebar of the Alfred mock: pinned agents as tiles, then people and agents
grouped by team, each with a role chip and a status line; Mercury's "Chief of
Staff" builder for creating agents (not drawn yet); Glass's centred composer
with the model under it.

What is specific to Work and drawn:

- Agents and people look different (rounded square vs circle), and each agent
  message says whom it works for and which model it ran on.
- The room header says its mode and trust ("Mixed · Company").
- A handoff card when an agent asks a person something.
- The composer's model picker lists only what the admin granted, marks each
  model External / Sovereign / On device, and shows what is left of the week.
- Admin: usage per person and shared agent, grants by role and team, the
  catalog, and invites with role, team, expiry and uses.

## 5. Where this plan is weak

1. **Quotas only bind what goes through the gateway.** A developer running
   Claude Code or Codex on their own subscription spends tokens the org never
   sees. Enforcing "1M a week" means company work goes through org API keys,
   and CLIs on personal plans are either banned for company data or simply
   not counted. `meter` says this in its docs; the admin UI should too.
2. **Some backends report no usage.** A2A agents (and some CLIs) send no
   token counts; those turns are estimated at 4 chars/token and flagged.
   Estimates can be gamed. Pay A2A agents by the call instead if that matters.
3. **"Sovereign AIs with no restraint" is the riskiest line in the brief.**
   In an enterprise product, an unaligned model with tools, company data and
   agents acting on someone's behalf is a liability for the customer and for
   you, and "governance" is exactly what a buyer is paying to get. Sell
   *sovereign* as residency and control (the org runs it, data never leaves),
   and keep the controls that do not depend on the model: tool permissions,
   approvals before actions (Anarchy's AgentApproval, designed but not yet
   built), audience-scoped
   retrieval, and an audit log. The prototype says so on the Models page.
4. **Tokens are the wrong unit across models.** 1M tokens of a frontier model
   costs orders of magnitude more than 1M of a local 8B. Allowances are in
   weighted credits for that reason; if you want the admin to type "tokens",
   set every weight to 1 and accept the distortion.
5. **The check runs before a turn, not during it.** A long reply that starts
   with 10 credits left finishes and goes over. Fine for a week's budget; a
   hard per-turn cap needs `max_tokens` passed down to the runner.
6. **Licences.** Workbench is MIT, Anarchy is AGPL-3.0. Linking Anarchy's
   crates into a Work binary makes that binary AGPL, including for hosted
   customers. Keep them as separate services talking over an API, or relicense
   (Anarchy has a CLA, so its owners can).
7. **End-to-end encryption and server-side agents pull against each other.**
   A hosted agent can only read a room by being a member with keys (Anarchy's
   Brain/sidekick model). Sealed rooms get on-device models only, which is what
   `Trust::Sealed` enforces; no shortcut through the server.
8. **a2a has no human brake.** The cap and the shared agent's own pool are the
   brakes; an overnight a2a room should also need an owner who can stop it.
9. **Several teams, most generous wins.** That is a choice, and the opposite
   is defensible. A person-level grant overrides both.

## 6. Next

1. Add roles, teams and a policy column to Anarchy (`orgs.policy_json` is in
   its architecture doc but not its schema), and a role on invites.
2. Wire `crates/work` to Anarchy's server as a service: rooms ← channels,
   members ← channel members + sidekicks.
3. Put the gateway in front of the runner with the org's keys, and make
   Workbench's Cloud route one of its tenants.
4. Persist the ledger (Postgres) and expose usage to Admin.
5. Agent builder (the Chief of Staff flow) writing `Who::Agent` with a
   catalog model the creator is allowed to use.
