//! The journal's promise: a client that applies what `changes_since` returns
//! ends up with exactly the state the project has now, across a restart whose
//! restored state differs from where the journal ended, and it is told to
//! reload when it cannot be caught up.

use backspace_core::changes::{apply, Journal, Resume};
use serde_json::{json, Value};

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("bs-changes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn catch_up(held: &mut Value, j: &Journal, epoch: &str, after: u64) -> u64 {
    match j.changes_since(epoch, after).unwrap() {
        Resume::Changes(cs) => {
            for c in &cs {
                apply(held, &c.patch).unwrap();
            }
            cs.last().map_or(after, |c| c.seq)
        }
        Resume::Reset => panic!("expected to catch up, was told to reload"),
    }
}

#[test]
fn a_client_catches_up_exactly_through_a_restart_that_changed_the_state() {
    let db = dir("restart").join("changes.db");
    let s0 = json!({"name": "p", "agents": [1], "planner_session": null, "board": {"a": 1}});
    let j = Journal::open(&db, &s0).unwrap();
    let (epoch, _, _) = j.current();

    // The client holds state at seq 1.
    j.record(&json!({"name": "p", "agents": [1, 2], "planner_session": "s1", "board": {"a": 1}})).unwrap();
    let (_, seq1, mut held) = j.current();

    // Two more changes the client misses: a value back to null, a key removed.
    j.record(&json!({"name": "p", "agents": [1, 2], "planner_session": null, "board": {"a": 1, "b": 2}})).unwrap();
    j.record(&json!({"name": "p", "agents": [1, 2, 3], "planner_session": null, "board": {"b": 2}})).unwrap();
    drop(j);

    // The app restarts from a saved state that is older than the journal's end.
    let restored = json!({"name": "p", "agents": [1, 2], "planner_session": null, "board": {"b": 2}, "extra": [true]});
    let j = Journal::open(&db, &restored).unwrap();
    let (epoch_after, seq_now, now) = j.current();
    assert_eq!(epoch_after, epoch, "same journal, same epoch");
    assert_eq!(now, restored);

    catch_up(&mut held, &j, &epoch, seq1);
    assert_eq!(held, restored);
    assert!(seq_now > seq1);
}

#[test]
fn a_client_that_cannot_be_caught_up_is_told_to_reload() {
    let db = dir("reset").join("changes.db");
    let j = Journal::open(&db, &json!({"n": 0})).unwrap();
    let (epoch, _, _) = j.current();
    for n in 1..=1700 {
        j.record(&json!({"n": n})).unwrap();
    }
    let (_, seq, _) = j.current();

    assert_eq!(j.changes_since("another-epoch", seq).unwrap(), Resume::Reset);
    assert_eq!(j.changes_since(&epoch, seq + 5).unwrap(), Resume::Reset);
    // 1700 changes with a snapshot every 500 keep only the last ~1000+ before it:
    // a client still at seq 3 fell behind what is kept.
    assert_eq!(j.changes_since(&epoch, 3).unwrap(), Resume::Reset);
    // One that is up to date has nothing to apply.
    assert_eq!(j.changes_since(&epoch, seq).unwrap(), Resume::Changes(vec![]));
}
