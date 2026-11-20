//! What the support card shows for the open stream (Tight tiles rule 1), read from the stream's
//! folder: Changes (the working tree's diff), Git (staged and unstaged files, recent commits) and
//! Files (the folder's tree). Each is read off the main thread with `git` or the file system, so
//! the panels draw Loading, the content, an empty state or the error. The parsers are pure.

use std::path::{Path, PathBuf};

/// A panel's content as it stands.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

/// One changed file: its path and the lines added and removed (None for a binary file).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FileStat {
    pub(crate) path: String,
    pub(crate) added: Option<u32>,
    pub(crate) removed: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiffKind {
    Hunk,
    Add,
    Del,
    Same,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DiffLine {
    pub(crate) kind: DiffKind,
    pub(crate) text: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Changes {
    pub(crate) files: Vec<FileStat>,
    /// The first changed file's diff, cut to `DIFF_MAX` lines.
    pub(crate) diff: Vec<DiffLine>,
}

/// `git status` in two lists: (status letter, path).
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct GitState {
    pub(crate) branch: Option<String>,
    pub(crate) staged: Vec<(char, String)>,
    pub(crate) unstaged: Vec<(char, String)>,
    /// (short hash, subject), newest first.
    pub(crate) commits: Vec<(String, String)>,
}

/// One row of the file tree: its depth, name, whether it is a folder and its git status letter.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub(crate) depth: usize,
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) dir: bool,
    pub(crate) mark: Option<char>,
}

const DIFF_MAX: usize = 400;
const COMMITS: usize = 8;
/// A folder lists at most this many entries, so a huge one cannot stall the panel.
const DIR_MAX: usize = 500;

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = backspace_runner::cmd("git").arg("-C").arg(dir).args(args).output().map_err(|e| format!("git could not run: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.contains("not a git repository") { "This folder is not a git repository.".into() } else { err })
    }
}

/// Fails with a sentence a person can read when the folder is not here.
fn here(dir: &Path) -> Result<(), String> {
    if dir.is_dir() { Ok(()) } else { Err("This stream's folder is not on this machine.".into()) }
}

pub(crate) fn read_changes(dir: &Path) -> Result<Changes, String> {
    here(dir)?;
    let files = parse_numstat(&git(dir, &["diff", "HEAD", "--numstat", "--no-color"])?);
    let diff = match files.first() {
        Some(f) => parse_diff(&git(dir, &["diff", "HEAD", "--no-color", "--", &f.path])?),
        None => Vec::new(),
    };
    Ok(Changes { files, diff })
}

pub(crate) fn read_git(dir: &Path) -> Result<GitState, String> {
    here(dir)?;
    let (staged, unstaged) = parse_status(&git(dir, &["status", "--porcelain=v1"])?);
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).ok().map(|b| b.trim().to_string()).filter(|b| !b.is_empty() && b != "HEAD");
    // A repository with no commit yet has no log; that is an empty list, not an error.
    let commits = git(dir, &["log", &format!("-{COMMITS}"), "--pretty=format:%h%x09%s"]).map(|l| parse_log(&l)).unwrap_or_default();
    Ok(GitState { branch, staged, unstaged, commits })
}

/// The folder's tree: the top level, and the inside of each folder in `open`. Folders first,
/// then files, each by name; `.git` is left out. Changed files carry their git letter.
pub(crate) fn read_files(dir: &Path, open: &std::collections::BTreeSet<PathBuf>) -> Result<Vec<Entry>, String> {
    here(dir)?;
    let marks: std::collections::HashMap<String, char> = git(dir, &["status", "--porcelain=v1"])
        .map(|s| {
            let (a, b) = parse_status(&s);
            b.into_iter().chain(a).map(|(c, p)| (p, c)).collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    walk(dir, dir, 0, open, &marks, &mut out)?;
    Ok(out)
}

fn walk(root: &Path, dir: &Path, depth: usize, open: &std::collections::BTreeSet<PathBuf>, marks: &std::collections::HashMap<String, char>, out: &mut Vec<Entry>) -> Result<(), String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("Could not read {}: {e}", dir.display()))?;
    let mut items: Vec<(bool, String, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != ".git")
        .map(|e| (e.file_type().map(|t| t.is_dir()).unwrap_or(false), e.file_name().to_string_lossy().to_string(), e.path()))
        .take(DIR_MAX)
        .collect();
    items.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    for (is_dir, name, path) in items {
        let rel = path.strip_prefix(root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        let mark = if is_dir { marks.keys().any(|k| k.starts_with(&format!("{rel}/"))).then_some('•') } else { marks.get(&rel).copied() };
        out.push(Entry { depth, path: path.clone(), name, dir: is_dir, mark });
        if is_dir && open.contains(&path) {
            walk(root, &path, depth + 1, open, marks, out)?;
        }
    }
    Ok(())
}

/// `git diff --numstat`: "added<TAB>removed<TAB>path", "-" for a binary file.
pub(crate) fn parse_numstat(s: &str) -> Vec<FileStat> {
    s.lines()
        .filter_map(|l| {
            let mut it = l.splitn(3, '\t');
            let (a, r, p) = (it.next()?, it.next()?, it.next()?);
            Some(FileStat { path: p.to_string(), added: a.parse().ok(), removed: r.parse().ok() })
        })
        .collect()
}

/// A unified diff's body: hunk heads, added, removed and context lines; the file headers are
/// dropped. Cut at `DIFF_MAX` lines.
pub(crate) fn parse_diff(s: &str) -> Vec<DiffLine> {
    let mut out = Vec::new();
    let mut body = false;
    for l in s.lines() {
        if l.starts_with("@@") {
            body = true;
            out.push(DiffLine { kind: DiffKind::Hunk, text: l.to_string() });
        } else if !body || l.starts_with("diff --git") {
            body = l.starts_with("@@");
        } else if let Some(t) = l.strip_prefix('+') {
            out.push(DiffLine { kind: DiffKind::Add, text: t.to_string() });
        } else if let Some(t) = l.strip_prefix('-') {
            out.push(DiffLine { kind: DiffKind::Del, text: t.to_string() });
        } else if let Some(t) = l.strip_prefix(' ') {
            out.push(DiffLine { kind: DiffKind::Same, text: t.to_string() });
        }
        if out.len() >= DIFF_MAX {
            break;
        }
    }
    out
}

/// `git status --porcelain=v1`: the index column is staged, the work-tree column unstaged;
/// "??" is an untracked file, listed as unstaged with git's own "?".
pub(crate) fn parse_status(s: &str) -> (Vec<(char, String)>, Vec<(char, String)>) {
    let (mut staged, mut unstaged) = (Vec::new(), Vec::new());
    for l in s.lines() {
        let b = l.as_bytes();
        if b.len() < 4 {
            continue;
        }
        let (x, y) = (b[0] as char, b[1] as char);
        // A rename reads "old -> new"; the new name is the file.
        let path = l[3..].rsplit(" -> ").next().unwrap_or(&l[3..]).trim_matches('"').to_string();
        if x == '?' {
            unstaged.push(('?', path));
            continue;
        }
        if x != ' ' {
            staged.push((x, path.clone()));
        }
        if y != ' ' {
            unstaged.push((y, path));
        }
    }
    (staged, unstaged)
}

/// `git log --pretty=format:%h%x09%s`.
pub(crate) fn parse_log(s: &str) -> Vec<(String, String)> {
    s.lines().filter_map(|l| l.split_once('\t').map(|(h, m)| (h.to_string(), m.to_string()))).collect()
}

/// One panel's content, as the shell keeps it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Data {
    Changes(Load<Changes>),
    Git(Load<GitState>),
    Files(Load<Vec<Entry>>),
}

impl Data {
    fn loading(p: Panel) -> Option<Data> {
        match p {
            Panel::Changes => Some(Data::Changes(Load::Loading)),
            Panel::Git => Some(Data::Git(Load::Loading)),
            Panel::Files => Some(Data::Files(Load::Loading)),
            _ => None,
        }
    }
}

fn load<T>(r: Result<T, String>) -> Load<T> {
    match r {
        Ok(v) => Load::Ready(v),
        Err(e) => Load::Failed(e),
    }
}

use super::*;
use gpui_kit::prelude::FluentBuilder;

impl Cauldron {
    fn stream_dir(&self) -> Option<String> {
        self.thread.as_ref().and_then(|t| t.project.clone())
    }

    /// Starts reading `p`'s content for the open stream's folder, once; a refresh drops it first.
    pub(super) fn load_panel(&mut self, p: Panel, cx: &mut Context<Self>) {
        let Some(dir) = self.stream_dir() else { return };
        let key = (p, dir.clone());
        if self.support_data.contains_key(&key) {
            return;
        }
        let Some(start) = Data::loading(p) else { return };
        self.support_data.insert(key.clone(), start);
        let open = self.files_open.clone();
        let task = cx.background_executor().spawn(async move {
            let d = Path::new(&dir);
            match p {
                Panel::Changes => Data::Changes(load(read_changes(d))),
                Panel::Git => Data::Git(load(read_git(d))),
                _ => Data::Files(load(read_files(d, &open))),
            }
        });
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let data = task.await;
            let _ = this.update(cx, |c, cx| {
                c.support_data.insert(key, data);
                cx.notify();
            });
        })
        .detach();
    }

    /// Forgets `p`'s content so the next render reads it again.
    fn refresh_panel(&mut self, p: Panel) {
        if let Some(dir) = self.stream_dir() {
            self.support_data.remove(&(p, dir));
        }
    }

    /// The support card's body for `p`: the panel's content, its loading, empty or error state,
    /// or, for a panel with no source yet, a marked placeholder.
    pub(super) fn support_body(&self, p: Panel, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let col = || div().id("support-body").size_full().flex().flex_col().gap(px(6.)).px(px(14.)).pt(px(14.)).pb(px(12.)).overflow_y_scroll().text_size(px(12.5));
        // A state in place of content: a title, a line, and the whole as one accessible name.
        let state = |id: &'static str, title: String, line: String, tag: Option<&'static str>| {
            col()
                .child(
                    div()
                        .id(id)
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .aria_label(format!("{title}. {line}"))
                        .child(div().flex().items_center().gap(px(8.)).font_weight(FontWeight::SEMIBOLD).text_color(fg2).child(title).when_some(tag, |d, t| d.child(div().px(px(5.)).h(px(17.)).flex().items_center().rounded(px(radius::TAG)).border_1().border_color(c(pal.line)).text_size(px(10.5)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg4)).child(t))))
                        .child(div().text_size(px(12.)).line_height(px(17.)).text_color(fg3).child(line))
                        .test_support(),
                )
                .into_any_element()
        };
        if let Some(why) = p.placeholder() {
            return state("support-placeholder", p.title().into(), why.into(), Some("Placeholder"));
        }
        let Some(t) = self.thread.as_ref() else {
            return state("support-empty", format!("No stream open"), format!("Open a stream to see its {}.", p.title().to_lowercase()), None);
        };
        if p == Panel::Highlights {
            let ls = super::thread::lines(t);
            let replies = ls.iter().filter(|l| !l.mine).count();
            let asks = ls.len() - replies;
            let stat = |id: &'static str, n: String, label: &'static str| div().id(id).flex().flex_col().aria_label(format!("{label}: {n}")).child(div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).font_features(tnum()).text_color(fg).child(n)).child(div().text_size(px(11.5)).text_color(fg3).child(label)).test_support();
            let mut b = col().child(
                div()
                    .flex()
                    .gap(px(18.))
                    .pb(px(14.))
                    .mb(px(8.))
                    .border_b_1()
                    .border_color(c(pal.line))
                    .child(stat("hl-asks", asks.to_string(), "Messages"))
                    .child(stat("hl-replies", replies.to_string(), "Responses"))
                    .child(stat("hl-cost", format!("${:.2}", super::thread::cost(t)), "Cost")),
            );
            let folder = t.project.clone().unwrap_or_else(|| "No folder".into());
            b = b.child(div().id("hl-route").text_color(fg2).aria_label(format!("Answered by {}", t.route.provider)).child(format!("Answered by {}", t.route.provider)).test_support());
            b = b.child(div().id("hl-folder").text_color(fg3).truncate().aria_label(folder.clone()).child(folder).test_support());
            return b.into_any_element();
        }
        if p == Panel::Browser {
            return state("support-empty", "No dev server".into(), "Cauldron does not see a dev server for this stream.".into(), None);
        }
        let Some(dir) = t.project.clone() else {
            return state("support-empty", "No folder".into(), format!("This stream has no folder, so there is no {} to show.", p.title().to_lowercase()), None);
        };
        let name = Path::new(&dir).file_name().map_or_else(|| dir.clone(), |n| n.to_string_lossy().to_string());
        let refresh = |cx: &mut Context<Self>| {
            div()
                .id("support-refresh")
                .size(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label("Refresh")
                .child(glyph("refresh-cw", 13., fg3))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.refresh_panel(p);
                    cx.notify();
                }))
                .test_support()
        };
        let head = |label: String, cx: &mut Context<Self>| div().flex().items_center().gap(px(8.)).text_size(px(12.)).text_color(fg3).child(div().flex_1().min_w_0().truncate().child(label)).child(refresh(cx));
        let failed = |e: &String, cx: &mut Context<Self>| {
            col()
                .child(div().id("support-error").flex().flex_col().gap(px(4.)).aria_label(e.clone()).child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(pal.bad)).child(format!("{} could not be read", p.title()))).child(div().text_size(px(12.)).text_color(fg3).child(e.clone())).test_support())
                .child(div().pt(px(4.)).child(refresh(cx)))
                .into_any_element()
        };
        let loading = || state("support-loading", format!("Reading {name}"), "One moment.".into(), None);
        let row = |id: (&'static str, usize)| div().id(id).h(px(28.)).flex().items_center().gap(px(8.)).px(px(6.)).rounded(px(radius::CONTROL)).text_color(fg2);
        match self.support_data.get(&(p, dir.clone())) {
            None | Some(Data::Changes(Load::Loading)) | Some(Data::Git(Load::Loading)) | Some(Data::Files(Load::Loading)) => loading(),
            Some(Data::Changes(Load::Failed(e))) | Some(Data::Git(Load::Failed(e))) | Some(Data::Files(Load::Failed(e))) => failed(e, cx),
            Some(Data::Changes(Load::Ready(ch))) => {
                if ch.files.is_empty() {
                    return state("support-empty", "No changes".into(), format!("{name} matches its last commit."), None);
                }
                let (a, r) = ch.files.iter().fold((0, 0), |(a, r), f| (a + f.added.unwrap_or(0), r + f.removed.unwrap_or(0)));
                let n = ch.files.len();
                let mut b = col().child(head(format!("{n} file{} changed · +{a} −{r}", if n == 1 { "" } else { "s" }), cx));
                for (i, f) in ch.files.iter().enumerate() {
                    let counts = match (f.added, f.removed) {
                        (Some(a), Some(r)) => format!("+{a} −{r}"),
                        _ => "binary".into(),
                    };
                    b = b.child(
                        row(("change", i))
                            .when(i == 0, |d| d.bg(c(pal.hover)).text_color(fg))
                            .aria_label(format!("{} {counts}", f.path))
                            .child(div().flex_1().min_w_0().truncate().child(f.path.clone()))
                            .child(div().flex_none().font_family(MONO).text_size(px(11.)).text_color(fg3).child(counts))
                            .test_support(),
                    );
                }
                let mut diff = div().id("diff").mt(px(8.)).flex().flex_col().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line)).overflow_hidden().font_family(MONO).text_size(px(11.5)).line_height(px(19.));
                diff = diff.bg(super::backdrop::surface(self.painting, pal.panel, super::backdrop::GLASS_TERM));
                for l in &ch.diff {
                    let (bg, color, sign) = match l.kind {
                        DiffKind::Hunk => (Some(c(pal.raised)), c(pal.fg4), ""),
                        DiffKind::Add => (Some(ca(pal.ok, 0.10)), fg, "+ "),
                        DiffKind::Del => (Some(ca(pal.bad, 0.10)), fg, "- "),
                        DiffKind::Same => (None, fg2, "  "),
                    };
                    diff = diff.child(div().px(px(10.)).when_some(bg, |d, b| d.bg(b)).text_color(color).child(format!("{sign}{}", l.text)));
                }
                b.child(diff.test_support()).into_any_element()
            }
            Some(Data::Git(Load::Ready(g))) => {
                let mut b = col().child(head(g.branch.clone().map_or_else(|| "No branch".into(), |br| format!("On {br}")), cx));
                let section = |title: &'static str| div().pt(px(8.)).pb(px(2.)).text_size(px(11.5)).text_color(fg3).child(title);
                let mut k = 0usize;
                for (title, list) in [("Staged", &g.staged), ("Changes", &g.unstaged)] {
                    b = b.child(section(title));
                    if list.is_empty() {
                        b = b.child(div().px(px(6.)).text_size(px(12.)).text_color(c(pal.fg4)).child("Nothing here"));
                    }
                    for (letter, path) in list {
                        b = b.child(
                            row(("git-file", k))
                                .aria_label(format!("{letter} {path}"))
                                .child(div().w(px(12.)).flex_none().font_family(MONO).text_size(px(11.)).text_color(c(pal.warn)).child(letter.to_string()))
                                .child(div().flex_1().min_w_0().truncate().child(path.clone()))
                                .test_support(),
                        );
                        k += 1;
                    }
                }
                b = b.child(section("Recent commits"));
                if g.commits.is_empty() {
                    b = b.child(div().px(px(6.)).text_size(px(12.)).text_color(c(pal.fg4)).child("No commits yet"));
                }
                for (i, (hash, msg)) in g.commits.iter().enumerate() {
                    b = b.child(
                        row(("commit", i))
                            .aria_label(msg.clone())
                            .child(div().flex_none().font_family(MONO).text_size(px(11.)).text_color(fg3).child(hash.clone()))
                            .child(div().flex_1().min_w_0().truncate().child(msg.clone()))
                            .test_support(),
                    );
                }
                b.into_any_element()
            }
            Some(Data::Files(Load::Ready(es))) => {
                if es.is_empty() {
                    return state("support-empty", "Empty folder".into(), format!("{name} has no files yet."), None);
                }
                let mut b = col().gap(px(0.)).child(div().pb(px(6.)).child(head(name.clone(), cx)));
                for (i, e) in es.iter().enumerate() {
                    let path = e.path.clone();
                    let (dir_row, open) = (e.dir, self.files_open.contains(&e.path));
                    b = b.child(
                        div()
                            .id(("file-row", i))
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .pl(px(6.0 + 14.0 * e.depth as f32))
                            .pr(px(6.))
                            .rounded(px(radius::CONTROL))
                            .text_color(fg2)
                            .when(dir_row, |d| d.cursor_pointer().hover(|d| d.bg(c(pal.hover))))
                            .aria_label(e.name.clone())
                            .aria_expanded(dir_row && open)
                            .child(glyph(if !dir_row { "file" } else if open { "folder-open" } else { "folder" }, 13., fg3))
                            .child(div().flex_1().min_w_0().truncate().child(e.name.clone()))
                            .when_some(e.mark, |d, m| d.child(div().flex_none().font_family(MONO).text_size(px(11.)).text_color(c(pal.warn)).child(m.to_string())))
                            .when(dir_row, |d| {
                                d.on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.files_open.remove(&path) {
                                        this.files_open.insert(path.clone());
                                    }
                                    this.refresh_panel(Panel::Files);
                                    cx.notify();
                                }))
                            })
                            .test_support(),
                    );
                }
                b.into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports: a glob of the shell brings GPUI's `test` macro, which shadows Rust's.
    use super::{parse_diff, parse_log, parse_numstat, parse_status, read_changes, read_files, read_git, Data, DiffKind, FileStat, Load};
    use super::super::Panel;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn numstat_reads_counts_and_binaries() {
        let f = parse_numstat("214\t38\tcrates/core/src/eval.rs\n-\t-\tlogo.png\n");
        assert_eq!(f[0], FileStat { path: "crates/core/src/eval.rs".into(), added: Some(214), removed: Some(38) });
        assert_eq!((f[1].added, f[1].removed), (None, None), "binary");
    }

    #[test]
    fn a_diff_keeps_hunks_and_lines_and_drops_headers() {
        let d = parse_diff("diff --git a/x b/x\nindex 1..2 100644\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n");
        let kinds: Vec<DiffKind> = d.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, [DiffKind::Hunk, DiffKind::Same, DiffKind::Del, DiffKind::Add]);
        assert_eq!((d[2].text.as_str(), d[3].text.as_str()), ("old", "new"), "the --- and +++ headers are not lines");
    }

    #[test]
    fn status_splits_staged_and_unstaged() {
        let (s, u) = parse_status("M  a.rs\n M b.rs\nMM c.rs\n?? new.txt\nR  old.rs -> moved.rs\n");
        assert_eq!(s, [('M', "a.rs".into()), ('M', "c.rs".into()), ('R', "moved.rs".into())]);
        assert_eq!(u, [('M', "b.rs".into()), ('M', "c.rs".into()), ('?', "new.txt".into())]);
    }

    #[test]
    fn log_reads_hash_and_subject() {
        assert_eq!(parse_log("abc1234\tFix the flag race\ndef5678\tBundle fonts"), [("abc1234".into(), "Fix the flag race".into()), ("def5678".into(), "Bundle fonts".into())]);
    }

    #[test]
    fn a_missing_folder_says_so() {
        let gone = std::env::temp_dir().join("cauldron-support-no-such-folder");
        assert_eq!(read_changes(&gone), Err("This stream's folder is not on this machine.".into()));
    }

    /// A repository with one commit ("First": src/a.rs), then src/a.rs changed and notes.md new.
    fn repo(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cauldron-support-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let run = |args: &[&str]| assert!(backspace_runner::cmd("git").arg("-C").arg(&dir).args(args).output().unwrap().status.success(), "git {args:?}");
        run(&["init", "-q"]);
        std::fs::write(dir.join("src/a.rs"), "one\ntwo\n").unwrap();
        run(&["add", "."]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "First"]);
        std::fs::write(dir.join("src/a.rs"), "one\nthree\n").unwrap();
        std::fs::write(dir.join("notes.md"), "hi\n").unwrap();
        dir
    }

    /// The support card on a real stream (Tight tiles piece 5): Changes, Git and Files read the
    /// stream's folder (Loading first), a folder in Files opens, Browser says it sees no dev
    /// server, Trace is a marked placeholder, Highlights counts the stream, and a folder that is
    /// not on this machine is an error with a refresh.
    #[gpui_kit::test]
    fn the_support_panels_read_the_streams_folder(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, chats) = super::super::thread::tests::open(cx);
        let dir = repo("ui");
        let route = backspace_core::chat::Route { kind: backspace_core::chat::RouteKind::Cli, provider: "cauldron-test-missing-cli".into(), model: None };
        let t = chats.create_in(route, backspace_core::chat::Scope { project: Some(dir.to_string_lossy().to_string()), ..Default::default() });
        handle
            .update(cx, |c, _, _| {
                c.refresh();
                c.open_thread(t.id.clone());
                c.toggle_panel(Panel::Changes);
            })
            .unwrap();
        let settle = |cx: &mut gpui_kit::TestAppContext| {
            handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
            for _ in 0..50 {
                cx.run_until_parked();
                let busy = handle.update(cx, |c, _, _| c.support_data.values().any(|d| matches!(d, Data::Changes(Load::Loading) | Data::Git(Load::Loading) | Data::Files(Load::Loading)))).unwrap();
                if !busy {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        // The card fades in on the wall clock: start from rest.
        handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("support-loading").is_some(), "Loading while git runs");
        })
        .unwrap();
        settle(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("change", 0usize)).label(), Some("src/a.rs +1 −1"));
            assert!(window.try_find("diff").is_some(), "the first file's diff");
            window.click(("rb", 2usize), cx);
        })
        .unwrap();
        settle(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("git-file", 0usize)).label(), Some("M src/a.rs"));
            assert_eq!(window.find(("git-file", 1usize)).label(), Some("? notes.md"));
            assert_eq!(window.find(("commit", 0usize)).label(), Some("First"));
            window.click(("rb", 5usize), cx);
        })
        .unwrap();
        settle(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("file-row", 0usize)).label(), Some("src"));
            window.click(("file-row", 0usize), cx);
        })
        .unwrap();
        settle(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("file-row", 1usize)).label(), Some("a.rs"), "the folder opened");
            window.click(("rb", 4usize), cx);
            window.render_frame(cx);
            assert!(window.find("support-empty").label().unwrap().contains("dev server"));
            window.click(("rb", 3usize), cx);
            window.render_frame(cx);
            assert!(window.find("support-placeholder").label().unwrap().starts_with("Trace."));
            window.click(("rb", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find("hl-asks").label(), Some("Messages: 0"));
        })
        .unwrap();
        // t1's folder is a Windows path that is not here.
        handle
            .update(cx, |c, _, _| {
                c.open_thread("t1".into());
                c.toggle_panel(Panel::Changes);
            })
            .unwrap();
        // A panel starts reading when it is drawn.
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx)).unwrap();
        settle(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("support-error").label(), Some("This stream's folder is not on this machine."));
            assert!(window.try_find("support-refresh").is_some(), "with a way to try again");
        })
        .unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A real repository: a committed file changed and a new one, read as Changes, Git and Files.
    #[test]
    fn a_real_repository_reads_through() {
        let dir = repo("read");
        let c = read_changes(&dir).unwrap();
        assert_eq!(c.files, [FileStat { path: "src/a.rs".into(), added: Some(1), removed: Some(1) }]);
        assert!(c.diff.iter().any(|l| l.kind == DiffKind::Add && l.text == "three"));
        let g = read_git(&dir).unwrap();
        assert_eq!(g.unstaged, [('M', "src/a.rs".into()), ('?', "notes.md".into())]);
        assert_eq!(g.commits.len(), 1);
        assert_eq!(g.commits[0].1, "First");
        let mut open = std::collections::BTreeSet::new();
        let top = read_files(&dir, &open).unwrap();
        assert_eq!(top.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["src", "notes.md"], "folders first, no .git");
        assert_eq!((top[0].mark, top[1].mark), (Some('•'), Some('?')), "a folder with a change is marked");
        open.insert(dir.join("src"));
        let all = read_files(&dir, &open).unwrap();
        assert_eq!((all[1].name.as_str(), all[1].depth, all[1].mark), ("a.rs", 1, Some('M')));
        assert_eq!(read_git(&std::env::temp_dir()).map(|_| ()), Err("This folder is not a git repository.".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
