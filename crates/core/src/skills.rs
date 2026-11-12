//! Skills: markdown playbooks an agent loads on demand with the `skill` tool.
//! Defaults are vendored from github.com/mattpocock/skills (MIT, see
//! skills/LICENSE-mattpocock-skills) and github.com/DietrichGebert/ponytail
//! (MIT, see skills/LICENSE-ponytail). A folder with the same name under
//! `~/.config/backspace/skills/` or `<workspace>/.backspace/skills/` replaces
//! a default or adds a new skill, so you can make them your own.

use std::collections::BTreeMap;
use std::path::Path;

macro_rules! vendored {
    ($($skill:literal => [$($file:literal),*]),* $(,)?) => {
        &[$(($skill, &[$(($file, include_str!(concat!("../skills/", $skill, "/", $file)))),*])),*]
    };
}

type Files = &'static [(&'static str, &'static str)];

/// Where a user's own skills live: `~/.config/backspace/skills/`.
pub fn user_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(std::path::PathBuf::from(home).join(".config/backspace/skills"))
}

const DEFAULTS: &[(&str, Files)] = vendored! {
    "grilling" => ["SKILL.md"],
    "to-spec" => ["SKILL.md"],
    "to-tickets" => ["SKILL.md"],
    "triage" => ["SKILL.md", "AGENT-BRIEF.md", "OUT-OF-SCOPE.md"],
    "tdd" => ["SKILL.md", "tests.md", "mocking.md"],
    "diagnosing-bugs" => ["SKILL.md"],
    "code-review" => ["SKILL.md"],
    "ponytail" => ["SKILL.md", "AGENTS.md"],
};

/// How the vendored skills' vocabulary maps onto Backspace. They were written
/// for other harnesses; translating here keeps the files verbatim and easy to
/// update from upstream.
pub const ADAPTER: &str = "Skills were written for a general coding agent. In Backspace: \"the Skill tool\" is the `skill` tool; \"the issue tracker\" and \"tickets\" are Backspace tickets (create_tickets, file_ticket, triage_ticket); \"dispatch a sub-agent\" means create_tickets + work_tickets if you have them, otherwise do it yourself; triage labels are ticket states (needs_triage, needs_info, ready_for_agent, ready_for_human, wontfix). Ignore setup steps such as /setup-matt-pocock-skills and anything about posting comments to GitHub. The ponytail skill's slash commands map onto this tool: `/ponytail` is loading it, `/ponytail lite|full|ultra` is the level in its body, and `/ponytail-review`, `/ponytail-audit`, `/ponytail-debt` and `/ponytail-gain` are just that level applied to a review of the diff, a read of the codebase, the shortcuts already in the code, or the size of the change you are about to make.";

#[derive(Clone, Debug)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub files: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct Skills {
    pub by_name: BTreeMap<String, Skill>,
}

/// Where skills come from, in load order: the app's own, then the user's, then
/// the workspace's. Later ones win, which is how a workspace overrides a
/// default and how a user overrides both.
#[derive(Clone, Debug)]
pub struct Found {
    pub name: String,
    pub description: String,
    pub files: Vec<String>,
    /// "built-in", "yours" (the user folder) or "workspace".
    pub origin: String,
    /// Absolute path, when it came from a folder rather than the binary.
    pub dir: Option<std::path::PathBuf>,
}

impl Skills {
    /// Every skill this machine can load, for the Settings list and for search.
    pub fn found(&self, workspace: &Path) -> Vec<Found> {
        let user = user_dir();
        let ws_dir = workspace.join(".backspace/skills");
        let mut out: Vec<Found> = self
            .by_name
            .values()
            .map(|s| {
                let dir = user
                    .as_ref()
                    .map(|u| u.join(&s.name))
                    .filter(|d| d.join("SKILL.md").is_file())
                    .or_else(|| {
                        let d = ws_dir.join(&s.name);
                        d.join("SKILL.md").is_file().then_some(d)
                    });
                let origin = match &dir {
                    Some(d) if ws_dir.join(&s.name) == *d => "workspace",
                    Some(_) => "yours",
                    None => "built-in",
                };
                Found {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    files: s.files.keys().cloned().collect(),
                    origin: origin.to_string(),
                    dir,
                }
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Search names, descriptions and bodies. Case-insensitive substring, which
    /// is what the Settings search box needs; ranked by where the match is
    /// (name first), then by name.
    pub fn search(&self, query: &str) -> Vec<(String, String)> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(u8, String, String)> = Vec::new();
        for s in self.by_name.values() {
            let name = s.name.to_lowercase();
            let desc = s.description.to_lowercase();
            let body = s
                .files
                .values()
                .map(|v| v.to_lowercase())
                .collect::<Vec<_>>()
                .join("\n");
            let (rank, where_) = if name.contains(&q) {
                (0, "the name")
            } else if desc.contains(&q) {
                (1, "the description")
            } else if body.contains(&q) {
                (2, "the body")
            } else {
                continue;
            };
            hits.push((rank, s.name.clone(), where_.to_string()));
        }
        hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        hits.into_iter().map(|(_, n, w)| (n, w)).collect()
    }
}

/// One line per skill for the system prompt; bodies load on demand.
impl Skills {
    pub fn load(workspace: &Path) -> Skills {
        let mut by_name = BTreeMap::new();
        for (name, files) in DEFAULTS {
            let files: BTreeMap<String, String> = files
                .iter()
                .map(|(f, c)| (f.to_string(), c.to_string()))
                .collect();
            by_name.insert(name.to_string(), skill(name, files));
        }
        let mut dirs = Vec::new();
        if let Some(dir) = user_dir() {
            dirs.push(dir);
        }
        dirs.push(workspace.join(".backspace/skills"));
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.join("SKILL.md").is_file() {
                    continue;
                }
                let files: BTreeMap<String, String> = std::fs::read_dir(&path)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|f| f.path().extension().is_some_and(|e| e == "md"))
                    .filter_map(|f| {
                        let content = std::fs::read_to_string(f.path()).ok()?;
                        Some((f.file_name().to_string_lossy().into_owned(), content))
                    })
                    .collect();
                let name = entry.file_name().to_string_lossy().into_owned();
                by_name.insert(name.clone(), skill(&name, files));
            }
        }
        Skills { by_name }
    }

    /// One line per skill for the system prompt; bodies load on demand.
    pub fn index(&self) -> String {
        self.index_where(|_| true)
    }

    /// The same, for the skills `keep` lets through (the policy's allowlist).
    pub fn index_where(&self, keep: impl Fn(&str) -> bool) -> String {
        self.by_name
            .values()
            .filter(|s| keep(&s.name))
            .map(|s| format!("- {}: {}", s.name, s.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn read(&self, name: &str, file: Option<&str>) -> Option<String> {
        let s = self.by_name.get(name)?;
        let file = file.unwrap_or("SKILL.md");
        let body = s.files.get(file)?;
        let others: Vec<&str> = s
            .files
            .keys()
            .map(String::as_str)
            .filter(|f| *f != file)
            .collect();
        Some(if others.is_empty() {
            body.clone()
        } else {
            format!(
                "{body}\n\n(Other files in this skill, readable with `file`: {})",
                others.join(", ")
            )
        })
    }
}

fn skill(name: &str, files: BTreeMap<String, String>) -> Skill {
    let description = files
        .get("SKILL.md")
        .and_then(|md| description(md.strip_prefix("---")?.split("\n---").next()?))
        .unwrap_or_default();
    Skill {
        name: name.to_string(),
        description,
        files,
    }
}

/// The frontmatter's `description`, either on its own line or as a YAML block
/// scalar (`>` folds the indented lines into one, `|` keeps their breaks), the
/// way upstream skills write it.
fn description(frontmatter: &str) -> Option<String> {
    let mut lines = frontmatter.lines();
    let first = lines.find_map(|l| l.strip_prefix("description:"))?.trim();
    if !first.starts_with(['>', '|']) {
        return Some(first.trim_matches('"').to_string());
    }
    let block: Vec<&str> = lines
        .take_while(|l| l.trim().is_empty() || l.starts_with([' ', '\t']))
        .map(str::trim)
        .collect();
    let sep = if first.starts_with('>') { " " } else { "\n" };
    Some(block.join(sep).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_load_with_descriptions() {
        let s = Skills::load(Path::new("/nonexistent"));
        for name in [
            "grilling",
            "to-spec",
            "to-tickets",
            "triage",
            "tdd",
            "diagnosing-bugs",
            "code-review",
            "ponytail",
        ] {
            assert!(
                !s.by_name[name].description.is_empty(),
                "{name} has no description"
            );
        }
        assert!(s
            .read("triage", Some("AGENT-BRIEF.md"))
            .unwrap()
            .contains("Agent Brief"));
        assert!(s.read("ponytail", None).unwrap().contains("lazy senior"));
        // A folded block (`description: >`) is the sentence under it, not `>`.
        assert!(
            s.by_name["ponytail"].description.starts_with("Lazy senior dev mode: the smallest change"),
            "{:?}",
            s.by_name["ponytail"].description
        );
        assert!(s.by_name["ponytail"].description.ends_with("Levels: lite, full (default), ultra."));
        assert!(s.by_name["to-spec"].description.starts_with("Turn the current conversation into a spec"));
        assert!(s.read("nope", None).is_none());
    }

    #[test]
    fn description_reads_plain_folded_and_literal() {
        assert_eq!(description("name: a\ndescription: \"Quoted.\"\n").as_deref(), Some("Quoted."));
        assert_eq!(
            description("description: >\n  One line\n  and the next.\nlicense: MIT").as_deref(),
            Some("One line and the next.")
        );
        assert_eq!(description("description: |\n  Kept\n  apart.\n").as_deref(), Some("Kept\napart."));
        assert_eq!(description("name: a"), None);
    }

    #[test]
    fn found_says_where_a_skill_came_from() {
        let s = Skills::load(Path::new("/nonexistent"));
        let found = s.found(Path::new("/nonexistent"));
        // Every built-in, in name order, with its files and no folder.
        assert_eq!(found.len(), s.by_name.len());
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "listed in name order");
        let pony = found.iter().find(|f| f.name == "ponytail").unwrap();
        assert_eq!(pony.origin, "built-in");
        assert!(pony.dir.is_none());
        assert!(pony.files.contains(&"SKILL.md".to_string()));
    }

    #[test]
    fn a_workspace_folder_overrides_a_default_and_shows_as_ours() {
        let dir = std::env::temp_dir().join(format!("bs-skills-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mine = dir.join(".backspace/skills/my-own");
        std::fs::create_dir_all(&mine).unwrap();
        std::fs::write(
            mine.join("SKILL.md"),
            "---\nname: my-own\ndescription: A skill only this project has.\n---\n\nDo the thing.\n",
        )
        .unwrap();
        // And an override of a default, which must replace it rather than add.
        let over = dir.join(".backspace/skills/tdd");
        std::fs::create_dir_all(&over).unwrap();
        std::fs::write(over.join("SKILL.md"), "---\ndescription: Ours, not upstream.\n---\n\nOurs.\n").unwrap();

        let s = Skills::load(&dir);
        assert!(s.by_name.contains_key("my-own"), "a new folder is a new skill");
        assert_eq!(
            s.by_name["tdd"].description,
            "Ours, not upstream.",
            "the folder wins over the vendored copy"
        );
        let found = s.found(&dir);
        let own = found.iter().find(|f| f.name == "my-own").unwrap();
        assert_eq!(own.origin, "workspace");
        assert!(own.dir.as_ref().unwrap().ends_with("my-own"));
        assert_eq!(found.iter().find(|f| f.name == "tdd").unwrap().origin, "workspace");
        assert_eq!(found.iter().find(|f| f.name == "grilling").unwrap().origin, "built-in");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_ranks_a_name_hit_above_a_body_hit() {
        let s = Skills::load(Path::new("/nonexistent"));
        // "tdd" is a name, and it also appears in other skills' bodies.
        let hits = s.search("tdd");
        assert_eq!(hits.first().map(|(n, _)| n.as_str()), Some("tdd"), "{hits:?}");
        assert_eq!(hits[0].1, "the name");
        // A description-only hit.
        let hits = s.search("bug");
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|(n, _)| n == "diagnosing-bugs"), "{hits:?}");
        // Case-insensitive, and an empty query finds nothing rather than everything.
        assert_eq!(s.search("PONYTAIL").first().map(|(n, _)| n.as_str()), Some("ponytail"));
        assert!(s.search("   ").is_empty());
        assert!(s.search("no-such-thing-anywhere").is_empty());
    }
}
