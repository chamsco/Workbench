//! Command receipts across a real crash: a child process begins a command and
//! is killed (`abort`) before it records a result. The parent reopens the
//! database and must see exactly what a crash leaves behind.

use backspace_core::receipts::{Begin, Receipts};

const CHILD_ENV: &str = "RECEIPTS_CRASH_DB";

/// Runs only as the child: records one finished and one unfinished command,
/// then dies without unwinding or closing the database.
#[test]
fn child_that_crashes() {
    let Ok(db) = std::env::var(CHILD_ENV) else { return };
    let r = Receipts::open(std::path::Path::new(&db)).unwrap();
    assert_eq!(r.begin("merge:a@1->run", "merge").unwrap(), Begin::New);
    r.done("merge:a@1->run", "merged").unwrap();
    assert_eq!(r.begin("merge:b@2->run", "merge").unwrap(), Begin::New);
    std::process::abort();
}

#[test]
fn a_killed_process_leaves_done_commands_done_and_the_rest_cut_off() {
    let dir = std::env::temp_dir().join(format!("bs-receipts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("receipts.db");

    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["child_that_crashes", "--exact", "--nocapture"])
        .env(CHILD_ENV, &db)
        .status()
        .unwrap();
    assert!(!status.success(), "the child must have crashed, not exited cleanly");

    let r = Receipts::open(&db).unwrap();
    // Finished before the crash: never repeated.
    assert_eq!(r.begin("merge:a@1->run", "merge").unwrap(), Begin::Done("merged".into()));
    // Started and cut off: reported as such, counted as a second attempt.
    let open = r.in_flight().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].key, "merge:b@2->run");
    assert_eq!(r.begin("merge:b@2->run", "merge").unwrap(), Begin::CutOff { attempts: 2 });
    // A new tip on the same branch is a different command.
    assert_eq!(r.begin("merge:a@3->run", "merge").unwrap(), Begin::New);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_abandoned_command_can_run_again() {
    let dir = std::env::temp_dir().join(format!("bs-receipts-ab-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let r = Receipts::open(&dir.join("r.db")).unwrap();
    assert_eq!(r.begin("k", "merge").unwrap(), Begin::New);
    r.abandon("k").unwrap();
    assert_eq!(r.begin("k", "merge").unwrap(), Begin::New, "a conflicted merge changed nothing, so it is not 'cut off'");
    let _ = std::fs::remove_dir_all(&dir);
}
