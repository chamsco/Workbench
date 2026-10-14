Skills: where they live, how to add one, how they are found

A skill is a folder with a `SKILL.md` in it. The frontmatter's `description` is
what the agent's index shows; the body is what it reads once it decides the
skill fits. Nothing else is required.

## Where they come from

Three places, read in this order, later winning:

| Origin | Path | Shown in Settings as |
|---|---|---|
| Built-in | compiled into the binary (`crates/core/src/skills.rs`, `DEFAULTS`) | Built-in |
| Yours | `~/.config/backspace/skills/<name>/SKILL.md` | Yours |
| This project | `<project>/.backspace/skills/<name>/SKILL.md` | This project |

A folder with the same name as a built-in **replaces** it, it does not sit
beside it. That is how a project overrides `tdd` or `code-review` with its own
version without forking the app, and how you try an edit to a vendored skill
before offering it upstream.

Settings → Agent skills lists every skill with its origin, whether it is
injected into a role, and a search box over names, descriptions and bodies.
**Open my skills folder** creates `~/.config/backspace/skills/` if it does not
exist and reveals it; **Copy the template** copies this:

```
---
name: my-skill
description: One line on when to use this, as the agent sees it.
---

# My skill

What to do, in the order to do it. Keep it short; the agent reads this when it
decides the skill fits.
```

## Which roles get which

`[workflow]` in `backspace.toml` decides what is *injected* into a role's system
prompt: `main_skills`, `worker_skills`, `triage_skills`. Injection is not the
only way in — any agent can load any skill with the `skill` tool, whether or not
it is injected. Injection is for the skills a role should always have in mind.

## Shape of a folder

```
~/.config/backspace/skills/my-skill/
  SKILL.md            required; the frontmatter and the instructions
  AGENTS.md           optional; extra files the skill refers to
  reference/*.md      optional; anything else, read on demand with `file`
```

Every file in the folder becomes part of the skill. `SKILL.md` is read first;
the rest are listed for the agent to read when it needs them, so a skill can
carry a lot without costing tokens until it is used.

## Adding a library to the repo

If skills should ship with the app rather than with a machine, put them in the
tree and add them to `DEFAULTS` in `crates/core/src/skills.rs`:

```rust
const DEFAULTS: &[(&str, Files)] = vendored! {
    "my-skill" => ["SKILL.md", "AGENTS.md"],
};
```

`vendored!` embeds the files at build time, so a skill added this way is present
on every install with no network and no folder to create. A skills *library* in
the repo (a folder of candidate skills, installable but not shipped) would be
the next step: read it the way `Apps` reads its catalog, and copy a chosen one
into the user's folder.

## Adding a skill for other harnesses

The vendored skills were written for other tools. `ADAPTER` in
`skills.rs` translates their vocabulary — "the Skill tool", "the issue tracker",
"dispatch a sub-agent" — onto Backspace's tools, and is appended to the body the
agent reads. A skill written for another harness needs no edit: add the mapping
there if its words differ.
