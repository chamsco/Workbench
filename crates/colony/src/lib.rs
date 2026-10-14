//! Colony: a durable object per agent.
//!
//! Workbench agents run on the host. Colony gives each one a computer of its
//! own — a **workspace**: a WSL2 distro or a Hyper-V VM created on demand,
//! addressable by name in chat, and thrown away when the task is done. The
//! registry (`colony.json`) is the durable object: it survives restarts, and
//! the agent's tools (`colony_create` … `colony_destroy`) act on it.
//!
//! This crate is the Workbench-side half of the `capsule/` project. It drives
//! `visor-core`'s JSON CLI (`capsule/core`) through [`VisorCli`], which
//! implements [`Backend`]. [`FakeBackend`] is the same trait in memory, so the
//! registry's whole lifecycle is tested without a VM, WSL, Hyper-V or a
//! network. See `docs/COLONY.md` for what is implemented and what is not.
//!
//! ```no_run
//! use backspace_colony::{Colony, CreateSpec};
//!
//! fn main() -> anyhow::Result<()> {
//!     let mut colony = Colony::open(std::env::temp_dir().join("colony"))?;
//!     let ws = colony.create(&CreateSpec::new("demo"))?;
//!     let argv = vec!["bash".to_string(), "-lc".to_string(), "uname -a".to_string()];
//!     println!("{}", colony.exec(&ws.name, &argv)?.stdout);
//!     colony.delete(&ws.name)?;
//!     Ok(())
//! }
//! ```

mod backend;
mod colony;
mod fake;
mod model;
mod prompt;
mod tools;

pub use backend::{create_args, parse_error, workspace_from_cli, Backend, VisorCli, EXE};
pub use colony::{Colony, STATE_FILE};
pub use fake::FakeBackend;
pub use model::{Capabilities, CreateSpec, ExecOut, ProviderKind, State, Workspace};
pub use prompt::chat_prompt;
pub use tools::to_tools;
