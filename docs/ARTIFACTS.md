Artifacts: what an answer is about, as a thing

`crates/core/src/artifact.rs`. An agent's reply is prose; the object it refers to
— the diff it wrote, a screenshot, a table, a document, the options it wants a
decision on, the trace of what it ran — is an artifact: addressable, attributed,
and drawn under the session rather than transcribed into the answer.

This is the one useful lesson from T3 Code's architecture (`docs/internals/
overview.md`): a thread is the unit, and its output is structured objects with
one renderer per kind. It is not their stack — execution stays in our harness —
only the shape.

## The record

```rust
pub struct Artifact {
    id: String,          // "art1", stable in the project; how a message refers to it
    by: String,          // the agent key that produced it
    kind: String,        // diff | image | options | trace | document | table
    title: String,       // what the user reads
    ticket: Option<String>,
    path: Option<String>,  // a file in the project (image, document)
    body: Option<String>,  // inline text (diff, document, trace excerpt)
    data: Option<String>,  // a data: URL (image)
    stat: Option<String>,  // for diff: one line per file
    columns: Vec<String>,  // for table
    rows: Vec<Vec<String>>,
    options: Option<String>, // for options: the OptionSet id
    trace: Option<String>,   // for trace: the trace id
    at: u64,
}
```

`kind` is a string, not an enum, on purpose: a newer shell or an app's tool can
record a kind this build has never heard of, the older shell lists it by name
instead of failing to deserialize the whole project, and no migration is needed
to add one.

## Rules

- **Validation is per kind**, in `artifact::add`, so a half-filled record cannot
  reach a renderer: a diff needs a body or a path, an image a path or data, an
  `options` artifact needs a set that actually exists, a trace an id, a table a
  header, and every table row must match that header.
- **Bounded**: `MAX_ARTIFACTS` 120 per project (oldest falls off), `MAX_BODY`
  200k chars, `MAX_ROWS` 200. The list is serialized into every snapshot, so it
  cannot grow without bound.
- **On the project state**, so it reaches every shell in the same snapshot as the
  board, the tickets and the agents. No new channel.
- **Addressable**: `artifact::get`, `by` (an agent's session view),
  `for_ticket` (the Workbench view).

## Producers

- The `produce` tool, for any agent. One call per artifact, `required: [kind]`.
- `Harness::produce(Artifact)`, for the shell's own affordances (a screenshot the
  user dropped in), and what the tests drive.

`show_options` predates this and still works; recording the set as an artifact
with `kind: "options"` and the set's id is what makes it addressable.

## Renderers

`app.js`: `artifactHTML` (the cards under a session), `artifactCard` (the header
with kind, title and a summary), `artifactBodyHTML` (one per kind — a coloured
diff, a sticky-header table, an image, a read-the-file button), and
`bindArtifacts`. A kind with no renderer still gets a card and a
"nothing to show for `<kind>`" body. Reading a file an artifact points at is the
`read_file` command, so the UI never touches disk itself.

## What is not done

- An artifact is not yet attached to a *message*, so a reply cannot yet say
  "here, this one" inline. Today it is attached to the agent (the session view)
  and to the ticket (the Workbench view).
- Images render from a `data:` URL or through `read_file`; there is no thumbnail
  resizing, so a large screenshot is carried whole.
- Checkpointing (T3 snapshots workspace state with hidden Git refs before review)
  is not implemented; we use a commit plus a worktree, which is the same idea
  with more visible scars.
