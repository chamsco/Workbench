//! M2 acceptance: the eval can be run from the command line on a file of
//! labeled traces, and a bad file is an error rather than a report.

use std::process::Command;

fn run(file: &std::path::Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_backspace-cli"))
        .arg("eval")
        .arg(file)
        .args(extra)
        .current_dir(std::env::temp_dir())
        .env_remove("BACKSPACE_CONFIG")
        .output()
        .unwrap()
}

fn write(tag: &str, body: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("bs-eval-cli-{tag}-{}.jsonl", std::process::id()));
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn it_prints_the_report_for_labeled_traces() {
    let f = write(
        "ok",
        concat!(
            "{\"id\":\"a\",\"text\":\"fix a typo in the readme\",\"label\":\"one_shot\"}\n",
            "{\"id\":\"b\",\"text\":\"research the options and write a comparison report, no code changes\",\"label\":\"research\"}\n",
        ),
    );
    let out = run(&f, &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("## Accuracy per type") && stdout.contains("2 traces"), "{stdout}");
    let _ = std::fs::remove_file(f);
}

#[test]
fn the_threshold_can_be_given() {
    let f = write("thr", "{\"id\":\"a\",\"text\":\"fix a typo in the readme\",\"label\":\"one_shot\"}\n");
    let out = run(&f, &["--threshold", "0.99"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("0.99"), "{stdout}");
    let _ = std::fs::remove_file(f);
}

#[test]
fn unlabeled_traces_are_an_error() {
    let f = write("bad", "{\"id\":\"a\",\"text\":\"fix a typo\"}\n");
    let out = run(&f, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("line 1"));
    let _ = std::fs::remove_file(f);
}

#[test]
fn a_missing_file_is_an_error() {
    let out = run(std::path::Path::new("/nonexistent/traces.jsonl"), &[]);
    assert!(!out.status.success());
}
