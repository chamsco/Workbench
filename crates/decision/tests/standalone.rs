//! The engine works for a host that is not Cauldron (PRD §2.2 rule 3-4).

use cauldron_decision::ladder::{cheapest_on, class_of, within, ModelClass, Rung};

struct HostModel {
    id: &'static str,
    tier: u8,
    usd: f64,
}

impl Rung for HostModel {
    fn id(&self) -> &str {
        self.id
    }
    fn tier(&self) -> u8 {
        self.tier
    }
    fn class(&self) -> Option<ModelClass> {
        None
    }
    fn input_usd_per_mtok(&self) -> f64 {
        self.usd
    }
    fn output_usd_per_mtok(&self) -> f64 {
        self.usd
    }
}

#[test]
fn a_foreign_host_routes_through_the_ladder() {
    let local = HostModel { id: "tiny", tier: 0, usd: 0.0 };
    let mid = HostModel { id: "mid", tier: 2, usd: 1.0 };
    let top = HostModel { id: "top", tier: 4, usd: 15.0 };
    assert_eq!(class_of(&top), ModelClass::Frontier);
    let all = [&local, &mid, &top];
    let kept: Vec<_> = within(&all, ModelClass::Mid).iter().map(|m| m.id).collect();
    assert_eq!(kept, ["tiny", "mid"]);
    assert_eq!(cheapest_on(&all, ModelClass::Frontier).map(|m| m.id), Some("top"));
}

#[test]
fn the_engine_has_no_path_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    assert!(!manifest.contains("path ="), "cauldron-decision must stay independently buildable: {manifest}");
}
