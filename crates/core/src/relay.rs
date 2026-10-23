//! Harness tiles talking to each other.
//!
//! The IDE can tile several harnesses side by side (Claude Code in one canvas,
//! Codex in the next...), each in its own chat thread. This is how they reach
//! one another:
//!
//! - The app writes the open tab's tiles to `<data>/relay/tiles.json`: each
//!   tile's thread, its short name (`claude`, `codex-2`) and whether it is the
//!   tab's coordinator.
//! - A tile's CLI gets `list_tiles` and `message_tile` through `backspace mcp`
//!   (it is started with [`ENV_TILE`] set to its own thread). `message_tile`
//!   drops a letter in `<data>/relay/outbox/`; the app picks letters up and
//!   delivers them (`Chats::deliver`).
//! - A letter becomes the receiving tile's next turn. Nothing reaches a CLI in
//!   the middle of a turn: each turn is its own headless process, so a letter
//!   for a busy tile waits until that turn ends.
//! - An answer goes back to the sender when it asked for one. Every hop is
//!   counted, and a chain stops at [`MAX_HOPS`], so two tiles can't keep each
//!   other busy forever.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

/// The thread a tile's CLI answers in, for `backspace mcp`.
pub const ENV_TILE: &str = "BACKSPACE_TILE";

/// How many hand-offs one thing the user asked may cause.
pub const MAX_HOPS: u8 = 6;

/// One harness tile in the open tab.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tile {
    /// Its chat thread.
    pub thread: String,
    /// What the others call it: lowercase letters, digits and dashes.
    pub label: String,
    /// What runs it, for people and models to read ("Claude Code", "Codex · gpt-5").
    #[serde(default)]
    pub runs: String,
    /// The tab's coordinator: it hands work out and gets every answer back.
    #[serde(default)]
    pub coordinator: bool,
}

/// A message from one tile to another, on its way.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Letter {
    /// The sender's thread.
    pub from: String,
    /// A label or a thread id.
    pub to: String,
    pub text: String,
    /// Send the receiver's answer back to the sender.
    #[serde(default = "yes")]
    pub reply: bool,
}

fn yes() -> bool {
    true
}

fn dir(data: &Path) -> PathBuf {
    data.join("relay")
}

/// A label anyone can type after an `@`.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(24).collect()
}

/// The open tab's tiles, as the app last wrote them.
pub fn tiles(data: &Path) -> Vec<Tile> {
    std::fs::read(dir(data).join("tiles.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Write the open tab's tiles. Labels are made unique and typeable, and at
/// most one tile coordinates.
pub fn set_tiles(data: &Path, mut list: Vec<Tile>) -> Result<Vec<Tile>> {
    let mut seen = std::collections::HashSet::new();
    let mut coordinated = false;
    for t in &mut list {
        let base = Some(slug(&t.label))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "tile".into());
        let mut label = base.clone();
        let mut n = 2;
        while !seen.insert(label.clone()) {
            label = format!("{base}-{n}");
            n += 1;
        }
        t.label = label;
        t.coordinator = t.coordinator && !coordinated;
        coordinated |= t.coordinator;
    }
    std::fs::create_dir_all(dir(data))?;
    let tmp = dir(data).join("tiles.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&list)?)?;
    std::fs::rename(&tmp, dir(data).join("tiles.json"))?;
    Ok(list)
}

/// The tile a name points at: its label (with or without the `@`) or its thread.
pub fn find<'a>(list: &'a [Tile], name: &str) -> Option<&'a Tile> {
    let n = name.trim().trim_start_matches('@');
    list.iter()
        .find(|t| t.thread == n)
        .or_else(|| list.iter().find(|t| t.label.eq_ignore_ascii_case(n)))
        .or_else(|| list.iter().find(|t| t.label == slug(n)))
}

/// `@codex fix the test` typed in a tile: the tile it names and the rest.
/// Only another tile in the tab counts; `@claude` naming no tile is left to
/// chat's own mention (another CLI answering in the same thread).
pub fn mention<'a>(list: &'a [Tile], me: &str, text: &str) -> Option<(&'a Tile, String)> {
    let w = text.trim_start().strip_prefix('@')?;
    let name: String = w
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let t = find(list, &name).filter(|t| t.thread != me)?;
    let rest = w[name.len()..]
        .trim_start_matches([',', ':'])
        .trim()
        .to_string();
    Some((t, rest))
}

/// Post a letter from a tile's CLI (`backspace mcp` is its own process, so it
/// never touches a thread itself).
pub fn post(data: &Path, l: &Letter) -> Result<()> {
    if l.text.trim().is_empty() {
        bail!("say something");
    }
    let list = tiles(data);
    let to = find(&list, &l.to).ok_or_else(|| {
        let names: Vec<_> = list
            .iter()
            .filter(|t| t.thread != l.from)
            .map(|t| t.label.as_str())
            .collect();
        anyhow!(
            "no tile called `{}`; the tab has: {}",
            l.to,
            if names.is_empty() {
                "nobody else".into()
            } else {
                names.join(", ")
            }
        )
    })?;
    if to.thread == l.from {
        bail!("that is you");
    }
    let out = dir(data).join("outbox");
    std::fs::create_dir_all(&out)?;
    // Sorted by name when drained: the counter keeps one millisecond's letters in order.
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 1_000_000;
    let name = format!(
        "{:013}-{seq:06}-{}",
        crate::chat::now_ms(),
        crate::chat::new_id()
    );
    let tmp = out.join(format!("{name}.tmp"));
    std::fs::write(
        &tmp,
        serde_json::to_vec(&Letter {
            to: to.thread.clone(),
            ..l.clone()
        })?,
    )?;
    std::fs::rename(&tmp, out.join(format!("{name}.json")))?;
    Ok(())
}

/// Letters waiting, oldest first; taking them removes them.
pub fn drain(data: &Path) -> Vec<Letter> {
    let out = dir(data).join("outbox");
    let Ok(rd) = std::fs::read_dir(&out) else {
        return vec![];
    };
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|p| {
            let l = std::fs::read(&p)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            let _ = std::fs::remove_file(&p);
            l
        })
        .collect()
}

/// `list_tiles`: the others, as a harness reads them.
pub fn list(data: &Path, me: &str) -> Result<String> {
    let all = tiles(data);
    let others: Vec<String> = all
        .iter()
        .filter(|t| t.thread != me)
        .map(|t| {
            format!(
                "- {}{}{}",
                t.label,
                if t.runs.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", t.runs)
                },
                if t.coordinator { ", coordinator" } else { "" }
            )
        })
        .collect();
    if others.is_empty() {
        return Ok("No other harness is tiled next to you.".into());
    }
    Ok(others.join("\n"))
}

/// `message_tile`: post a letter and say what happens next.
pub fn send(data: &Path, me: &str, to: &str, text: &str, reply: bool) -> Result<String> {
    post(
        data,
        &Letter {
            from: me.into(),
            to: to.into(),
            text: text.into(),
            reply,
        },
    )?;
    let name = find(&tiles(data), to)
        .map(|t| t.label.clone())
        .unwrap_or_else(|| to.into());
    Ok(if reply {
        format!("Sent to `{name}`. Its answer comes back to you as a message once it has answered and your turn has ended.")
    } else {
        format!("Sent to `{name}`.")
    })
}

/// `backspace tile list | send <name> <text>`, for CLIs without MCP.
pub fn cli(args: &[String]) -> Result<String> {
    let me = std::env::var(ENV_TILE)
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("not started in a harness tile"))?;
    let data = crate::prefs::Prefs::data_dir();
    match args.first().map(String::as_str) {
        Some("list") => list(&data, &me),
        Some("send") if args.len() >= 3 => send(&data, &me, &args[1], &args[2..].join(" "), true),
        _ => bail!("usage: tile list | tile send <name> <text>"),
    }
}

/// What a tile's harness is told about the others, added to its brief.
pub fn brief(list: &[Tile], me: &str) -> Option<String> {
    let mine = list.iter().find(|t| t.thread == me)?;
    let others: Vec<&Tile> = list.iter().filter(|t| t.thread != me).collect();
    if others.is_empty() {
        return None;
    }
    let mut s = format!(
        "# Other harnesses\n\nYou are `{}`, one of {} coding harnesses tiled side by side in the user's IDE, each with its own session. The others:\n\n",
        mine.label,
        list.len()
    );
    for t in &others {
        s.push_str(&format!(
            "- `{}`{}{}\n",
            t.label,
            if t.runs.is_empty() {
                String::new()
            } else {
                format!(" ({})", t.runs)
            },
            if t.coordinator {
                ", the coordinator"
            } else {
                ""
            }
        ));
    }
    s.push_str(
        "\nTo hand one of them something, call the `message_tile` tool with its name. It becomes their next turn \
         (it waits if they are busy) and their answer comes back to you as a message. Send them what they need to \
         act without your context. Don't message a harness just to say thanks or acknowledge something.\n",
    );
    if mine.coordinator {
        s.push_str(
            "\nYou are the coordinator. Split what the user asks into pieces for the others, send each piece to the \
             harness best placed for it, wait for their answers, then check and report. Do yourself only what is \
             quicker than explaining.\n",
        );
    } else if let Some(c) = others.iter().find(|t| t.coordinator) {
        s.push_str(&format!(
            "\n`{}` coordinates: when it hands you something, do it and answer it plainly.\n",
            c.label
        ));
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bs-relay-{name}-{}", crate::chat::now_ms()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn tile(thread: &str, label: &str, coordinator: bool) -> Tile {
        Tile {
            thread: thread.into(),
            label: label.into(),
            runs: String::new(),
            coordinator,
        }
    }

    #[test]
    fn labels_are_unique_and_one_tile_coordinates() {
        let d = tmp("labels");
        let out = set_tiles(
            &d,
            vec![
                tile("a", "Claude Code", true),
                tile("b", "claude code", true),
                tile("c", "", false),
            ],
        )
        .unwrap();
        assert_eq!(
            out.iter().map(|t| t.label.as_str()).collect::<Vec<_>>(),
            ["claude-code", "claude-code-2", "tile"]
        );
        assert_eq!(out.iter().filter(|t| t.coordinator).count(), 1);
        assert_eq!(tiles(&d), out);
    }

    #[test]
    fn a_mention_names_another_tile_only() {
        let list = vec![tile("a", "claude", false), tile("b", "codex", false)];
        let (t, rest) = mention(&list, "a", "@codex, fix the failing test").unwrap();
        assert_eq!(
            (t.thread.as_str(), rest.as_str()),
            ("b", "fix the failing test")
        );
        assert!(
            mention(&list, "a", "@claude look").is_none(),
            "naming yourself is not a hand-off"
        );
        assert!(
            mention(&list, "a", "@cursor look").is_none(),
            "no such tile: chat's own @mention applies"
        );
        assert!(mention(&list, "a", "fix @codex").is_none());
    }

    #[test]
    fn letters_go_through_the_outbox_once_in_order() {
        let d = tmp("outbox");
        set_tiles(
            &d,
            vec![tile("a", "claude", false), tile("b", "codex", false)],
        )
        .unwrap();
        post(
            &d,
            &Letter {
                from: "a".into(),
                to: "@codex".into(),
                text: "one".into(),
                reply: true,
            },
        )
        .unwrap();
        post(
            &d,
            &Letter {
                from: "a".into(),
                to: "b".into(),
                text: "two".into(),
                reply: false,
            },
        )
        .unwrap();
        assert!(post(
            &d,
            &Letter {
                from: "a".into(),
                to: "claude".into(),
                text: "me".into(),
                reply: true
            }
        )
        .is_err());
        let e = post(
            &d,
            &Letter {
                from: "a".into(),
                to: "nobody".into(),
                text: "x".into(),
                reply: true,
            },
        )
        .unwrap_err();
        assert!(e.to_string().contains("codex"), "{e}");
        let got = drain(&d);
        assert_eq!(
            got.iter()
                .map(|l| (l.to.as_str(), l.text.as_str()))
                .collect::<Vec<_>>(),
            [("b", "one"), ("b", "two")]
        );
        assert!(drain(&d).is_empty());
    }

    #[test]
    fn the_brief_names_the_others_and_the_coordinator() {
        let list = vec![tile("a", "claude", true), tile("b", "codex", false)];
        let a = brief(&list, "a").unwrap();
        assert!(a.contains("`codex`") && a.contains("You are the coordinator"));
        let b = brief(&list, "b").unwrap();
        assert!(b.contains("`claude` coordinates"));
        assert!(
            brief(&[tile("a", "claude", false)], "a").is_none(),
            "alone: nothing to say"
        );
        assert!(brief(&list, "z").is_none());
    }
}
