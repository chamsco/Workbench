//! The five tools a chat agent gets, as JSON.
//!
//! Shape: one object per tool, `{"name", "description", "schema"}`, mirroring
//! `crates/core/src/provider.rs::ToolDef` (`crates/core/src/tools.rs` builds
//! the same three fields). `schema` is a bare JSON Schema for the arguments —
//! the caller wraps it for whichever provider it is sending to. This crate
//! defines the tools; the harness dispatches them.

use serde_json::{json, Value};

/// `colony_create`, `colony_exec`, `colony_list`, `colony_status`,
/// `colony_destroy`, in that order.
pub fn to_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "colony_create",
            "description": "Create a disposable workspace (a WSL2 distro, or a Hyper-V VM with provider \"hyperv\" and elevation) and register it in this colony. WSL is quick; Hyper-V boots a real guest and takes minutes.",
            "schema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Unique workspace name in this colony, e.g. \"api-tests\"."},
                    "provider": {"type": "string", "enum": ["wsl", "hyperv"], "description": "wsl (fast, no elevation) or hyperv (full VM, needs an elevated host). Defaults to wsl."},
                    "os": {"type": "string", "description": "Guest image, e.g. \"ubuntu-24.04\", or \"clone:<distro>\" to clone an existing WSL distro."},
                    "cpus": {"type": "integer", "minimum": 1, "description": "Default 2."},
                    "memory_mib": {"type": "integer", "minimum": 512, "description": "RAM in MiB. Default 4096."},
                    "disk_gib": {"type": "integer", "minimum": 1, "description": "Disk in GiB. Default 20."},
                    "ssh_key_path": {"type": "string", "description": "Host path to an SSH public key to inject into a Hyper-V Linux guest. A path, not the key itself."},
                    "packages": {"type": "array", "items": {"type": "string"}, "description": "apt packages to install on the guest's first boot."},
                    "task": {"type": "string", "description": "One line on what this workspace is for; kept in the registry, never sent to the guest."}
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "colony_exec",
            "description": "Run a command inside a workspace and return its exit code, stdout and stderr. WSL workspaces only (visor-core refuses others; use SSH for Hyper-V). `command` is argv, so use [\"bash\", \"-lc\", \"...\"] for shell syntax.",
            "schema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "The workspace to run in."},
                    "command": {
                        "type": "array",
                        "items": {"type": "string"},
                        "minItems": 1,
                        "description": "argv, run as-is: [\"bash\", \"-lc\", \"make test\"]."
                    }
                },
                "required": ["name", "command"]
            }
        }),
        json!({
            "name": "colony_list",
            "description": "List every workspace in this colony with its provider, OS and state. Also adopts workspaces the backend reports but the registry has not seen.",
            "schema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "colony_status",
            "description": "Report one workspace: state, provider, OS and IP. Cheap; run it before assuming a workspace is still running.",
            "schema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "The workspace to report."}
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "colony_destroy",
            "description": "Delete a workspace and everything inside it, and forget it. Irreversible.",
            "schema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "The workspace to delete."}
                },
                "required": ["name"]
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        to_tools()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn five_tools_with_the_right_names() {
        assert_eq!(
            names(),
            vec![
                "colony_create",
                "colony_exec",
                "colony_list",
                "colony_status",
                "colony_destroy"
            ]
        );
    }

    #[test]
    fn every_tool_carries_a_description_and_an_object_schema() {
        for tool in to_tools() {
            let name = tool["name"].as_str().unwrap();
            assert!(
                tool["description"].as_str().is_some_and(|d| d.len() > 20),
                "{name} needs a real description"
            );
            assert_eq!(tool["schema"]["type"], "object", "{name}");
        }
    }

    #[test]
    fn required_arguments_are_the_ones_the_registry_needs() {
        let tools = to_tools();
        let required = |name: &str| {
            tools.iter().find(|t| t["name"] == name).unwrap()["schema"]["required"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        assert_eq!(required("colony_create"), vec!["name"]);
        assert_eq!(required("colony_exec"), vec!["name", "command"]);
        assert_eq!(required("colony_status"), vec!["name"]);
        assert_eq!(required("colony_destroy"), vec!["name"]);
        assert!(required("colony_list").is_empty());
        // The exec schema has to accept argv, not a shell string.
        assert_eq!(tools[1]["schema"]["properties"]["command"]["type"], "array");
    }
}
