//! What one Flow frame costs on this machine: `cargo run --release -p cauldron-chrome --example flow_bench`.
use cauldron_chrome::flow;
use std::time::Instant;

fn main() {
    // The new-stream card at 1280x800 and at 2560x1600 (both in window points), and the frost.
    for (name, ww, wh, (x, y, w, h)) in [("card in 1280x800", 1280.0, 800.0, (250.0, 12.0, 720u32, 776u32)), ("card in 2560x1600", 2560.0, 1600.0, (250.0, 12.0, 2000, 1576))] {
        let n = 60;
        let t0 = Instant::now();
        for i in 0..n {
            std::hint::black_box(flow::cells(ww, wh, x, y, w, h, i as f32 / 15.0, true));
        }
        println!("{name}: {:.2} ms per frame ({} by {} cells)", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, w.div_ceil(flow::CELL), h.div_ceil(flow::CELL));
    }
    let t0 = Instant::now();
    for i in 0..60 {
        std::hint::black_box(flow::frost(i as f32 / 2.0, true));
    }
    println!("frost 100x60: {:.2} ms", t0.elapsed().as_secs_f64() * 1000.0 / 60.0);
}
