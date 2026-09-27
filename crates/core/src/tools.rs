//! Pi's four tools: read, write, edit, bash. Everything else an agent needs it
//! can do through bash. Paths are resolved inside the project workspace.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::provider::ToolDef;

const MAX_OUTPUT: usize = 30_000;

pub fn file_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "read",
            description: "Read a text file in the workspace. Optional 1-based line offset and line limit.",
            schema: json!({"type": "object", "properties": {
                "path": {"type": "string"},
                "offset": {"type": "integer"},
                "limit": {"type": "integer"}
            }, "required": ["path"]}),
        },
        ToolDef {
            name: "write",
            description: "Create or overwrite a file in the workspace with the given content. Creates parent directories.",
            schema: json!({"type": "object", "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"}
            }, "required": ["path", "content"]}),
        },
        ToolDef {
            name: "edit",
            description: "Replace one exact, unique occurrence of old_text with new_text in a workspace file.",
            schema: json!({"type": "object", "properties": {
                "path": {"type": "string"},
                "old_text": {"type": "string"},
                "new_text": {"type": "string"}
            }, "required": ["path", "old_text", "new_text"]}),
        },
        ToolDef {
            name: "bash",
            description: "Run a bash command with the workspace as the working directory. Returns combined stdout/stderr and exit code.",
            schema: json!({"type": "object", "properties": {
                "command": {"type": "string"}
            }, "required": ["command"]}),
        },
    ]
}

#[derive(Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub bash_timeout: Duration,
}

impl Workspace {
    /// Lexically resolve `p` under the root and refuse anything that escapes.
    /// Bash can still leave the workspace; this guards the file tools only.
    pub fn resolve(&self, p: &str) -> Result<PathBuf> {
        let rel = Path::new(p);
        let rel = rel.strip_prefix(&self.root).unwrap_or(rel);
        if rel.is_absolute() {
            bail!("absolute path outside workspace: {p}");
        }
        let mut out = self.root.clone();
        let mut depth = 0usize;
        for c in rel.components() {
            match c {
                Component::Normal(s) => {
                    out.push(s);
                    depth += 1;
                }
                Component::ParentDir => {
                    if depth == 0 {
                        bail!("path escapes workspace: {p}");
                    }
                    out.pop();
                    depth -= 1;
                }
                Component::CurDir => {}
                _ => bail!("unsupported path: {p}"),
            }
        }
        Ok(out)
    }

    pub async fn run(&self, name: &str, input: &Value) -> Result<String> {
        let s = |k: &str| {
            input[k]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("missing string argument `{k}`"))
        };
        match name {
            "read" => {
                let path = self.resolve(&s("path")?)?;
                let text = tokio::fs::read_to_string(&path).await?;
                let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
                let limit = input["limit"]
                    .as_u64()
                    .map(|l| l as usize)
                    .unwrap_or(usize::MAX);
                let body: Vec<&str> = text.lines().skip(offset - 1).take(limit).collect();
                Ok(cap(body.join("\n")))
            }
            "write" => {
                let path = self.resolve(&s("path")?)?;
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let content = s("content")?;
                tokio::fs::write(&path, &content).await?;
                Ok(format!(
                    "wrote {} bytes to {}",
                    content.len(),
                    self.rel(&path)
                ))
            }
            "edit" => {
                let path = self.resolve(&s("path")?)?;
                let (old, new) = (s("old_text")?, s("new_text")?);
                let text = tokio::fs::read_to_string(&path).await?;
                match text.matches(&old).count() {
                    0 => bail!("old_text not found in {}", self.rel(&path)),
                    1 => {}
                    n => bail!(
                        "old_text matches {n} times in {}; include more context",
                        self.rel(&path)
                    ),
                }
                tokio::fs::write(&path, text.replacen(&old, &new, 1)).await?;
                Ok(format!("edited {}", self.rel(&path)))
            }
            "bash" => {
                let (code, text) = self.sh(&s("command")?).await?;
                Ok(format!("{text}\n[exit {code}]"))
            }
            other => bail!("unknown tool {other}"),
        }
    }

    /// Run a shell command in the workspace: (exit code, capped output).
    pub async fn sh(&self, cmd: &str) -> Result<(i32, String)> {
        let child = tokio::process::Command::new("bash")
            .arg("-lc")
            .arg(cmd)
            .current_dir(&self.root)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output();
        let out = tokio::time::timeout(self.bash_timeout, child)
            .await
            .map_err(|_| anyhow!("timed out after {}s", self.bash_timeout.as_secs()))??;
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        Ok((
            out.status.code().unwrap_or(-1),
            cap(text.trim_end().to_string()),
        ))
    }

    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.root)
            .unwrap_or(p)
            .display()
            .to_string()
    }
}

fn cap(mut s: String) -> String {
    if s.len() > MAX_OUTPUT {
        let keep = crate::router::truncate(&s, MAX_OUTPUT).len();
        s.truncate(keep);
        s.push_str("\n[output truncated]");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> Workspace {
        Workspace {
            root: PathBuf::from("/tmp/ws"),
            bash_timeout: Duration::from_secs(5),
        }
    }

    #[test]
    fn resolve_blocks_escape() {
        let w = ws();
        assert_eq!(
            w.resolve("a/../b.txt").unwrap(),
            PathBuf::from("/tmp/ws/b.txt")
        );
        assert_eq!(w.resolve("/tmp/ws/c").unwrap(), PathBuf::from("/tmp/ws/c"));
        assert!(w.resolve("../etc/passwd").is_err());
        assert!(w.resolve("/etc/passwd").is_err());
    }

    #[tokio::test]
    async fn write_edit_read_bash() {
        let dir = std::env::temp_dir().join(format!("bs-tools-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let w = Workspace {
            root: dir.clone(),
            bash_timeout: Duration::from_secs(5),
        };
        w.run(
            "write",
            &json!({"path": "x/a.txt", "content": "hello world"}),
        )
        .await
        .unwrap();
        w.run(
            "edit",
            &json!({"path": "x/a.txt", "old_text": "world", "new_text": "backspace"}),
        )
        .await
        .unwrap();
        assert_eq!(
            w.run("read", &json!({"path": "x/a.txt"})).await.unwrap(),
            "hello backspace"
        );
        let out = w
            .run("bash", &json!({"command": "cat x/a.txt"}))
            .await
            .unwrap();
        assert!(out.starts_with("hello backspace") && out.ends_with("[exit 0]"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
