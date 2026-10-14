//! The system prompt for an agent that owns a workspace.

use crate::model::Workspace;

/// The prompt to prepend when an agent is talking to the user *as* the agent
/// whose computer is this workspace.
///
/// It is deliberately plain: the agent's real power is the `colony_exec` tool
/// (see [`crate::to_tools`]); this text just tells it what it is holding, that
/// the workspace is disposable, and that the workspace is addressable by name
/// so a human can say "run it in `dev`" to any agent in the chat.
pub fn chat_prompt(ws: &Workspace) -> String {
    let task = match ws.task.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(t) => format!("\nIt was made for: {t}"),
        None => String::new(),
    };
    format!(
        "You have a computer of your own: a disposable workspace called `{name}` \
(provider {provider}, OS {os}, currently {state}).{task}\n\
\n\
- Run commands in it with the `colony_exec` tool: \
{{\"name\": \"{name}\", \"command\": [\"bash\", \"-lc\", \"uname -a\"]}}. The argv runs \
inside the workspace, not on the host.\n\
- `colony_list` shows every workspace in this colony, `colony_status` reports one, \
`colony_create` makes another, and `colony_destroy` throws one away.\n\
- The workspace is reachable by name: when the user says \"run it in {name}\", that \
means here. Anyone in the chat can address it that way.\n\
- It is disposable. Keep nothing there you cannot rebuild from the repository or a \
script; treat scratch files as scratch. If a command breaks it beyond repair, destroy \
it and create a new one instead of nursing it.\n\
- The host is not your computer. Only the workspace is.",
        name = ws.name,
        provider = ws.provider,
        os = ws.os,
        state = ws.state.as_str(),
        task = task,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{State, Workspace};

    fn ws() -> Workspace {
        Workspace {
            name: "dev".into(),
            provider: "wsl".into(),
            os: "ubuntu-24.04".into(),
            state: State::Running,
            ip: None,
            created: 1,
            task: Some("fix the parser".into()),
        }
    }

    #[test]
    fn chat_prompt_names_the_workspace() {
        let p = chat_prompt(&ws());
        assert!(p.contains("`dev`"), "{p}");
        assert!(p.contains("colony_exec"), "{p}");
        assert!(p.contains("colony_destroy"), "{p}");
        assert!(p.contains("disposable"), "{p}");
        assert!(p.contains("fix the parser"), "the task is mentioned");
        assert!(p.contains("running"), "the state is mentioned");
    }

    #[test]
    fn chat_prompt_copes_without_a_task() {
        let mut w = ws();
        w.task = None;
        w.state = State::Stopped;
        let p = chat_prompt(&w);
        assert!(p.contains("`dev`"));
        assert!(p.contains("stopped"));
        assert!(!p.contains("It was made for"), "{p}");
    }
}
