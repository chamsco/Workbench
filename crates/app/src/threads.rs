//! The sidebar's thread list (UI3): which group a thread is in, the search filter and the meta
//! after the title. Pure functions over `Chats::list()`, so the view only draws.
//!
//! Groups are the Tight tiles board's: Needs you (an unread or failed last reply), Running and
//! Done. Hibernating and Archived wait for core and are left out rather than faked.

use backspace_core::chat::ThreadInfo;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Group {
    /// A reply you have not read, or one that failed: either way it waits on you.
    NeedsYou,
    Running,
    /// Finished in the last day, or pinned: still in view under "Streams" (Charles, 23 Nov: a new
    /// stream "does not go into the left sidebar" because it went straight to folded Done).
    Recent,
    /// Finished more than a day ago.
    Done,
    /// Put away; `of` never returns it, the sidebar adds it as its own shelf.
    Archived,
    /// Snoozed until later; a shelf like Archived.
    Snoozed,
}

impl Group {
    /// Drawing order, top to bottom.
    pub(crate) const ALL: [Group; 4] = [Group::NeedsYou, Group::Running, Group::Recent, Group::Done];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Group::NeedsYou => "Needs you",
            Group::Running => "Running",
            Group::Recent => "Recent",
            Group::Done => "Done",
            Group::Archived => "Archived",
            Group::Snoozed => "Snoozed",
        }
    }

    /// A stream in two states goes to the first that applies: working beats a reply that needs you.
    pub(crate) fn of(t: &ThreadInfo) -> Group {
        if t.busy {
            Group::Running
        } else if t.failed.is_some() || t.unread {
            Group::NeedsYou
        } else {
            Group::Done
        }
    }
}

/// How long a finished stream stays under Streams before it folds into Done.
pub(crate) const RECENT_MS: u64 = 24 * 60 * 60 * 1000;

/// The sidebar group of `t` at `now`: `Group::of`, with a stream finished in the last day (or
/// pinned) kept in view under Streams.
pub(crate) fn place(t: &ThreadInfo, now: u64) -> Group {
    match Group::of(t) {
        Group::Done if t.pinned || now.saturating_sub(t.updated) < RECENT_MS => Group::Recent,
        g => g,
    }
}

/// Threads matching `query` (case-insensitive, in the title, the preview or the folder's name), in
/// groups, each
/// keeping `list()`'s order (pinned first, then newest). Empty groups are left out.
pub(crate) fn grouped<'a>(list: &'a [ThreadInfo], query: &str) -> Vec<(Group, Vec<&'a ThreadInfo>)> {
    grouped_at(list, query, backspace_core::chat::now_ms())
}

pub(crate) fn grouped_at<'a>(list: &'a [ThreadInfo], query: &str, now: u64) -> Vec<(Group, Vec<&'a ThreadInfo>)> {
    let q = query.trim().to_lowercase();
    let hit = |t: &ThreadInfo| q.is_empty() || t.title.to_lowercase().contains(&q) || t.preview.to_lowercase().contains(&q) || (t.project.is_some() && meta(t).to_lowercase().contains(&q));
    Group::ALL
        .iter()
        .map(|&g| (g, list.iter().filter(|t| place(t, now) == g && hit(t)).collect::<Vec<_>>()))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// How a row reads where the dot was (Charles, 22 Nov, after T3 Code): a word, not a colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Status {
    Working,
    Failed,
    /// Its last reply asks you something.
    Input,
    /// Finished and not read yet.
    Done,
}

impl Status {
    pub(crate) fn of(t: &ThreadInfo) -> Option<Status> {
        if t.busy {
            Some(Status::Working)
        } else if t.failed.is_some() {
            Some(Status::Failed)
        } else if t.asking {
            Some(Status::Input)
        } else if t.unread {
            Some(Status::Done)
        } else {
            None
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Status::Working => "Working",
            Status::Failed => "\u{2715} Failed",
            Status::Input => "? Input",
            Status::Done => "\u{2713} Done",
        }
    }
}

/// Snoozed (Charles, 22 Nov): out of the list until its time comes, unless a search asks for it.
pub(crate) fn snoozed(t: &ThreadInfo, now: u64) -> bool {
    t.snoozed_until > now
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
        archived: false,
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
        side_of: None,
        branch: None,
        models: "Claude".into(),
        asking: false,
        snoozed_until: 0,
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
            // One reply ends on a question.
            asking: id == "u2",
            ..sample(id, title)
        })
        .collect();
    v.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.updated.cmp(&a.updated)));
    v
}

#[cfg(test)]
mod tests {
    use super::{grouped, grouped_at, meta, place, sample, Group, RECENT_MS};

    #[test]
    fn a_row_says_its_state_in_words() {
        use super::Status;
        let mut t = sample("a", "A");
        assert_eq!(Status::of(&t), None, "a read, finished stream stays quiet");
        t.unread = true;
        assert_eq!(Status::of(&t).map(Status::label), Some("\u{2713} Done"));
        t.asking = true;
        assert_eq!(Status::of(&t).map(Status::label), Some("? Input"), "a question beats done");
        t.failed = Some("usage limit".into());
        assert_eq!(Status::of(&t), Some(Status::Failed));
        t.busy = true;
        assert_eq!(Status::of(&t), Some(Status::Working), "working beats everything");
    }

    #[test]
    fn each_thread_lands_in_one_group_by_precedence() {
        let mut t = sample("a", "A");
        assert_eq!(Group::of(&t), Group::Done);
        t.unread = true;
        assert_eq!(Group::of(&t), Group::NeedsYou, "an unread reply needs you");
        t.unread = false;
        t.failed = Some("usage limit".into());
        assert_eq!(Group::of(&t), Group::NeedsYou, "so does a failed one");
        t.busy = true;
        assert_eq!(Group::of(&t), Group::Running, "running beats needs you");
    }

    #[test]
    fn groups_come_in_order_keep_list_order_and_skip_empty_ones() {
        let mut v: Vec<_> = ["p", "q", "r", "s", "t"].iter().map(|id| sample(id, id)).collect();
        v[0].pinned = true;
        v[1].busy = true;
        v[2].unread = true;
        v[4].unread = true;
        let g = grouped_at(&v, "", 10 * RECENT_MS);
        let ids: Vec<(Group, Vec<&str>)> = g.iter().map(|(g, ts)| (*g, ts.iter().map(|t| t.id.as_str()).collect())).collect();
        assert_eq!(ids, vec![(Group::NeedsYou, vec!["r", "t"]), (Group::Running, vec!["q"]), (Group::Recent, vec!["p"]), (Group::Done, vec!["s"])]);
    }

    /// Regression (Charles, 23 Nov): a stream that just finished and was read went straight into
    /// Done, which starts folded, so a new stream never showed in the sidebar.
    #[test]
    fn a_stream_finished_today_stays_in_view_until_a_day_has_passed() {
        let mut t = sample("a", "A");
        let now = 100 * RECENT_MS;
        t.updated = now - 60_000;
        assert_eq!(place(&t, now), Group::Recent);
        t.updated = now - RECENT_MS - 1;
        assert_eq!(place(&t, now), Group::Done);
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
