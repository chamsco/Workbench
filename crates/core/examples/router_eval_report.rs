//! §10 historical-prompt replay: the numbers the owner review needs, as JSON.
//!
//!   cargo run -p backspace-core --example router_eval_report -- <traces.jsonl> [--threshold N] [--no-length]
//!   cargo run -p backspace-core --example router_eval_report -- <controls.jsonl> --controls [--threshold N]
//!
//! It makes no network calls. `--no-length` runs the keyword baseline with only
//! its length rule removed. `--controls` takes rows that are not tasks (no
//! label needed) and reports how often the decider would act on them.

use backspace_core::config::{Config, DEFAULT_CONFIG};
use backspace_core::decision::{task_distribution, DecisionModel, KeywordDecider, KeywordDeciderNoLength, TaskType, Triage};
use backspace_core::eval::{evaluate, parse_traces, Trace};
use serde_json::{json, Value};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().ok_or_else(|| anyhow::anyhow!("usage: router_eval_report <file.jsonl> [--threshold N] [--no-length] [--controls]"))?;
    let threshold: f64 = match args.iter().position(|a| a == "--threshold") {
        Some(i) => args.get(i + 1).ok_or_else(|| anyhow::anyhow!("--threshold needs a value"))?.parse()?,
        None => backspace_core::decision::DEFAULT_TRIAGE_THRESHOLD,
    };
    let no_length = args.iter().any(|a| a == "--no-length");
    let controls = args.iter().any(|a| a == "--controls");
    let model: Box<dyn DecisionModel> = if no_length { Box::new(KeywordDeciderNoLength) } else { Box::new(KeywordDecider) };
    let src = std::fs::read_to_string(path)?;

    if controls {
        let mut rows = vec![];
        for l in src.lines().filter(|l| !l.trim().is_empty()) {
            let v: Value = serde_json::from_str(l)?;
            let text = v["text"].as_str().unwrap_or_default();
            let dist = task_distribution(model.as_ref(), text)?;
            let tri = Triage::from_distribution(&dist, threshold);
            rows.push(json!({"id": v["id"], "chars": text.chars().count(), "best": tri.best().as_str(), "p": tri.p(), "confident": matches!(tri, Triage::Confident { .. })}));
        }
        let confident = rows.iter().filter(|r| r["confident"] == true).count();
        println!("{}", serde_json::to_string_pretty(&json!({"decider": model.name(), "threshold": threshold, "n": rows.len(), "confident": confident, "asked": rows.len() - confident, "rows": rows}))?);
        return Ok(());
    }

    let traces: Vec<Trace> = parse_traces(&src)?;
    let cfg = Config::parse(DEFAULT_CONFIG)?;
    let rep = evaluate(model.as_ref(), &traces, threshold, &cfg)?;

    let types = TaskType::ALL;
    let idx = |t: TaskType| types.iter().position(|x| *x == t).unwrap();
    let mut confusion = vec![vec![0usize; 3]; 3]; // [true][predicted]
    let (mut conf_n, mut conf_ok, mut ask_n, mut ask_wrong) = (0usize, 0usize, 0usize, 0usize);
    let mut per = vec![];
    for t in &traces {
        let dist = task_distribution(model.as_ref(), &t.text)?;
        let tri = Triage::from_distribution(&dist, threshold);
        let (best, p) = (tri.best(), tri.p());
        confusion[idx(t.label)][idx(best)] += 1;
        let ok = best == t.label;
        let confident = matches!(tri, Triage::Confident { .. });
        if confident {
            conf_n += 1;
            conf_ok += ok as usize;
        } else {
            ask_n += 1;
            ask_wrong += !ok as usize;
        }
        per.push(json!({"id": t.id, "label": t.label.as_str(), "best": best.as_str(), "p": p, "confident": confident, "correct": ok, "chars": t.text.chars().count()}));
    }
    let n = traces.len();
    let per_type: Vec<Value> = rep.per_type.iter().map(|(t, s)| json!({"type": t.as_str(), "n": s.n, "correct": s.correct})).collect();
    let bins: Vec<Value> = rep.bins.iter().filter(|b| b.n > 0).map(|b| json!({"lo": b.lo, "hi": b.hi, "n": b.n, "mean_confidence": b.mean_confidence, "accuracy": b.accuracy})).collect();
    let out = json!({
        "decider": rep.decider, "threshold": threshold, "n": n, "correct": rep.correct,
        "per_type": per_type,
        "confusion_rows_true_cols_predicted": {"order": types.iter().map(|t| t.as_str()).collect::<Vec<_>>(), "matrix": confusion},
        "confident": conf_n, "confident_correct": conf_ok, "confident_wrong": conf_n - conf_ok,
        "coverage": conf_n as f64 / n as f64,
        "selective_accuracy": if conf_n > 0 { Some(conf_ok as f64 / conf_n as f64) } else { None },
        "asked": ask_n, "wrong_caught_by_asking": ask_wrong, "asked_but_right": ask_n - ask_wrong,
        "bins": bins, "ece": rep.ece, "brier": rep.brier,
        "evaluate_confident_wrong_check": rep.confident_wrong, "evaluate_below_threshold_check": rep.below_threshold,
        "cost": rep.cost.map(|_| "computed").unwrap_or("Not computed — insufficient measured token and execution data"),
        "per_trace": per,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
