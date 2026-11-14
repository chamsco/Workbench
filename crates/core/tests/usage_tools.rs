//! Per-tool usage from the logs the coding tools keep on this machine
//! (usage.rs `tool_usage_summary`). Only what the logs record: counts and
//! tokens for the last 24 hours and 7 days. Never a limit, a percentage, a
//! reset time or a cost. Every test reads a fixture under tests/fixtures
//! copied to a temp folder, never the real home directory.

use std::path::{Path, PathBuf};

use backspace_core::usage::{parse_ms, tool_usage_summary, ToolUsage, ToolWindow, Tokens};

fn now() -> u64 {
    parse_ms("2026-11-01T12:00:00Z").unwrap()
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A fresh home for one test. Copies are new files, so their modified time is
/// recent whatever the fixture's was.
fn home(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bs-tool-usage-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fixture_home(name: &str) -> PathBuf {
    let h = home(name);
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/usage_home"), &h);
    h
}

fn tokens(input: u64, output: u64, cache_read: u64, cache_write: u64) -> Tokens {
    Tokens { input, output, cache_read, cache_write }
}

fn window(sessions: u32, prompts: u32, tool_calls: u32, tokens: Tokens) -> ToolWindow {
    ToolWindow { sessions, prompts, tool_calls, tokens }
}

fn tool<'a>(v: &'a [ToolUsage], id: &str) -> &'a ToolUsage {
    v.iter().find(|t| t.tool == id).unwrap_or_else(|| panic!("no {id} in {v:?}"))
}

#[test]
fn claude_code_today_and_week_come_from_its_logs() {
    let h = fixture_home("claude");
    let v = tool_usage_summary(&h, now());
    let c = tool(&v, "claude-code");
    // Today: one prompt, one call, the streamed message counted once, plus the
    // subagent's own message. The same message copied into the subagent file adds nothing.
    assert_eq!(c.today, window(1, 1, 1, tokens(13, 7, 100, 7)));
    // The week adds the two-day-old prompt and message; the 17-day-old one is outside.
    assert_eq!(c.week, window(1, 2, 2, tokens(33, 17, 300, 7)));
    assert_eq!(c.source_path, h.join(".claude").join("projects").to_string_lossy());
    assert_eq!(c.as_of, now());
}

#[test]
fn codex_today_and_week_come_from_its_rollouts() {
    let h = fixture_home("codex");
    let v = tool_usage_summary(&h, now());
    let c = tool(&v, "codex");
    // Context the CLI injects is not a prompt; cached input is counted as cache reads.
    assert_eq!(c.today, window(1, 1, 1, tokens(60, 9, 40, 0)));
    assert_eq!(c.week, window(2, 2, 1, tokens(110, 14, 40, 0)));
    assert_eq!(c.source_path, h.join(".codex").join("sessions").to_string_lossy());
    assert_eq!(c.as_of, now());
}

#[test]
fn tools_are_listed_in_a_fixed_order() {
    let h = fixture_home("order");
    let v = tool_usage_summary(&h, now());
    assert_eq!(v.iter().map(|t| t.tool.as_str()).collect::<Vec<_>>(), vec!["claude-code", "codex"]);
}

#[test]
fn a_home_with_no_logs_has_no_tools() {
    let h = home("empty");
    assert!(tool_usage_summary(&h, now()).is_empty());
}

#[test]
fn a_tool_whose_folder_is_missing_is_left_out_not_zeroed() {
    let h = home("only-codex");
    std::fs::create_dir_all(h.join(".codex/sessions")).unwrap();
    let v = tool_usage_summary(&h, now());
    assert_eq!(v.len(), 1);
    // Its folder is there and holds nothing in the window: zeros are true.
    assert_eq!(tool(&v, "codex").today, ToolWindow::default());
    assert_eq!(tool(&v, "codex").week, ToolWindow::default());
}

#[test]
fn logs_older_than_the_week_count_for_neither_window() {
    let h = home("old");
    let dir = h.join(".claude/projects/-w-old");
    std::fs::create_dir_all(&dir).unwrap();
    let old = r#"{"type":"assistant","timestamp":"2026-10-01T00:00:00.000Z","message":{"id":"m9","model":"claude-x","usage":{"input_tokens":999,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"content":[]}}"#;
    std::fs::write(dir.join("old.jsonl"), old).unwrap();
    let v = tool_usage_summary(&h, now());
    assert_eq!(tool(&v, "claude-code").today, ToolWindow::default());
    assert_eq!(tool(&v, "claude-code").week, ToolWindow::default());
}

#[test]
fn opencode_reads_its_database_read_only() {
    let h = home("opencode");
    let dir = h.join(".local/share/opencode");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("opencode.db");
    let (two_hours, three_days) = (now() - 2 * 3_600_000, now() - 3 * 86_400_000);
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch(&format!(
        "CREATE TABLE session(id TEXT, directory TEXT, parent_id TEXT);
         CREATE TABLE message(id TEXT, session_id TEXT, time_created INT, data TEXT);
         CREATE TABLE part(id TEXT, session_id TEXT, time_created INT, data TEXT);
         INSERT INTO session VALUES('ses_abcdef123','/w/app',NULL);
         INSERT INTO message VALUES('m1','ses_abcdef123',{a},'{{\"role\":\"user\"}}');
         INSERT INTO message VALUES('m2','ses_abcdef123',{b},'{{\"role\":\"assistant\",\"modelID\":\"m-x\",\"tokens\":{{\"input\":10,\"output\":4,\"reasoning\":1,\"cache\":{{\"read\":50,\"write\":3}}}}}}');
         INSERT INTO message VALUES('m3','ses_abcdef123',{c},'{{\"role\":\"user\"}}');
         INSERT INTO message VALUES('m4','ses_abcdef123',{d},'{{\"role\":\"assistant\",\"modelID\":\"m-x\",\"tokens\":{{\"input\":20,\"output\":2,\"reasoning\":0,\"cache\":{{\"read\":0,\"write\":0}}}}}}');
         INSERT INTO part VALUES('p1','ses_abcdef123',{b},'{{\"type\":\"tool\",\"tool\":\"bash\",\"state\":{{\"status\":\"completed\",\"time\":{{\"start\":{b},\"end\":{b}}}}}}}');",
        a = two_hours - 1000,
        b = two_hours,
        c = three_days - 1000,
        d = three_days,
    ))
    .unwrap();
    drop(c);
    let v = tool_usage_summary(&h, now());
    let o = tool(&v, "opencode");
    assert_eq!(o.today, window(1, 1, 1, tokens(10, 5, 50, 3)));
    assert_eq!(o.week, window(1, 2, 1, tokens(30, 7, 50, 3)));
    assert_eq!(o.source_path, db.to_string_lossy());
}

#[test]
fn the_summary_holds_no_prompt_text_and_no_limit_cost_or_reset() {
    let h = fixture_home("shape");
    let json = serde_json::to_string(&tool_usage_summary(&h, now())).unwrap();
    for text in ["older prompt", "a prompt from today", "do it", "an older prompt", "Bash", "shell"] {
        assert!(!json.contains(text), "{text} leaked into {json}");
    }
    // Nothing a log does not record: no limit, percentage, reset or money.
    for word in ["limit", "percent", "reset", "cost", "price", "remaining", "left"] {
        assert!(!json.to_lowercase().contains(word), "{word} in {json}");
    }
}
