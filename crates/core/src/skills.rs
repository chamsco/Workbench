//! Skills: markdown playbooks an agent loads on demand with the `skill` tool.
//! Defaults are vendored from github.com/mattpocock/skills (MIT, see
//! skills/LICENSE-mattpocock-skills). A folder with the same name under
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

const DEFAULTS: &[(&str, Files)] = vendored! {
    "grilling" => ["SKILL.md"],
    "to-tickets" => ["SKILL.md"],
    "triage" => ["SKILL.md", "AGENT-BRIEF.md", "OUT-OF-SCOPE.md"],
    "tdd" => ["SKILL.md", "tests.md", "mocking.md"],
    "diagnosing-bugs" => ["SKILL.md"],
    "code-review" => ["SKILL.md"],
};

/// How the vendored skills' vocabulary maps onto Backspace. They were written
/// for other harnesses; translating here keeps the files verbatim and easy to
/// update from upstream.
pub const ADAPTER: &str = "Skills were written for a general coding agent. In Backspace: \"the Skill tool\" is the `skill` tool; \"the issue tracker\" and \"tickets\" are Backspace tickets (create_tickets, file_ticket, triage_ticket); \"dispatch a sub-agent\" means create_tickets + work_tickets if you have them, otherwise do it yourself; triage labels are ticket states (needs_triage, needs_info, ready_for_agent, ready_for_human, wontfix). Ignore setup steps such as /setup-matt-pocock-skills and anything about posting comments to GitHub.";

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
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(Path::new(&home).join(".config/backspace/skills"));
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
        self.by_name
            .values()
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
        .and_then(|md| {
            let fm = md.strip_prefix("---")?.split("\n---").next()?;
            fm.lines()
                .find_map(|l| l.strip_prefix("description:"))
                .map(|d| d.trim().trim_matches('"').to_string())
        })
        .unwrap_or_default();
    Skill {
        name: name.to_string(),
        description,
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_load_with_descriptions() {
        let s = Skills::load(Path::new("/nonexistent"));
        for name in [
            "grilling",
            "to-tickets",
            "triage",
            "tdd",
            "diagnosing-bugs",
            "code-review",
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
        assert!(s.read("nope", None).is_none());
    }
}
