//! The router eval PLAN §10 requires before a decision model ships: accuracy
//! per task type, calibration, cost against always-frontier, and the
//! escalation rate below the confidence threshold, over labeled traces.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::decision::{task_distribution, DecisionModel, TaskType, Triage};
use crate::ladder::{cheapest_on, planner_start, ModelClass};

/// One labeled task. Token counts are optional; without them cost is not
/// computed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Trace {
    pub id: String,
    pub text: String,
    pub label: TaskType,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
}

/// One JSON object per line. A line without a label, or with a label that is
/// not a task type, is an error naming the line.
pub fn parse_traces(jsonl: &str) -> Result<Vec<Trace>> {
    let mut out = vec![];
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let t: Trace = serde_json::from_str(line).map_err(|e| anyhow!("line {}: {e}", i + 1))?;
        out.push(t);
    }
    Ok(out)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeStats {
    pub n: usize,
    pub correct: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bin {
    pub lo: f64,
    pub hi: f64,
    pub n: usize,
    pub mean_confidence: f64,
    pub accuracy: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CostCompare {
    pub priced: usize,
    pub routed_usd: f64,
    pub always_frontier_usd: f64,
}

#[derive(Clone, Debug)]
pub struct EvalReport {
    pub decider: String,
    pub threshold: f64,
    pub n: usize,
    pub correct: usize,
    pub per_type: BTreeMap<TaskType, TypeStats>,
    pub bins: Vec<Bin>,
    pub ece: f64,
    pub brier: f64,
    pub below_threshold: usize,
    pub wrong_below_threshold: usize,
    pub confident_wrong: usize,
    pub cost: Option<CostCompare>,
}

pub fn evaluate(model: &dyn DecisionModel, traces: &[Trace], threshold: f64, cfg: &Config) -> Result<EvalReport> {
    if traces.is_empty() {
        bail!("no traces to evaluate");
    }
    let api_models: Vec<_> = cfg.models.iter().filter(|m| cfg.cli_of(&m.id).is_none()).collect();
    let frontier = cheapest_on(&api_models, ModelClass::Frontier);

    let mut per_type: BTreeMap<TaskType, TypeStats> = TaskType::ALL.into_iter().map(|t| (t, TypeStats::default())).collect();
    // (n, sum of confidence, correct) per equal-width confidence bin.
    let mut raw = [(0usize, 0.0f64, 0usize); 10];
    let (mut correct, mut brier) = (0usize, 0.0f64);
    let (mut below, mut wrong_below, mut confident_wrong) = (0, 0, 0);
    let (mut priced, mut routed_usd, mut always_usd) = (0usize, 0.0f64, 0.0f64);

    for t in traces {
        let dist = task_distribution(model, &t.text).with_context(|| format!("trace `{}`", t.id))?;
        let triage = Triage::from_distribution(&dist, threshold);
        let (best, conf) = (triage.best(), triage.p());
        let right = best == t.label;

        let st = per_type.get_mut(&t.label).expect("all types present");
        st.n += 1;
        st.correct += right as usize;
        correct += right as usize;

        let bin = ((conf * 10.0).floor() as usize).min(9);
        raw[bin].0 += 1;
        raw[bin].1 += conf;
        raw[bin].2 += right as usize;

        brier += dist.iter().map(|&(k, p)| (p - (k == t.label) as u8 as f64).powi(2)).sum::<f64>();

        if conf < threshold {
            below += 1;
            wrong_below += !right as usize;
        } else {
            confident_wrong += !right as usize;
        }

        if let (Some(i), Some(o), Some(front)) = (t.input_tokens, t.output_tokens, frontier) {
            if let Some(start) = cheapest_on(&api_models, planner_start(Some(&triage))) {
                priced += 1;
                routed_usd += start.cost(i, o);
                always_usd += front.cost(i, o);
            }
        }
    }

    let n = traces.len();
    let bins: Vec<Bin> = raw
        .iter()
        .enumerate()
        .map(|(i, &(bn, sum, ok))| Bin {
            lo: i as f64 / 10.0,
            hi: (i + 1) as f64 / 10.0,
            n: bn,
            mean_confidence: if bn > 0 { sum / bn as f64 } else { 0.0 },
            accuracy: if bn > 0 { ok as f64 / bn as f64 } else { 0.0 },
        })
        .collect();
    let ece = bins.iter().filter(|b| b.n > 0).map(|b| b.n as f64 / n as f64 * (b.accuracy - b.mean_confidence).abs()).sum();

    Ok(EvalReport {
        decider: model.name().to_string(),
        threshold,
        n,
        correct,
        per_type,
        bins,
        ece,
        brier: brier / n as f64,
        below_threshold: below,
        wrong_below_threshold: wrong_below,
        confident_wrong,
        cost: (priced > 0).then_some(CostCompare { priced, routed_usd, always_frontier_usd: always_usd }),
    })
}

fn pct(num: usize, den: usize) -> String {
    if den == 0 {
        "n/a".into()
    } else {
        format!("{:.1}%", 100.0 * num as f64 / den as f64)
    }
}

impl EvalReport {
    pub fn to_markdown(&self) -> String {
        let mut m = String::new();
        let _ = writeln!(m, "# Router eval\n");
        let _ = writeln!(m, "Decision model: `{}` · {} traces · confidence threshold {}\n", self.decider, self.n, self.threshold);

        let _ = writeln!(m, "## Accuracy per type\n");
        let _ = writeln!(m, "| type | n | correct | accuracy |\n|---|---|---|---|");
        for (t, s) in &self.per_type {
            let _ = writeln!(m, "| {} | {} | {}/{} | {} |", t.as_str(), s.n, s.correct, s.n, pct(s.correct, s.n));
        }
        let _ = writeln!(m, "| all | {} | {}/{} | {} |\n", self.n, self.correct, self.n, pct(self.correct, self.n));

        let _ = writeln!(m, "## Calibration\n");
        let _ = writeln!(m, "| confidence | n | mean confidence | accuracy |\n|---|---|---|---|");
        for b in self.bins.iter().filter(|b| b.n > 0) {
            let _ = writeln!(m, "| {:.1}–{:.1} | {} | {:.3} | {:.3} |", b.lo, b.hi, b.n, b.mean_confidence, b.accuracy);
        }
        let _ = writeln!(m, "\nExpected calibration error: {:.3} · Brier score: {:.3}\n", self.ece, self.brier);

        let _ = writeln!(m, "## Cost versus always-frontier\n");
        match &self.cost {
            Some(c) => {
                let saving = if c.always_frontier_usd > 0.0 { 100.0 * (1.0 - c.routed_usd / c.always_frontier_usd) } else { 0.0 };
                let _ = writeln!(
                    m,
                    "Over {} priced traces: routed ${:.4}, always-frontier ${:.4} (cheapest frontier-class model), a saving of {:.1}%. Routed = each trace starts on the cheapest model of the class its predicted type implies; unsure ones start mid. Asking the user is not priced, and neither are retries or escalations.\n",
                    c.priced, c.routed_usd, c.always_frontier_usd, saving
                );
            }
            None => {
                let _ = writeln!(m, "Cost is not computed: the traces carry no `input_tokens` and `output_tokens`, and none are guessed.\n");
            }
        }

        let _ = writeln!(m, "## Escalation below the threshold\n");
        let _ = writeln!(
            m,
            "Below the threshold (top probability under {}): {} of {} ({}). These are asked about, not acted on; {} of them were wrong, the mistakes asking catches.",
            self.threshold, self.below_threshold, self.n, pct(self.below_threshold, self.n), self.wrong_below_threshold
        );
        let confident = self.n - self.below_threshold;
        let _ = writeln!(
            m,
            "Confident and wrong: {} of {} confident answers ({}), the mistakes asking does not catch.",
            self.confident_wrong, confident, pct(self.confident_wrong, confident)
        );
        m
    }
}
