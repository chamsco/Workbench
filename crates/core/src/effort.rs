use serde::{Deserialize, Serialize};

/// Canonical effort ladder. Jev scores a task onto this ladder; each model
/// then clamps it to the levels its provider actually accepts.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    /// `max` on the wire, plus the largest turn budget. Reserved for work where
    /// a wrong answer costs more than the tokens.
    Ultra,
}

impl Effort {
    pub const LADDER: [Effort; 6] = [
        Effort::Low,
        Effort::Medium,
        Effort::High,
        Effort::Xhigh,
        Effort::Max,
        Effort::Ultra,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Effort {
        Self::LADDER[i.min(Self::LADDER.len() - 1)]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
            Effort::Ultra => "ultra",
        }
    }

    /// Agent-loop turn budget. Cheap tasks get cut off early by design.
    pub fn max_turns(self) -> usize {
        match self {
            Effort::Low => 8,
            Effort::Medium => 16,
            Effort::High => 30,
            Effort::Xhigh => 50,
            Effort::Max => 80,
            Effort::Ultra => 120,
        }
    }

    /// Rubric Jev scores against, lowest first. Index == ladder position.
    pub fn rubric() -> Vec<&'static str> {
        vec![
            "low: trivial or mechanical work; lookups, renames, formatting, boilerplate, short factual answers",
            "medium: ordinary, well-specified coding or writing with a clear path; a single file or component",
            "high: multi-step work needing judgment; several files, integration between parts, debugging",
            "xhigh: long-horizon agentic work; architecture, tricky debugging across a codebase, many interacting parts",
            "max: hard reasoning where correctness matters far more than cost; security, concurrency, novel algorithms",
            "ultra: the hardest problems; open-ended research-grade reasoning, planning an entire system from an ambiguous goal",
        ]
    }

    /// Clamp to the nearest level a model supports, rounding up (a model that
    /// only knows low|high gets high for medium). Empty = model has no effort knob.
    pub fn clamp_to(self, supported: &[Effort]) -> Option<Effort> {
        if supported.is_empty() {
            return None;
        }
        let mut sorted = supported.to_vec();
        sorted.sort();
        sorted
            .iter()
            .copied()
            .find(|e| *e >= self)
            .or_else(|| sorted.last().copied())
    }
}

impl std::fmt::Display for Effort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_up_then_down() {
        let s = [Effort::Low, Effort::High, Effort::Max];
        assert_eq!(Effort::Medium.clamp_to(&s), Some(Effort::High));
        assert_eq!(Effort::Ultra.clamp_to(&s), Some(Effort::Max));
        assert_eq!(Effort::Low.clamp_to(&[]), None);
    }
}
