//! Undoes a change to the policy files made while an agent was acting.
//!
//! The agents' shell is not sandboxed (that is M6), so the file tools'
//! refusal to touch `backspace.toml` does not stop `sed -i`. What this does
//! instead: around every tool batch and every CLI turn it notes what the two
//! files that carry the policy (the workspace's `backspace.toml` and the
//! adopted-policy lock outside it) held when no agent was acting, and puts
//! that back if it differs afterwards, saying so in the agent's log.
//!
//! Limits, stated in docs/cauldron/M1-HARDENING-NOTES.md: it answers for what
//! happens during an action. A process an agent leaves running can write
//! later, between actions, and that is taken as a person's edit. A person's
//! edit made *during* an agent's action is undone with the agent's. Only a
//! sandbox or a separate OS user closes those.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct Watched {
    path: PathBuf,
    label: &'static str,
    /// What the file held when last no agent was acting; None: it did not exist.
    good: Option<Vec<u8>>,
}

struct State {
    inflight: usize,
    files: Vec<Watched>,
}

pub struct PolicyGuard {
    state: Mutex<State>,
}

fn read(path: &PathBuf) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

impl PolicyGuard {
    /// Watch `(path, what to call it)` files, as they are now.
    pub fn new(files: Vec<(PathBuf, &'static str)>) -> Arc<PolicyGuard> {
        let files = files.into_iter().map(|(path, label)| Watched { good: read(&path), path, label }).collect();
        Arc::new(PolicyGuard { state: Mutex::new(State { inflight: 0, files }) })
    }

    /// An agent is about to act. When none was acting, whatever the files
    /// hold is what a person left them as.
    pub fn begin(self: &Arc<Self>) -> Action {
        let mut s = self.state.lock().unwrap();
        if s.inflight == 0 {
            for f in &mut s.files {
                f.good = read(&f.path);
            }
        }
        s.inflight += 1;
        Action { guard: self.clone(), done: false }
    }

    fn finish(&self) -> Vec<String> {
        let mut s = self.state.lock().unwrap();
        s.inflight = s.inflight.saturating_sub(1);
        let mut notes = vec![];
        for f in &mut s.files {
            if read(&f.path) == f.good {
                continue;
            }
            let ok = match &f.good {
                Some(bytes) => {
                    if let Some(dir) = f.path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    std::fs::write(&f.path, bytes).is_ok()
                }
                None => std::fs::remove_file(&f.path).is_ok(),
            };
            notes.push(if ok {
                format!("restored {}: it was changed while an agent was working, and the policy is not an agent's to change", f.label)
            } else {
                format!("{} was changed while an agent was working and could not be put back: check it before the next session", f.label)
            });
        }
        notes
    }
}

/// An agent's action in progress. `end` puts the policy files back if the
/// action changed them; dropping it does the same, silently.
pub struct Action {
    guard: Arc<PolicyGuard>,
    done: bool,
}

impl Action {
    pub fn end(mut self) -> Vec<String> {
        self.done = true;
        self.guard.finish()
    }
}

impl Drop for Action {
    fn drop(&mut self) {
        if !self.done {
            self.guard.finish();
        }
    }
}
