//! Platform services (PRD §3): the bus, the inbox, and the rule that the
//! platform depends on no app.

use cauldron_platform::events::{Bus, Event, Kind};
use cauldron_platform::notify::{Inbox, QuietHours};

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cauldron-platform-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ev(kind: Kind, source: &str, title: &str) -> Event {
    Event { app: "code".into(), source: source.into(), kind, title: title.into(), body: String::new(), at: 1 }
}

#[test]
fn every_subscriber_gets_each_event_and_a_dropped_one_is_forgotten() {
    let bus = Bus::default();
    let a = bus.subscribe();
    let b = bus.subscribe();
    assert_eq!(bus.publish(ev(Kind::Delivered, "fam-1", "Delivered")), 2);
    assert_eq!(a.try_recv().unwrap().title, "Delivered");
    assert_eq!(b.try_recv().unwrap().kind, Kind::Delivered);
    drop(b);
    assert_eq!(bus.publish(ev(Kind::Info, "fam-1", "x")), 1, "a dropped subscriber is not counted");
}

#[test]
fn the_inbox_keeps_notes_across_opens_and_counts_unread() {
    let data = tmp("inbox");
    let inbox = Inbox::open(&data);
    let n = inbox.push(ev(Kind::NeedsYou, "fam-1", "Approve the spec"), None, 12).unwrap().expect("a new note");
    assert!(n.alert && !n.read);
    inbox.push(ev(Kind::Delivered, "fam-2", "PR #52"), None, 12).unwrap().unwrap();
    let again = Inbox::open(&data);
    assert_eq!(again.list().len(), 2);
    assert_eq!(again.unread(), 2);
    again.mark_read(n.id).unwrap();
    assert_eq!(Inbox::open(&data).unread(), 1);
    again.mark_all_read().unwrap();
    assert_eq!(Inbox::open(&data).unread(), 0);
}

#[test]
fn the_same_unread_note_is_not_repeated() {
    let data = tmp("dedupe");
    let inbox = Inbox::open(&data);
    assert!(inbox.push(ev(Kind::NeedsYou, "fam-1", "Approve the spec"), None, 9).unwrap().is_some());
    assert!(inbox.push(ev(Kind::NeedsYou, "fam-1", "Approve the spec"), None, 9).unwrap().is_none(), "still unread: no second note");
    let id = inbox.list()[0].id;
    inbox.mark_read(id).unwrap();
    assert!(inbox.push(ev(Kind::NeedsYou, "fam-1", "Approve the spec"), None, 9).unwrap().is_some(), "read, then asked again: a new note");
}

#[test]
fn quiet_hours_keep_the_note_but_not_the_alert_and_wrap_past_midnight() {
    let q = QuietHours { from_hour: 22, to_hour: 7 };
    assert!(q.contains(23) && q.contains(0) && q.contains(6));
    assert!(!q.contains(7) && !q.contains(12) && !q.contains(21));
    let day = QuietHours { from_hour: 12, to_hour: 14 };
    assert!(day.contains(13) && !day.contains(14));
    let data = tmp("quiet");
    let n = Inbox::open(&data).push(ev(Kind::Failed, "fam-1", "Verify refused"), Some(q), 23).unwrap().unwrap();
    assert!(!n.alert, "kept, but no alert in quiet hours");
    assert_eq!(Inbox::open(&data).unread(), 1);
}

#[test]
fn the_platform_depends_on_no_app() {
    let toml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    assert!(!toml.contains("path ="), "the platform crate takes no workspace crate as a dependency:\n{toml}");
    for app in ["backspace-core", "backspace-cli", "backspace", "cauldron-decision"] {
        assert!(!toml.lines().any(|l| l.trim_start().starts_with(&format!("{app} ")) || l.trim_start().starts_with(&format!("{app}="))), "{app}");
    }
}
