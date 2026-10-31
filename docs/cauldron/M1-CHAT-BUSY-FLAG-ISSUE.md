# Chat reply can leave a thread busy

A separate issue found while hardening M1. **Not fixed, not part of M1.** It is in `crates/core/src/chat.rs`, which M1 does not otherwise touch. The fix below needs the owner's approval.

## What happens

`Chats::spawn_reply` starts the reply task and then records the task's abort handle in `running`, the table `is_busy` reads:

```rust
let task = self.rt.spawn(async move {
    let res = me.run(...).await;
    ...
    me.running.lock().unwrap().remove(&tid);   // the task removes its own entry
    let spec = ...;
    me.finish(&tid, &mid, res);                // then the message is marked Done / Error
    if spec { me.file_spec(&tid, &mid); }      // then the spec verdict is attached
    ...
});
self.running.lock().unwrap().insert(id.to_string(), task.abort_handle());   // the parent inserts it afterwards
```

Two ordering problems, one consequence each:

1. **Insert after spawn (can leave a thread busy for good).** The runtime is multi-threaded, so the task can run to its end before the parent has inserted the handle. The task's `remove` finds nothing; the parent then inserts a handle for a task that is already finished. `is_busy` is true from then on, `send` answers "wait for the reply, or stop it first", and nothing clears the entry except `stop`.
2. **Remove before finish (a window, not a leak).** `is_busy` goes false while the message is still `Streaming` and before `file_spec` attaches the verdict. A message sent in that window starts a second reply on the thread. This is what made `spec_gate` flaky; the test now waits for the verdict itself.

## Evidence

The probe is a scratch test, not in the repository (source below). It creates a thread on a route whose reply fails at once (`nope is not installed or not on PATH`), sends, and checks `is_busy` 500 ms later, then again two seconds after the last trial. 4 worker threads, nothing else running.

| Run | Code | Trials | Still busy after the reply ended |
|---|---|---|---|
| control, 1 run | unmodified (`577f819`) | 3,000 | 1 |
| control, 3 runs | unmodified | 3 x 3,000 | 2, 0, 0 |
| injected delay | `sleep(20ms)` between `spawn` and `insert`, to stand in for the parent being descheduled | 100 | 99, all still busy two seconds later |
| injected delay, proposed fix | the same delay, and the `running` lock held from before `spawn` until the handle is inserted | 100 | 0 |

So it is reproducible on unmodified code at roughly 3 in 12,000 trials; every stuck thread had an `Error` reply and stayed busy. The rate on a real machine depends on how long the parent is descheduled and on how fast a reply can fail, so it is not an estimate of how often a person meets it. The injected-delay rows show the cause, not the frequency.

## Proposed follow-up (needs owner approval)

1. In `spawn_reply`, take the `running` lock before `rt.spawn`, insert the handle, and release it. The task's `remove` then waits for the insert. Verified against the probe above (0 of 100 with the delay injected).
2. Move the task's `remove` to after `finish` and `file_spec`, so `is_busy` is true until the verdict is on the message. `spec_gate` already waits for the verdict on its own, so it does not depend on the old order.
3. Keep the probe as a regression test with the delay behind a test-only hook, so it is deterministic rather than a stress loop.

## Recovery today

`stop(id)` removes the entry and aborts the (already finished) task, which frees the thread. Restarting the app also clears it, since `running` is in memory.

## Probe source

```rust
//! Scratch probe, not part of the repository: does a reply that ends at once leave the thread "busy"?
use std::time::{Duration, Instant};

use backspace_core::chat::{Chats, Ctx, Route, RouteKind, Scope};
use backspace_core::prefs::Prefs;

#[test]
fn a_reply_that_fails_at_once_leaves_the_thread_free() {
    let n: usize = std::env::var("TRIALS").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let dir = std::env::temp_dir().join(format!("bs-busy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    let (tx, _rx) = async_channel::bounded(64);
    let chats = Chats::open(dir.join("chats"), rt.handle().clone(), tx);
    let route = Route { kind: RouteKind::Cli, provider: "nope".into(), model: None };
    let ctx = Ctx { prefs: Prefs::default(), memory: None, agents: None, plan: false };
    let mut stuck = 0;
    let mut ids = vec![];
    for _ in 0..n {
        let t = chats.create_in(route.clone(), Scope::default());
        chats.send(&t.id, "hi", vec![], None, ctx.clone()).unwrap();
        let start = Instant::now();
        while chats.is_busy(&t.id) && start.elapsed() < Duration::from_millis(500) {
            std::thread::sleep(Duration::from_millis(1));
        }
        if chats.is_busy(&t.id) {
            stuck += 1;
            ids.push(t.id.clone());
        }
    }
    // Still busy two seconds on: stuck, not slow. The thread's last message says how the reply ended.
    std::thread::sleep(Duration::from_secs(2));
    let still = ids.iter().filter(|i| chats.is_busy(i)).count();
    for i in &ids {
        let th = chats.thread(i).unwrap();
        let m = th.messages.last().unwrap();
        eprintln!("BUSY-PROBE thread {i}: busy={} last message {:?} error={:?}", chats.is_busy(i), m.status, m.error);
    }
    eprintln!("BUSY-PROBE still busy after 2 more seconds: {still} of {stuck}");
    eprintln!("BUSY-PROBE stuck {stuck} of {n}");
    assert_eq!(stuck, 0, "{stuck} of {n} threads are still busy after their reply ended");
}
```

Run in a scratch worktree with `TRIALS=3000 cargo test -p backspace-core --test busy_probe -- --nocapture`. The raw runs are in `M1-codex-busy-flag-evidence.txt`.
