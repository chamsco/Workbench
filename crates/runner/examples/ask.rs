//! `cargo run -p backspace-runner --example ask -- claude "your question"`:
//! one turn on a CLI from PATH, printing every event.

use backspace_runner::{run, Request, Target, Turn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let provider = args.next().unwrap_or_else(|| "claude".into());
    let question = args.collect::<Vec<_>>().join(" ");
    let bin = std::env::var_os("PATH")
        .and_then(|p| std::env::split_paths(&p).map(|d| d.join(&provider)).find(|p| p.is_file()))
        .ok_or_else(|| anyhow::anyhow!("{provider} is not on PATH"))?;
    let req = Request::new(Target::Cli { provider, bin }, vec![Turn::user(question)]);
    run(&reqwest::Client::new(), &req, &mut |e| println!("{}", serde_json::to_string(&e).unwrap())).await
}
