//! Folders: a field for filing streams, separate from the directory a stream
//! runs in. The rules these tests hold:
//!
//! - a folder has a trimmed, non-empty name, unique ignoring case, at most 60
//!   characters, and a colour taken from the palette in creation order;
//! - filing a stream changes its `folder` and nothing else: not `project`
//!   (where the CLI runs), not `updated`, not one other byte of its file;
//! - deleting a folder files its streams nowhere and never deletes a stream;
//! - a proposal is data: accepting it files the stream (creating the folder
//!   first when it names a new one), rejecting it changes nothing;
//! - the registry and the pending proposals survive a restart.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use backspace_core::chat::{Chats, Life, Route, RouteKind, Scope, Thread};
use backspace_core::folders::{Folder, FolderCount, FolderProposal, Folders, ProposalTarget};
use serde_json::{json, Value};

const PALETTE: [&str; 6] = ["#8b5cf6", "#3fb27f", "#e0a32a", "#5b8cf0", "#d9669a", "#38c0e8"];

/// Removes the data dir when the test ends, pass or fail.
struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A data dir with the chats and the folders registry opened on it, the way
/// the Fleet opens them.
struct Env {
    dir: PathBuf,
    folders: Folders,
    chats: Arc<Chats>,
    _rt: tokio::runtime::Runtime,
    _guard: Guard,
}

fn open(dir: &Path) -> (Arc<Chats>, Folders, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let (tx, _rx) = async_channel::bounded(1);
    let chats = Chats::open(dir.to_path_buf(), rt.handle().clone(), tx);
    let folders = Folders::open(dir.to_path_buf(), chats.clone());
    (chats, folders, rt)
}

/// One stream as an older build wrote it: no `folder` key.
fn write_thread(dir: &Path, id: &str, updated: u64, project: Option<&str>) {
    let mut v = json!({
        "id": id,
        "title": format!("stream {id}"),
        "created": 1000,
        "updated": updated,
        "pinned": false,
        "route": { "kind": "cli", "provider": "claude", "model": null },
        "cli_session": "session-1",
        "goal": "ship it",
        "messages": [
            { "id": "m1", "role": "user", "text": "hello", "at": 1000, "status": "done" },
            { "id": "m2", "role": "assistant", "text": "hi there", "at": 1001, "status": "done" }
        ]
    });
    if let Some(p) = project {
        v["project"] = json!(p);
    }
    std::fs::create_dir_all(dir.join("threads")).unwrap();
    std::fs::write(dir.join("threads").join(format!("{id}.json")), serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

impl Env {
    /// `threads`: `(id, updated, project)` written as old-format files first.
    fn new(name: &str, threads: &[(&str, u64, Option<&str>)]) -> Env {
        let dir = std::env::temp_dir().join(format!("bs-folders-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (id, updated, project) in threads {
            write_thread(&dir, id, *updated, *project);
        }
        let (chats, folders, rt) = open(&dir);
        Env { dir: dir.clone(), folders, chats, _rt: rt, _guard: Guard(dir) }
    }

    /// Close everything and open it again from disk, as a restart does.
    fn reopen(self) -> Env {
        let Env { dir, folders, chats, _rt, _guard } = self;
        drop(folders);
        drop(chats);
        drop(_rt);
        let (chats, folders, rt) = open(&dir);
        Env { dir, folders, chats, _rt: rt, _guard }
    }

    /// The thread's file as JSON, straight off the disk.
    fn file(&self, id: &str) -> Value {
        serde_json::from_slice(&self.bytes(id)).unwrap()
    }

    fn bytes(&self, id: &str) -> Vec<u8> {
        std::fs::read(self.dir.join("threads").join(format!("{id}.json"))).unwrap()
    }

    fn thread(&self, id: &str) -> Thread {
        self.chats.peek(id).unwrap()
    }

    fn names(&self) -> Vec<String> {
        self.folders.list().into_iter().map(|f| f.name).collect()
    }
}

fn ids(v: Vec<backspace_core::chat::ThreadInfo>) -> Vec<String> {
    v.into_iter().map(|t| t.id).collect()
}

fn existing(thread: &str, folder: &str) -> FolderProposal {
    FolderProposal { thread_id: thread.into(), target: ProposalTarget::Existing(folder.into()), reason: "same repo".into(), created: 5 }
}

fn new_folder(thread: &str, name: &str) -> FolderProposal {
    FolderProposal { thread_id: thread.into(), target: ProposalTarget::New(name.into()), reason: "all about trips".into(), created: 6 }
}

// ------------------------------------------------------------ create, rename, delete

#[test]
fn create_trims_and_refuses_empty_and_case_duplicates() {
    let e = Env::new("create", &[]);
    let work = e.folders.create("  Work  ").unwrap();
    assert_eq!(work.name, "Work");
    assert!(!work.id.is_empty());
    assert!(work.created > 0 && work.updated >= work.created);

    for empty in ["", "   ", "\t\n "] {
        assert!(e.folders.create(empty).is_err(), "{empty:?} should be refused");
    }
    for dup in ["Work", "work", "WORK", "  wOrK  "] {
        assert!(e.folders.create(dup).is_err(), "{dup:?} differs from Work only by case or space");
    }
    assert_eq!(e.names(), ["Work"], "refusals leave the registry alone");

    let home = e.folders.create("Home").unwrap();
    assert_ne!(home.id, work.id);
    assert_eq!(e.names(), ["Work", "Home"]);
}

#[test]
fn a_name_is_capped_at_sixty_characters() {
    let e = Env::new("cap", &[]);
    let sixty = "x".repeat(60);
    assert_eq!(e.folders.create(&sixty).unwrap().name, sixty);
    // Characters, not bytes: 70 two-byte letters keep their first 60.
    let accents = "é".repeat(70);
    assert_eq!(e.folders.create(&accents).unwrap().name, "é".repeat(60));
    // Cut to the cap first, then compared: the same long name twice is a duplicate.
    assert!(e.folders.create(&format!("{sixty}and then some")).is_err());
}

#[test]
fn rename_keeps_identity_and_obeys_the_same_rules() {
    let e = Env::new("rename", &[]);
    let work = e.folders.create("Work").unwrap();
    let home = e.folders.create("Home").unwrap();

    let before = e.folders.list();
    assert!(e.folders.rename(&work.id, "home").is_err(), "taken by Home, differing only by case");
    assert!(e.folders.rename(&work.id, "  ").is_err());
    assert!(e.folders.rename("no-such-folder", "Anything").is_err());
    assert_eq!(e.folders.list(), before, "a refused rename changes nothing");

    // Case-only change of its own name is allowed; the rest of the record stays.
    let renamed = e.folders.rename(&work.id, "  WORK  ").unwrap();
    assert_eq!(renamed.name, "WORK");
    assert_eq!((renamed.id.as_str(), renamed.color.as_str(), renamed.created), (work.id.as_str(), work.color.as_str(), work.created));
    assert!(renamed.updated > work.updated, "a rename is an update");
    assert_eq!(e.folders.list()[1], home, "the other folder is untouched");
    assert_eq!(e.names(), ["WORK", "Home"], "and the order is still creation order");
}

#[test]
fn list_is_creation_order_and_stable_across_a_reload() {
    let e = Env::new("order", &[]);
    for n in ["Zeta", "Alpha", "Mid", "beta"] {
        e.folders.create(n).unwrap();
    }
    let first = e.folders.list();
    assert_eq!(first.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Zeta", "Alpha", "Mid", "beta"]);
    let alpha = first[1].id.clone();
    e.folders.rename(&alpha, "Omega").unwrap();
    assert_eq!(e.names(), ["Zeta", "Omega", "Mid", "beta"], "renaming does not move a folder");
    let e = e.reopen();
    assert_eq!(e.names(), ["Zeta", "Omega", "Mid", "beta"]);
}

#[test]
fn colours_follow_the_palette_in_creation_order_and_wrap() {
    let e = Env::new("colours", &[]);
    let made: Vec<Folder> = (0..8).map(|i| e.folders.create(&format!("Folder {i}")).unwrap()).collect();
    let got: Vec<&str> = made.iter().map(|f| f.color.as_str()).collect();
    let want: Vec<&str> = (0..8).map(|i| PALETTE[i % 6]).collect();
    assert_eq!(got, want);
    assert_eq!(made[6].color, made[0].color, "the seventh wraps to the first");
    // And they are what is stored, not recomputed.
    let e = e.reopen();
    assert_eq!(e.folders.list().iter().map(|f| f.color.as_str()).collect::<Vec<_>>(), want);
    // A folder made after a restart carries on from where the sequence was.
    assert_eq!(e.folders.create("Folder 8").unwrap().color, PALETTE[8 % 6]);
}

#[test]
fn deleting_a_folder_unfiles_its_streams_and_keeps_every_stream() {
    let e = Env::new("delete", &[("t1", 3000, Some("/work/site")), ("t2", 2000, Some("/work/app")), ("t3", 1000, None)]);
    let doomed = e.folders.create("Doomed").unwrap();
    let kept = e.folders.create("Kept").unwrap();
    e.folders.set_thread_folder("t1", Some(&doomed.id)).unwrap();
    e.folders.set_thread_folder("t2", Some(&doomed.id)).unwrap();
    e.folders.set_thread_folder("t3", Some(&kept.id)).unwrap();
    let before = e.chats.list().len();

    assert!(e.folders.delete("no-such-folder").is_err());
    e.folders.delete(&doomed.id).unwrap();

    assert_eq!(e.names(), ["Kept"]);
    assert_eq!(e.chats.list().len(), before, "no stream was deleted");
    for id in ["t1", "t2"] {
        assert_eq!(e.thread(id).folder, None, "{id} is unfiled now");
        assert!(e.file(id).get("folder").is_none(), "and its file says so");
        assert_eq!(e.thread(id).messages.len(), 2, "with its conversation");
    }
    assert_eq!(e.thread("t1").project.as_deref(), Some("/work/site"), "where it runs did not move");
    assert_eq!(e.thread("t2").project.as_deref(), Some("/work/app"));
    assert_eq!(e.thread("t3").folder.as_deref(), Some(kept.id.as_str()), "another folder's stream is untouched");
    assert_eq!(ids(e.folders.threads_in(&doomed.id)), Vec::<String>::new(), "nothing is in a folder that is gone");
    assert_eq!(ids(e.folders.unfiled()), ["t1", "t2"]);

    // The same on disk after a restart; and the name is free again.
    let e = e.reopen();
    assert_eq!(e.thread("t1").folder, None);
    assert_eq!(e.thread("t3").folder.as_deref(), Some(kept.id.as_str()));
    assert!(e.folders.create("doomed").is_ok());
}

// ------------------------------------------------------------ filing a stream

#[test]
fn filing_a_stream_changes_only_its_folder() {
    let e = Env::new("file-only", &[("t1", 4242, Some("/work/site"))]);
    let f = e.folders.create("Site").unwrap();
    e.chats.pin("t1", true).unwrap(); // a non-default value among the other fields
    let before_file = e.file("t1");
    let before = e.thread("t1");
    assert!(before_file.get("folder").is_none());

    e.folders.set_thread_folder("t1", Some(&f.id)).unwrap();

    let after = e.thread("t1");
    assert_eq!(after.folder.as_deref(), Some(f.id.as_str()));
    assert_eq!(after.project, before.project, "where it runs");
    assert_eq!(after.project.as_deref(), Some("/work/site"));
    assert_eq!(after.updated, before.updated, "filing is not activity, so it does not reorder the list");
    assert_eq!(after.title, before.title);
    assert_eq!(after.pinned, before.pinned);
    assert_eq!(after.messages.len(), before.messages.len());

    // On disk: the same file with one more key.
    let mut after_file = e.file("t1");
    assert_eq!(after_file["folder"], json!(f.id));
    after_file.as_object_mut().unwrap().remove("folder");
    assert_eq!(after_file, before_file);

    // Moving it to another folder, then out, changes the same one key again.
    let g = e.folders.create("Other").unwrap();
    e.folders.set_thread_folder("t1", Some(&g.id)).unwrap();
    assert_eq!(e.thread("t1").folder.as_deref(), Some(g.id.as_str()));
    e.folders.set_thread_folder("t1", None).unwrap();
    assert_eq!(e.thread("t1").folder, None);
    assert_eq!(e.file("t1"), before_file, "unfiled again: the file it was");
    assert_eq!(e.thread("t1").project.as_deref(), Some("/work/site"));
}

#[test]
fn filing_into_an_unknown_folder_errors_and_changes_nothing() {
    let e = Env::new("unknown", &[("t1", 100, Some("/work/site")), ("t2", 200, None)]);
    let a = e.folders.create("A").unwrap();
    e.folders.set_thread_folder("t1", Some(&a.id)).unwrap();
    let (t1, t2) = (e.bytes("t1"), e.bytes("t2"));
    let registry = e.folders.list();

    assert!(e.folders.set_thread_folder("t2", Some("no-such-folder")).is_err());
    assert!(e.folders.set_thread_folder("t1", Some("no-such-folder")).is_err(), "not even to move it");
    assert!(e.folders.set_thread_folder("no-such-thread", Some(&a.id)).is_err());
    assert!(e.folders.set_thread_folder("no-such-thread", None).is_err());

    assert_eq!(e.thread("t2").folder, None);
    assert_eq!(e.thread("t1").folder.as_deref(), Some(a.id.as_str()), "a failed move leaves it where it was");
    assert_eq!((e.bytes("t1"), e.bytes("t2")), (t1, t2), "no thread file was written");
    assert_eq!(e.folders.list(), registry);
}

#[test]
fn a_ghost_chat_cannot_be_filed() {
    let e = Env::new("ghost", &[]);
    let f = e.folders.create("A").unwrap();
    let route = Route { kind: RouteKind::Local, provider: "ollama".into(), model: Some("m".into()) };
    let ghost = e.chats.create_in(route, Scope { life: Life::Ghost, ..Default::default() });
    assert!(e.folders.set_thread_folder(&ghost.id, Some(&f.id)).is_err(), "a ghost is never saved, so there is nothing to file");
    assert_eq!(e.folders.counts()[0].streams, 0);
}

#[test]
fn a_thread_file_from_before_folders_loads_unfiled_and_round_trips() {
    let e = Env::new("old-json", &[("old", 700, Some("/work/site"))]);
    assert!(e.file("old").get("folder").is_none(), "the fixture is an old-format file");
    let t = e.thread("old");
    assert_eq!(t.folder, None);
    assert_eq!(t.project.as_deref(), Some("/work/site"));

    // The type itself: no key in, no key out.
    let original = e.file("old");
    let parsed: Thread = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(parsed.folder, None);
    let back = serde_json::to_value(&parsed).unwrap();
    assert!(back.get("folder").is_none(), "an unfiled stream writes no folder key");
    assert_eq!(back["project"], original["project"]);
    assert_eq!(back["goal"], original["goal"]);
    assert_eq!(back["cli_session"], original["cli_session"]);
    let texts = |v: &Value| v["messages"].as_array().unwrap().iter().map(|m| m["text"].clone()).collect::<Vec<_>>();
    assert_eq!(texts(&back), texts(&original), "the conversation comes back as it went in");
    let again: Thread = serde_json::from_value(back.clone()).unwrap();
    assert_eq!(serde_json::to_value(&again).unwrap(), back, "and writing it again changes nothing");

    // Filed it carries the key; and a restart reads it back.
    let f = e.folders.create("Site").unwrap();
    e.folders.set_thread_folder("old", Some(&f.id)).unwrap();
    let reparsed: Thread = serde_json::from_value(e.file("old")).unwrap();
    assert_eq!(reparsed.folder.as_deref(), Some(f.id.as_str()));
    let e = e.reopen();
    assert_eq!(e.thread("old").folder.as_deref(), Some(f.id.as_str()));
    assert_eq!(e.thread("old").project.as_deref(), Some("/work/site"));
}

#[test]
fn a_branch_stays_in_its_sources_folder() {
    let e = Env::new("branch", &[("t1", 100, Some("/work/site"))]);
    let f = e.folders.create("Site").unwrap();
    e.folders.set_thread_folder("t1", Some(&f.id)).unwrap();
    let b = e.chats.branch("t1", "m1").unwrap();
    assert_eq!(b.folder.as_deref(), Some(f.id.as_str()));
    assert_eq!(b.project.as_deref(), Some("/work/site"));
    assert_eq!(e.folders.counts()[0].streams, 2);
}

// ------------------------------------------------------------ reading them back

#[test]
fn threads_in_unfiled_and_counts_agree() {
    let e = Env::new(
        "counts",
        &[("t1", 5000, None), ("t2", 7000, None), ("t3", 3000, None), ("t4", 9000, None), ("t5", 1000, None)],
    );
    let a = e.folders.create("A").unwrap();
    let b = e.folders.create("B").unwrap();
    let c = e.folders.create("C").unwrap();

    // Nothing filed yet: every folder is empty, every stream unfiled.
    assert_eq!(
        e.folders.counts(),
        vec![
            FolderCount { folder_id: a.id.clone(), streams: 0, last_activity: 0 },
            FolderCount { folder_id: b.id.clone(), streams: 0, last_activity: 0 },
            FolderCount { folder_id: c.id.clone(), streams: 0, last_activity: 0 },
        ]
    );
    assert_eq!(ids(e.folders.unfiled()), ["t4", "t2", "t1", "t3", "t5"], "newest first, like the sidebar");

    e.folders.set_thread_folder("t1", Some(&a.id)).unwrap();
    e.folders.set_thread_folder("t2", Some(&a.id)).unwrap();
    e.folders.set_thread_folder("t3", Some(&b.id)).unwrap();

    assert_eq!(ids(e.folders.threads_in(&a.id)), ["t2", "t1"]);
    assert_eq!(ids(e.folders.threads_in(&b.id)), ["t3"]);
    assert_eq!(ids(e.folders.threads_in(&c.id)), Vec::<String>::new());
    assert_eq!(ids(e.folders.threads_in("no-such-folder")), Vec::<String>::new());
    assert_eq!(ids(e.folders.unfiled()), ["t4", "t5"]);
    assert_eq!(
        e.folders.counts(),
        vec![
            FolderCount { folder_id: a.id.clone(), streams: 2, last_activity: 7000 },
            FolderCount { folder_id: b.id.clone(), streams: 1, last_activity: 3000 },
            FolderCount { folder_id: c.id.clone(), streams: 0, last_activity: 0 },
        ],
        "creation order, with the empty folder still listed"
    );
    let all = e.chats.list().len();
    let filed: usize = e.folders.counts().iter().map(|c| c.streams).sum();
    assert_eq!(filed + e.folders.unfiled().len(), all, "every stream is in exactly one place");

    // Move one, delete one: the numbers follow without any bookkeeping.
    e.folders.set_thread_folder("t2", Some(&c.id)).unwrap();
    e.chats.delete("t3").unwrap();
    let counts = e.folders.counts();
    assert_eq!((counts[0].streams, counts[0].last_activity), (1, 5000));
    assert_eq!((counts[1].streams, counts[1].last_activity), (0, 0));
    assert_eq!((counts[2].streams, counts[2].last_activity), (1, 7000));

    // And after a restart.
    let e = e.reopen();
    assert_eq!(e.folders.counts(), counts);
    assert_eq!(ids(e.folders.unfiled()), ["t4", "t5"]);
}

#[test]
fn a_stream_filed_in_a_folder_that_is_not_in_the_registry_counts_as_unfiled() {
    // The registry file was lost or edited by hand: the stream's folder id
    // points nowhere. It must not vanish from every list.
    let e = Env::new("dangling", &[("t1", 100, None)]);
    let a = e.folders.create("A").unwrap();
    e.folders.set_thread_folder("t1", Some(&a.id)).unwrap();
    let dir = e.dir.clone();
    let e = e.reopen();
    assert_eq!(e.thread("t1").folder.as_deref(), Some(a.id.as_str()));
    std::fs::remove_file(dir.join("folders.json")).unwrap();
    let e = e.reopen();
    assert!(e.folders.list().is_empty());
    assert_eq!(ids(e.folders.unfiled()), ["t1"]);
}

// ------------------------------------------------------------ the registry on disk

#[test]
fn the_registry_survives_a_reload() {
    let e = Env::new("registry", &[]);
    let a = e.folders.create("Alpha").unwrap();
    let b = e.folders.create("Beta").unwrap();
    let b = e.folders.rename(&b.id, "Beta two").unwrap();
    let before = e.folders.list();
    assert_eq!(before, vec![a.clone(), b.clone()]);
    assert!(e.dir.join("folders.json").is_file(), "its own file in the data dir");

    let e = e.reopen();
    assert_eq!(e.folders.list(), before);
    assert!(e.folders.create("alpha").is_err(), "uniqueness is read back too");
    e.folders.delete(&a.id).unwrap();
    let e = e.reopen();
    assert_eq!(e.folders.list(), vec![b]);
}

// ------------------------------------------------------------ proposals

#[test]
fn accepting_a_proposal_for_an_existing_folder_files_the_stream() {
    let e = Env::new("accept-existing", &[("t1", 100, Some("/work/site"))]);
    let f = e.folders.create("Site").unwrap();
    let p = e.folders.propose(existing("t1", &f.id)).unwrap();
    assert_eq!(e.folders.proposals(), vec![p.clone()]);
    assert_eq!(e.thread("t1").folder, None, "a proposal changes nothing until accepted");

    let filed = e.folders.accept(&p).unwrap();
    assert_eq!(filed, f);
    assert_eq!(e.thread("t1").folder.as_deref(), Some(f.id.as_str()));
    assert_eq!(e.thread("t1").project.as_deref(), Some("/work/site"), "where it runs did not move");
    assert_eq!(e.names(), ["Site"], "no folder was made");
    assert!(e.folders.proposals().is_empty(), "an accepted proposal is spent");
}

#[test]
fn accepting_a_proposal_for_a_new_folder_creates_it_then_files_the_stream() {
    let e = Env::new("accept-new", &[("t1", 100, Some("/work/site"))]);
    e.folders.create("First").unwrap();
    let p = e.folders.propose(new_folder("t1", "  Trips  ")).unwrap();
    assert_eq!(e.names(), ["First"], "proposing makes no folder");

    let made = e.folders.accept(&p).unwrap();
    assert_eq!(made.name, "Trips");
    assert_eq!(made.color, PALETTE[1], "the second folder, so the second colour");
    assert_eq!(e.names(), ["First", "Trips"]);
    assert_eq!(e.thread("t1").folder.as_deref(), Some(made.id.as_str()));
    assert_eq!(e.thread("t1").project.as_deref(), Some("/work/site"));
    assert!(e.folders.proposals().is_empty());
}

#[test]
fn a_new_folder_proposal_whose_name_now_exists_files_into_that_folder() {
    // Two streams proposed "Trips"; the first accept made it; the second must
    // not be stuck behind a duplicate-name error.
    let e = Env::new("accept-twice", &[("t1", 100, None), ("t2", 200, None)]);
    let p1 = e.folders.propose(new_folder("t1", "Trips")).unwrap();
    let p2 = e.folders.propose(new_folder("t2", "trips")).unwrap();
    let a = e.folders.accept(&p1).unwrap();
    let b = e.folders.accept(&p2).unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!(e.folders.list().len(), 1);
    assert_eq!(ids(e.folders.threads_in(&a.id)), ["t2", "t1"]);
}

#[test]
fn rejecting_a_proposal_changes_nothing_but_the_proposal() {
    let e = Env::new("reject", &[("t1", 100, Some("/work/site")), ("t2", 200, None)]);
    let f = e.folders.create("Site").unwrap();
    let p1 = e.folders.propose(existing("t1", &f.id)).unwrap();
    let p2 = e.folders.propose(new_folder("t2", "Brand new")).unwrap();
    let (t1, t2) = (e.bytes("t1"), e.bytes("t2"));
    let registry = e.folders.list();

    e.folders.reject(&p2).unwrap();
    e.folders.reject(&p1).unwrap();
    assert!(e.folders.proposals().is_empty());
    assert_eq!(e.folders.list(), registry, "a rejected new folder is never created");
    assert_eq!((e.bytes("t1"), e.bytes("t2")), (t1, t2), "no stream was touched");
    assert_eq!(e.thread("t1").folder, None);
    assert!(e.folders.reject(&p1).is_ok(), "rejecting twice is harmless");
}

#[test]
fn a_proposal_that_cannot_be_applied_changes_nothing() {
    let e = Env::new("accept-bad", &[("t1", 100, None)]);
    let f = e.folders.create("Site").unwrap();

    // The stream is gone: no folder may be created for it either.
    assert!(e.folders.accept(&new_folder("no-such-thread", "Ghost town")).is_err());
    assert!(e.folders.accept(&existing("no-such-thread", &f.id)).is_err());
    // The folder is gone.
    assert!(e.folders.accept(&existing("t1", "no-such-folder")).is_err());
    // A New proposal with nothing to call it.
    assert!(e.folders.accept(&new_folder("t1", "   ")).is_err());
    assert_eq!(e.names(), ["Site"]);
    assert_eq!(e.thread("t1").folder, None);

    // Proposing the same is refused up front, so nothing bad is ever stored.
    assert!(e.folders.propose(new_folder("no-such-thread", "X")).is_err());
    assert!(e.folders.propose(existing("t1", "no-such-folder")).is_err());
    assert!(e.folders.propose(new_folder("t1", "")).is_err());
    assert!(e.folders.proposals().is_empty());
}

#[test]
fn a_newer_proposal_for_a_stream_replaces_the_older_one() {
    let e = Env::new("replace", &[("t1", 100, None), ("t2", 200, None)]);
    let f = e.folders.create("Site").unwrap();
    let old = e.folders.propose(new_folder("t1", "Old idea")).unwrap();
    let other = e.folders.propose(new_folder("t2", "Elsewhere")).unwrap();
    let newer = e.folders.propose(existing("t1", &f.id)).unwrap();
    assert_eq!(e.folders.proposals(), vec![other.clone(), newer.clone()]);
    // Rejecting the one that was replaced does not drop the one that replaced it.
    e.folders.reject(&old).unwrap();
    assert_eq!(e.folders.proposals(), vec![other, newer]);
}

#[test]
fn proposals_survive_a_reload_and_stay_actionable() {
    let e = Env::new("proposals-reload", &[("t1", 100, None), ("t2", 200, None)]);
    let f = e.folders.create("Site").unwrap();
    let p1 = e.folders.propose(existing("t1", &f.id)).unwrap();
    let p2 = e.folders.propose(new_folder("t2", "Trips")).unwrap();
    assert_eq!(p1.created, 5, "a stamp the caller gave is kept");
    let zero = e.folders.propose(FolderProposal { created: 0, ..new_folder("t1", "Zero") }).unwrap();
    assert!(zero.created > 0, "and none is stamped now");
    // That last one replaced p1 for t1; put p1 back for the rest of the test.
    let p1 = e.folders.propose(p1).unwrap();
    let pending = e.folders.proposals();
    assert_eq!(pending, vec![p2.clone(), p1.clone()]);

    let e = e.reopen();
    assert_eq!(e.folders.proposals(), pending, "same proposals, same order, same reasons");
    assert_eq!(e.folders.proposals()[0].reason, "all about trips");
    // Still actionable after the restart.
    e.folders.accept(&p2).unwrap();
    e.folders.reject(&p1).unwrap();
    assert_eq!(e.names(), ["Site", "Trips"]);
    assert!(e.thread("t2").folder.is_some());
    assert_eq!(e.thread("t1").folder, None);
    let e = e.reopen();
    assert!(e.folders.proposals().is_empty(), "accepted and rejected stay gone");
}

#[test]
fn proposals_do_not_outlive_the_folder_or_the_stream_they_name() {
    let e = Env::new("proposals-gone", &[("t1", 100, None), ("t2", 200, None)]);
    let f = e.folders.create("Site").unwrap();
    e.folders.propose(existing("t1", &f.id)).unwrap();
    let keep = e.folders.propose(new_folder("t2", "Keep")).unwrap();
    e.folders.delete(&f.id).unwrap();
    assert_eq!(e.folders.proposals(), vec![keep.clone()], "a proposal for a deleted folder is dropped");
    e.chats.delete("t2").unwrap();
    assert!(e.folders.proposals().is_empty(), "and one for a deleted stream is not offered");
}
