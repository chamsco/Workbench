//! The cost ladder lives in the `cauldron-decision` crate (PRD §5); this is
//! Cauldron's adapter, so `crate::ladder::*` paths keep working.
pub use cauldron_decision::ladder::*;

use crate::config::ModelSpec;

impl Rung for ModelSpec {
    fn id(&self) -> &str {
        &self.id
    }
    fn tier(&self) -> u8 {
        self.tier
    }
    fn class(&self) -> Option<ModelClass> {
        self.class
    }
    fn input_usd_per_mtok(&self) -> f64 {
        self.input_usd_per_mtok
    }
    fn output_usd_per_mtok(&self) -> f64 {
        self.output_usd_per_mtok
    }
}

// Cauldron's callers (and its tests) use the ladder on `ModelSpec` only, so
// these keep the concrete signatures; they shadow the generic re-exports.

/// See [`cauldron_decision::ladder::class_of`].
pub fn class_of(spec: &ModelSpec) -> ModelClass {
    cauldron_decision::ladder::class_of(spec)
}

/// See [`cauldron_decision::ladder::within`].
pub fn within<'a>(candidates: &[&'a ModelSpec], ceiling: ModelClass) -> Vec<&'a ModelSpec> {
    cauldron_decision::ladder::within(candidates, ceiling)
}

/// See [`cauldron_decision::ladder::cheapest_on`].
pub fn cheapest_on<'a>(candidates: &[&'a ModelSpec], class: ModelClass) -> Option<&'a ModelSpec> {
    cauldron_decision::ladder::cheapest_on(candidates, class)
}
