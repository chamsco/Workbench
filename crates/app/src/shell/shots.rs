//! Screenshots of the shell through GPUI's headless Metal renderer (macOS only), for the build
//! reports: no window on the display and no screen-recording permission. With the `shots` feature:
//! `CAULDRON_SHOTS=<dir> BACKSPACE_DATA=<sample data> cargo run -p backspace --features shots` writes
//! `<state>-<theme>.png` at 1200x720 for each state and exits. It runs on the main thread, which
//! AppKit needs, so it is not a test.

use super::{AgentTab, Cauldron, Panel, Threads};
use backspace_core::chat::Chats;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{px, size, AppContext, HeadlessAppContext};
use std::path::PathBuf;
use std::sync::Arc;

/// The states each gate is checked in: New stream, a thread with the drawer, the drawer folded,
/// an agentic thread's tabs.
const STATES: [&str; 4] = ["new", "thread", "folded", "tabs"];

fn shot(dir: &PathBuf, data: &PathBuf, state: &str, theme: &str) {
    std::env::set_var("CAULDRON_THEME", theme);
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(crate::icons::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    cx.update(|cx| cx.set_reduce_motion(true));
    cx.update(|cx| cx.text_system().add_fonts(crate::ui::bundled_fonts())).unwrap();
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let (tx, _) = async_channel::bounded(1);
    let chats = Chats::open(data.clone(), rt.handle().clone(), tx);
    let ch = chats.clone();
    let handle = cx
        .open_window(size(px(1200.), px(720.)), move |_, cx| {
            cx.new(|_| {
                let mut c = Cauldron { threads: Threads::Ready(ch.list()), core: Some((rt, ch)), ..Cauldron::new() };
                if state == "folded" {
                    c.drawer = None;
                    c.drawer_last = Some(Panel::Highlights);
                }
                if state == "tabs" {
                    c.coordinator = Some("Cauldron · coordinator".into());
                    c.agents = ["Rampart", "Builder", "Reviewer"].iter().enumerate().map(|(i, n)| AgentTab { name: (*n).into(), dot: i }).collect();
                    c.active = 3;
                }
                c
            })
        })
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        if state == "thread" || state == "folded" {
            window.click(("thread", 0usize), cx);
        }
        window.render_frame(cx);
    })
    .unwrap();
    // A second frame once the transcript's list has measured its rows and its fade-in (on the
    // wall clock) has run.
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(400));
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx)).unwrap();
    let img = cx.capture_screenshot(handle.into()).expect("Metal rendering");
    img.save(dir.join(format!("{state}-{theme}.png"))).unwrap();
}

pub(crate) fn run(dir: PathBuf) {
    let data = PathBuf::from(std::env::var("BACKSPACE_DATA").expect("BACKSPACE_DATA=<sample data>"));
    std::fs::create_dir_all(&dir).unwrap();
    for theme in ["dark", "light"] {
        for state in STATES {
            shot(&dir, &data, state, theme);
        }
    }
}
