//! The sidebar's thread list (UI3): which group a thread is in, the search filter and the meta
//! after the title. Pure functions over `Chats::list()`, so the view only draws.
//!
//! Groups are the ones core can back today (docs/cauldron/UI3-CORE-MAP.md): Needs you per
//! thread, Hibernating and Archived wait for core and are left out rather than faked.

use backspace_core::chat::ThreadInfo;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Group {
    /// An unopened reply, where the wireframe's "Needs you" sits.
    Unread,
    Running,
    Failed,
    Done,
}

impl Group {
    /// Drawing order, top to bottom.
    pub(crate) const ALL: [Group; 4] = [Group::Unread, Group::Running, Group::Failed, Group::Done];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Group::Unread => "Unread",
            Group::Running => "Running",
            Group::Failed => "Failed",
            Group::Done => "Done",
        }
    }

    /// A thread in two states goes to the first that applies: working beats a failed last reply,
    /// which beats an unread one.
    pub(crate) fn of(t: &ThreadInfo) -> Group {
        if t.busy {
            Group::Running
        } else if t.failed.is_some() {
            Group::Failed
        } else if t.unread {
            Group::Unread
        } else {
            Group::Done
        }
    }
}

/// One folder streams can be filed in (core's `folders::Folder`): its id, name and colour.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FolderRef {
    pub id: String,
    pub name: String,
    pub colour: u32,
}

/// Where each stream is filed (core's `Thread.folder`, which is not where it runs): the folders in
/// the order they were made, and each filed stream's folder id. A stream in none is Unfiled.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Filing {
    pub folders: Vec<FolderRef>,
    pub of: HashMap<String, String>,
}

impl Filing {
    /// The folder a stream is filed in, if any (a folder that is gone counts as none).
    pub(crate) fn folder_of(&self, t: &ThreadInfo) -> Option<&FolderRef> {
        let id = self.of.get(&t.id)?;
        self.folders.iter().find(|f| &f.id == id)
    }

    pub(crate) fn name_of(&self, t: &ThreadInfo) -> Option<String> {
        self.folder_of(t).map(|f| f.name.clone())
    }

    /// A folder's colour by name; Unfiled grey.
    pub(crate) fn colour(&self, name: &str) -> u32 {
        self.folders.iter().find(|f| f.name == name).map_or(0x8a8a93, |f| f.colour)
    }

    /// What "File by working directory" would make: a folder per working-directory name, in the
    /// order the list first meets it, each stream filed in its own. The tests use it as a filing.
    pub(crate) fn by_workdir(list: &[ThreadInfo]) -> Filing {
        const PALETTE: [u32; 6] = [0x8b5cf6, 0x3fb27f, 0xe0a32a, 0x5b8cf0, 0xd9669a, 0x38c0e8];
        let mut f = Filing::default();
        for t in list {
            if let Some(name) = folder(t) {
                let id = format!("wd-{name}");
                if !f.folders.iter().any(|x| x.id == id) {
                    let colour = PALETTE[f.folders.len() % PALETTE.len()];
                    f.folders.push(FolderRef { id: id.clone(), name, colour });
                }
                f.of.insert(t.id.clone(), id);
            }
        }
        f
    }
}

/// Streams by folder (Charles, 13 Nov 13:40Z: streams belong to a folder, for tidying up and
/// search): each folder in the order it was made, empty ones too, the streams in list order under
/// it, and Unfiled last. A query matches the title, the last words or the folder's name, ignoring
/// case; folders with no match are left out.
pub(crate) fn by_folder<'a>(list: &'a [ThreadInfo], query: &str, filing: &Filing) -> Vec<(String, Vec<&'a ThreadInfo>)> {
    let q = query.trim().to_lowercase();
    let hit = |t: &ThreadInfo| q.is_empty() || t.title.to_lowercase().contains(&q) || t.preview.to_lowercase().contains(&q) || filing.name_of(t).is_some_and(|f| f.to_lowercase().contains(&q));
    let mut out: Vec<(String, Vec<&ThreadInfo>)> = filing.folders.iter().map(|f| (f.name.clone(), Vec::new())).collect();
    let mut unfiled = Vec::new();
    for t in list.iter().filter(|t| hit(t)) {
        match filing.folder_of(t).and_then(|f| filing.folders.iter().position(|x| x.id == f.id)) {
            Some(i) => out[i].1.push(t),
            None => unfiled.push(t),
        }
    }
    if !q.is_empty() {
        out.retain(|(_, ts)| !ts.is_empty());
    }
    if !unfiled.is_empty() {
        out.push((NO_FOLDER.into(), unfiled));
    }
    out
}

/// The heading for streams with no folder.
pub(crate) const NO_FOLDER: &str = "Unfiled";

/// A stream waiting on you: an unopened reply (core has no other "needs you" yet).
pub(crate) fn needs_you(t: &ThreadInfo) -> bool {
    Group::of(t) == Group::Unread
}

/// One line of the sidebar's list (the pages board 5b7662c, folders direction B).
pub(crate) enum Item<'a> {
    /// "Needs you", pinned on top, with how many.
    Needs(usize),
    /// "Folders", with its +.
    Folders,
    /// A folder: its name, every stream in it, open or not, and a dot when closed over a stream
    /// that needs you.
    Folder { name: String, count: usize, open: bool, dot: bool },
    /// A stream, with its folder as a chip where the folder is not the heading above it.
    Row { t: &'a ThreadInfo, chip: Option<String> },
}

/// The sidebar's list. Searching, a flat list of hits, each with its folder. Otherwise: Needs you
/// (flat, those streams not repeated under their folder), Folders, then each folder, open when it
/// holds a running or waiting stream, or the selected one, unless `toggles` says otherwise, and
/// Unfiled last.
pub(crate) fn sidebar<'a>(list: &'a [ThreadInfo], query: &str, toggles: &HashMap<String, bool>, selected: Option<&str>, filing: &Filing) -> Vec<Item<'a>> {
    let chip = |t: &ThreadInfo| Some(filing.name_of(t).unwrap_or_else(|| NO_FOLDER.into()));
    if !query.trim().is_empty() {
        return by_folder(list, query, filing).into_iter().flat_map(|(_, ts)| ts).map(|t| Item::Row { t, chip: chip(t) }).collect();
    }
    let needs: Vec<&ThreadInfo> = list.iter().filter(|t| needs_you(t)).collect();
    // Needs you only when something does: an empty heading reads as a missing list.
    let mut out = Vec::new();
    if !needs.is_empty() {
        out.push(Item::Needs(needs.len()));
        out.extend(needs.iter().map(|t| Item::Row { t, chip: chip(t) }));
    }
    out.push(Item::Folders);
    for (name, ts) in by_folder(list, "", filing) {
        let waiting = ts.iter().any(|t| needs_you(t));
        let open = toggles.get(&name).copied().unwrap_or(waiting || ts.iter().any(|t| t.busy || Some(t.id.as_str()) == selected));
        out.push(Item::Folder { name: name.clone(), count: ts.len(), open, dot: !open && waiting });
        if open {
            out.extend(ts.into_iter().filter(|t| !needs_you(t)).map(|t| Item::Row { t, chip: None }));
        }
    }
    out
}

/// How long ago, as the row's age: 4m, 2h, 3d, 1w, 2y.
pub(crate) fn age(updated: u64, now: u64) -> String {
    let m = now.saturating_sub(updated) / 60_000;
    match m {
        0..=59 => format!("{}m", m.max(1)),
        60..=1439 => format!("{}h", m / 60),
        1440..=10079 => format!("{}d", m / 1440),
        10080..=525599 => format!("{}w", m / 10080),
        _ => format!("{}y", m / 525600),
    }
}

/// A stream's working directory's name (its project's last path part). Not its folder: filing is
/// `Filing`, and never changes where a stream runs.
pub(crate) fn folder(t: &ThreadInfo) -> Option<String> {
    t.project.as_ref().map(|p| p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string())
}

/// The row's meta after the title, as the board's `.m`: the project's folder name, else "no repo".
pub(crate) fn meta(t: &ThreadInfo) -> String {
    match &t.project {
        Some(p) => p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string(),
        None => "no repo".into(),
    }
}

#[cfg(any(test, debug_assertions))]
pub(crate) fn sample(id: &str, title: &str) -> ThreadInfo {
    use backspace_core::chat::{Route, RouteKind};
    ThreadInfo {
        id: id.into(),
        title: title.into(),
        created: 0,
        updated: 0,
        pinned: false,
        route: Route { kind: RouteKind::Cli, provider: "claude".into(), model: None },
        preview: String::new(),
        busy: false,
        unread: false,
        project: None,
        app: None,
        agent: None,
        members: Vec::new(),
        failed: None,
        ghost: false,
        expires_at: None,
    }
}

/// Made-up threads for screenshots (`CAULDRON_FIXTURE_THREADS=list`), one or more per group.
#[cfg(debug_assertions)]
pub(crate) fn fixture(now: u64) -> Vec<ThreadInfo> {
    let m = 60_000;
    let rows = [
        // id, title, project, minutes ago, pinned, busy, unread, failed
        ("u1", "Approve M3 start_task evidence", "Cauldron", 120, true, false, true, false),
        ("u2", "Pick a koi palette for light mode", "Ocean", 300, false, false, true, false),
        ("r1", "Rampart mutation score", "Cauldron", 4, false, true, false, false),
        ("r2", "Cauldron home, three layouts", "Cauldron", 1, false, true, false, false),
        ("f1", "M2 eval task texts", "Cauldron", 1500, false, false, false, true),
        ("f2", "Agent delegation A to F", "Cauldron", 2900, false, false, false, false),
        ("f3", "Router weights for the eval set", "Brainwave", 8700, false, false, false, false),
    ];
    let mut v: Vec<ThreadInfo> = rows
        .iter()
        .map(|&(id, title, project, ago, pinned, busy, unread, failed)| ThreadInfo {
            updated: now - ago * m,
            pinned,
            busy,
            unread,
            project: Some(project.into()),
            failed: failed.then(|| "usage limit".into()),
            // One thread with an agent; it still shows a state dot.
            agent: (id == "r1").then(|| "Rampart".into()),
            ..sample(id, title)
        })
        .collect();
    v.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.updated.cmp(&a.updated)));
    v
}

#[cfg(test)]
mod tests {
    use super::{age, by_folder, meta, sample, sidebar, Filing, Group, Item, NO_FOLDER};
    use std::collections::HashMap;

    #[test]
    fn each_thread_lands_in_one_group_by_precedence() {
        let mut t = sample("a", "A");
        assert_eq!(Group::of(&t), Group::Done);
        t.unread = true;
        assert_eq!(Group::of(&t), Group::Unread);
        t.failed = Some("usage limit".into());
        assert_eq!(Group::of(&t), Group::Failed, "failed beats unread");
        t.busy = true;
        assert_eq!(Group::of(&t), Group::Running, "running beats failed");
    }

    #[test]
    fn streams_group_by_folder_in_list_order_with_no_folder_last() {
        let mut v: Vec<_> = ["p", "q", "r", "s", "t"].iter().map(|id| sample(id, id)).collect();
        v[0].project = Some("/code/Cauldron".into());
        v[1].project = None;
        v[2].project = Some("C:\\code\\Ocean\\".into());
        v[3].project = Some("/code/Cauldron/".into());
        v[4].project = Some("/elsewhere/Cauldron".into());
        let g: Vec<(String, Vec<&str>)> = by_folder(&v, "", &Filing::by_workdir(&v)).iter().map(|(f, ts)| (f.clone(), ts.iter().map(|t| t.id.as_str()).collect())).collect();
        assert_eq!(g, vec![("Cauldron".to_string(), vec!["p", "s", "t"]), ("Ocean".into(), vec!["r"]), (NO_FOLDER.into(), vec!["q"])]);
    }

    /// Direction B: Needs you flat on top (not repeated under its folder), Folders, each folder
    /// open when it holds running or waiting work, closed (with a dot if it hides a waiting stream)
    /// otherwise, a toggle wins, and Unfiled last.
    #[test]
    fn the_sidebar_is_needs_you_then_folders() {
        let mut v: Vec<_> = ["a", "b", "c", "d", "e"].iter().map(|id| sample(id, id)).collect();
        v[0].project = Some("/code/Cauldron".into());
        v[0].unread = true;
        v[1].project = Some("/code/Cauldron".into());
        v[2].project = Some("/code/Ocean".into());
        v[3].project = Some("/code/Ocean".into());
        v[3].busy = true;
        let mut toggles = HashMap::new();
        let names = |items: &[Item]| -> Vec<String> {
            items
                .iter()
                .map(|i| match i {
                    Item::Needs(n) => format!("needs {n}"),
                    Item::Folders => "folders".into(),
                    Item::Folder { name, count, open, dot } => format!("{name} {count} {} {}", if *open { "open" } else { "shut" }, if *dot { "dot" } else { "-" }),
                    Item::Row { t, chip } => format!("{} {}", t.id, chip.clone().unwrap_or_default()),
                })
                .collect()
        };
        assert_eq!(
            names(&sidebar(&v, "", &toggles, None, &Filing::by_workdir(&v))),
            ["needs 1", "a Cauldron", "folders", "Cauldron 2 open -", "b ", "Ocean 2 open -", "c ", "d ", "Unfiled 1 shut -"]
        );
        toggles.insert("Cauldron".to_string(), false);
        toggles.insert("Unfiled".to_string(), true);
        assert_eq!(
            names(&sidebar(&v, "", &toggles, None, &Filing::by_workdir(&v))),
            ["needs 1", "a Cauldron", "folders", "Cauldron 2 shut dot", "Ocean 2 open -", "c ", "d ", "Unfiled 1 open -", "e "]
        );
        assert_eq!(names(&sidebar(&v, "ocean", &toggles, None, &Filing::by_workdir(&v))), ["c Ocean", "d Ocean"], "a search lists its hits flat, each with its folder");
        toggles.clear();
        assert!(names(&sidebar(&v, "", &toggles, Some("e"), &Filing::by_workdir(&v))).contains(&"Unfiled 1 open -".to_string()), "a folder opens over the selected stream");
    }

    /// With nothing waiting, there is no Needs you heading: the list starts at Folders.
    #[test]
    fn no_needs_you_heading_without_a_waiting_stream() {
        let mut v: Vec<_> = ["a", "b"].iter().map(|id| sample(id, id)).collect();
        v[0].project = Some("/code/Cauldron".into());
        let items = sidebar(&v, "", &HashMap::new(), None, &Filing::by_workdir(&v));
        assert!(matches!(items.first(), Some(Item::Folders)), "Folders first");
        assert!(!items.iter().any(|i| matches!(i, Item::Needs(_))));
        v[1].unread = true;
        assert!(matches!(sidebar(&v, "", &HashMap::new(), None, &Filing::by_workdir(&v)).first(), Some(Item::Needs(1))), "and back when one waits");
    }

    /// Filing is its own field: a stream filed in a folder shows there whatever its working
    /// directory; an empty folder still lists; a stream in a folder that is gone is Unfiled.
    #[test]
    fn folders_come_from_filing_not_the_working_directory() {
        use super::FolderRef;
        let mut v: Vec<_> = ["a", "b", "c"].iter().map(|id| sample(id, id)).collect();
        v[0].project = Some("/code/Cauldron".into());
        v[1].project = Some("/code/Cauldron".into());
        let mut f = Filing::default();
        f.folders.push(FolderRef { id: "f1".into(), name: "Launch".into(), colour: 0 });
        f.folders.push(FolderRef { id: "f2".into(), name: "Empty".into(), colour: 0 });
        f.of.insert("b".into(), "f1".into());
        f.of.insert("c".into(), "gone".into());
        let g: Vec<(String, Vec<&str>)> = by_folder(&v, "", &f).iter().map(|(n, ts)| (n.clone(), ts.iter().map(|t| t.id.as_str()).collect())).collect();
        assert_eq!(g, vec![("Launch".to_string(), vec!["b"]), ("Empty".into(), vec![]), (NO_FOLDER.into(), vec!["a", "c"])]);
        assert!(by_folder(&v, "b", &f).iter().all(|(n, _)| n != "Empty"), "a search drops empty folders");
    }

    #[test]
    fn age_reads_minutes_hours_days_weeks() {
        let m = 60_000u64;
        let now = 1000 * 10080 * m;
        assert_eq!([age(now, now), age(now - 4 * m, now), age(now - 120 * m, now), age(now - 3 * 1440 * m, now), age(now - 15 * 1440 * m, now), age(now - 800 * 1440 * m, now)], ["1m", "4m", "2h", "3d", "2w", "2y"]);
    }

    #[test]
    fn search_matches_title_preview_or_folder_ignoring_case() {
        let mut v = vec![sample("a", "Rampart mutation score"), sample("b", "Koi palette"), sample("c", "Build")];
        v[1].preview = "Light MODE colours".into();
        v[2].project = Some("/code/Brainwave".into());
        let f = Filing::by_workdir(&v);
        let ids = |q: &str| by_folder(&v, q, &f).into_iter().flat_map(|(_, ts)| ts.into_iter().map(|t| t.id.clone())).collect::<Vec<_>>();
        assert_eq!(ids("  ramPART "), ["a"]);
        assert_eq!(ids("mode"), ["b"]);
        assert_eq!(ids("brainWAVE"), ["c"], "the folder's name finds its streams");
        assert!(ids("nothing like it").is_empty());
        assert_eq!(ids("").len(), 3);
    }

    #[test]
    fn meta_is_the_project_folder_else_no_repo() {
        let mut t = sample("a", "A");
        t.preview = "last words".into();
        assert_eq!(meta(&t), "no repo");
        t.project = Some("C:\\code\\Cauldron\\".into());
        assert_eq!(meta(&t), "Cauldron");
        t.project = Some("/home/x/brainwave".into());
        assert_eq!(meta(&t), "brainwave");
    }
}
