//! The sidebar's thread list (UI3): which group a thread is in, the search filter and the meta
//! after the title. Pure functions over `Chats::list()`, so the view only draws.
//!
//! Groups are the ones core can back today (docs/cauldron/UI3-CORE-MAP.md): Needs you per
//! thread, Hibernating and Archived wait for core and are left out rather than faked.

use backspace_core::chat::ThreadInfo;

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

/// Threads matching `query` (case-insensitive, in the title or the preview), in groups, each
/// keeping `list()`'s order (pinned first, then newest). Empty groups are left out.
pub(crate) fn grouped<'a>(list: &'a [ThreadInfo], query: &str) -> Vec<(Group, Vec<&'a ThreadInfo>)> {
    let q = query.trim().to_lowercase();
    let hit = |t: &ThreadInfo| q.is_empty() || t.title.to_lowercase().contains(&q) || t.preview.to_lowercase().contains(&q);
    Group::ALL
        .iter()
        .map(|&g| (g, list.iter().filter(|t| Group::of(t) == g && hit(t)).collect::<Vec<_>>()))
        .filter(|(_, v)| !v.is_empty())
        .collect()
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
            // One agentic thread, so the sidebar shows a figure beside the state dots.
            agent: (id == "r1").then(|| "Rampart".into()),
            ..sample(id, title)
        })
        .collect();
    v.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.updated.cmp(&a.updated)));
    v
}

#[cfg(test)]
mod tests {
    use super::{grouped, meta, sample, Group};

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
    fn groups_come_in_order_keep_list_order_and_skip_empty_ones() {
        let mut v: Vec<_> = ["p", "q", "r", "s", "t"].iter().map(|id| sample(id, id)).collect();
        v[0].pinned = true;
        v[1].busy = true;
        v[2].unread = true;
        v[4].unread = true;
        let g = grouped(&v, "");
        let ids: Vec<(Group, Vec<&str>)> = g.iter().map(|(g, ts)| (*g, ts.iter().map(|t| t.id.as_str()).collect())).collect();
        assert_eq!(ids, vec![(Group::Unread, vec!["r", "t"]), (Group::Running, vec!["q"]), (Group::Done, vec!["p", "s"])]);
    }

    #[test]
    fn search_matches_title_or_preview_ignoring_case() {
        let mut v = vec![sample("a", "Rampart mutation score"), sample("b", "Koi palette")];
        v[1].preview = "Light MODE colours".into();
        let ids = |q: &str| grouped(&v, q).into_iter().flat_map(|(_, ts)| ts.into_iter().map(|t| t.id.clone())).collect::<Vec<_>>();
        assert_eq!(ids("  ramPART "), ["a"]);
        assert_eq!(ids("mode"), ["b"]);
        assert!(ids("nothing like it").is_empty());
        assert_eq!(ids("").len(), 2);
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
