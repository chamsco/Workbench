//! Chat projects: a folder plus the bots assigned to it. Deliberately not the
//! heavy Workbench project in Code — no tickets, branches or approvals — one
//! project per folder, stored once in `<data>/agent-projects.json`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

use crate::chat::{new_id, now_ms};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub folder: String,
    /// "" is this Mac. Stored and shown; only this Mac is settable today.
    #[serde(default)]
    pub machine: String,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
}

pub struct Projects {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Projects {
    pub fn open(data_dir: PathBuf) -> Self {
        Self {
            path: data_dir.join("agent-projects.json"),
            lock: Mutex::new(()),
        }
    }

    fn load(&self) -> Vec<Project> {
        let Ok(b) = std::fs::read(&self.path) else { return vec![] };
        serde_json::from_slice(&b).unwrap_or_default()
    }

    fn write(&self, projects: &[Project]) -> Result<()> {
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&self.path, serde_json::to_vec_pretty(projects)?)?;
        Ok(())
    }

    /// Newest first, like the rest of the sidebar.
    pub fn list(&self) -> Vec<Project> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = self.load();
        out.sort_by(|a, b| b.updated.cmp(&a.updated).then(b.created.cmp(&a.created)));
        out
    }

    pub fn get(&self, id: &str) -> Option<Project> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.load().into_iter().find(|p| p.id == id)
    }

    /// Create (empty id) or update a project. The name is required and the
    /// folder has to be a folder that exists on this machine.
    pub fn save(&self, mut p: Project) -> Result<Project> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.load();
        p.name = p.name.trim().chars().take(60).collect();
        if p.name.is_empty() {
            bail!("a project needs a name");
        }
        let folder = crate::agents::expand(p.folder.trim());
        if !folder.is_dir() {
            bail!("{} is not a folder", folder.display());
        }
        p.folder = dunce::canonicalize(&folder).unwrap_or(folder).display().to_string();
        p.machine = p.machine.trim().to_string();
        let now = now_ms();
        if p.id.is_empty() {
            p.id = new_id();
            p.created = now;
        } else if let Some(old) = all.iter().find(|x| x.id == p.id) {
            p.created = old.created;
        } else {
            bail!("no such project");
        }
        if all.iter().any(|x| x.id != p.id && same_folder(&x.folder, &p.folder)) {
            bail!("a project already has {}", p.folder);
        }
        p.updated = now;
        match all.iter_mut().find(|x| x.id == p.id) {
            Some(slot) => *slot = p.clone(),
            None => all.push(p.clone()),
        }
        self.write(&all)?;
        Ok(p)
    }

    /// Remove the record only: bots, folders and threads stay.
    pub fn delete(&self, id: &str) -> Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.load();
        let before = all.len();
        all.retain(|p| p.id != id);
        if all.len() == before {
            bail!("no such project");
        }
        self.write(&all)
    }

    pub fn for_agent(&self, agent: &str) -> Vec<Project> {
        self.list().into_iter().filter(|p| p.agents.iter().any(|a| a == agent)).collect()
    }

    /// Assign a bot to a project (or take it off): idempotent.
    pub fn assign(&self, project: &str, agent: &str, on: bool) -> Result<Project> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.load();
        let i = all.iter().position(|p| p.id == project).ok_or_else(|| anyhow!("no such project"))?;
        let was = all[i].agents.iter().any(|a| a == agent);
        if on && !was {
            all[i].agents.push(agent.to_string());
        } else if !on && was {
            all[i].agents.retain(|a| a != agent);
        }
        if on != was {
            all[i].updated = now_ms();
            self.write(&all)?;
        }
        Ok(all[i].clone())
    }
}

/// Two project folder strings are the same folder when they resolve the same.
fn same_folder(a: &str, b: &str) -> bool {
    match (dunce::canonicalize(a), dunce::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => Path::new(a) == Path::new(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("bs-projects-{}", new_id()))
    }

    fn project(name: &str, folder: &Path) -> Project {
        Project {
            id: String::new(),
            name: name.into(),
            folder: folder.display().to_string(),
            machine: String::new(),
            agents: vec![],
            created: 0,
            updated: 0,
        }
    }

    #[test]
    fn save_list_rename_assign_delete() {
        let dir = dir();
        let projects = Projects::open(dir.clone());
        let site = dir.join("site");
        let app = dir.join("app");
        std::fs::create_dir_all(&site).unwrap();
        std::fs::create_dir_all(&app).unwrap();

        // A project needs a name, an existing folder, and only one per folder.
        assert!(projects.save(Project { name: "  ".into(), ..project("x", &site) }).is_err());
        assert!(projects.save(Project { folder: String::new(), ..project("Site", &site) }).is_err());
        assert!(projects.save(project("Gone", Path::new("no/such/folder"))).is_err());
        let saved = projects.save(project("Site", &site)).unwrap();
        assert!(!saved.id.is_empty() && saved.created > 0);
        assert!(projects.save(project("Again", &site)).is_err(), "one project per folder");

        // Rename keeps the id and creation time, and the list is newest first.
        std::thread::sleep(std::time::Duration::from_millis(3));
        let other = projects.save(project("App", &app)).unwrap();
        let renamed = projects.save(Project { name: "Website".into(), ..saved.clone() }).unwrap();
        assert_eq!((renamed.id.as_str(), renamed.created, renamed.name.as_str()), (saved.id.as_str(), saved.created, "Website"));
        assert_eq!(projects.list().iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Website", "App"], "the just-edited one is newest");
        assert_eq!(projects.list().len(), 2);

        // Assign is idempotent both ways; for_agent reads it back.
        let on = projects.assign(&saved.id, "kira", true).unwrap();
        assert_eq!(on.agents, ["kira"]);
        projects.assign(&saved.id, "kira", true).unwrap();
        assert_eq!(projects.get(&saved.id).unwrap().agents, ["kira"]);
        assert_eq!(projects.for_agent("kira").len(), 1);
        assert_eq!(projects.for_agent(&other.id).len(), 0);
        assert!(projects.assign(&saved.id, "kira", false).unwrap().agents.is_empty());
        assert_eq!(projects.for_agent("kira").len(), 0);
        assert!(projects.assign("nope", "kira", true).is_err());

        // It is on disk, in the data dir's own file, and gone after delete.
        assert!(projects.list().iter().any(|p| p.id == saved.id));
        let reopened = Projects::open(dir.clone());
        assert_eq!(reopened.list().len(), 2);
        reopened.delete(&saved.id).unwrap();
        assert_eq!(reopened.list().len(), 1);
        assert!(reopened.get(&saved.id).is_none());
        assert!(reopened.delete(&saved.id).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
