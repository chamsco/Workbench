//! Visual options: an agent shows the user a few variants of a design, a
//! layout or a change — as screenshots it wrote, or as live pages — and the
//! user picks one. The pick comes back to the agent as its tool's result.
//!
//! A set lives on the project state, so it reaches the desktop shell in the
//! same snapshot as the board and the canvas ops. `show_options` in
//! `harness.rs` creates one and waits; the shell's Options canvas draws it.
//!
//! The `picked` field says where the user got to: `None` means they have not
//! decided yet, `Some("")` means they dismissed the set, and `Some(id)` is
//! the variant they chose.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::project::ProjectState;

/// How many variants one set may hold.
pub const MAX_VARIANTS: usize = 6;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Variant {
    /// Short id within the set ("now", "a", "b"). Filled in when omitted.
    #[serde(default)]
    pub id: String,
    /// What the user reads under the thumbnail ("A: Compact").
    pub label: String,
    /// One line on what this variant does.
    #[serde(default)]
    pub note: String,
    /// A screenshot the agent wrote: a path in the project, or a data: URL.
    #[serde(default)]
    pub image: Option<String>,
    /// Or a live page to show in the card.
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OptionSet {
    pub id: String,
    pub title: String,
    /// The agent's explanation of the set as a whole.
    #[serde(default)]
    pub note: String,
    pub variants: Vec<Variant>,
    /// The user's answer: None undecided, Some("") dismissed, Some(id) chosen.
    #[serde(default)]
    pub picked: Option<String>,
    pub at: u64,
    /// The agent that asked.
    pub by: String,
}

/// Add a set to the project. Variants with no id get "a", "b", …; a variant
/// with neither an image nor a url is refused.
pub fn add(
    s: &mut ProjectState,
    by: &str,
    title: &str,
    note: &str,
    mut variants: Vec<Variant>,
) -> Result<OptionSet> {
    let title = title.trim();
    if title.is_empty() {
        bail!("an options set needs a title");
    }
    if variants.len() < 2 {
        bail!("show at least two variants to choose between");
    }
    if variants.len() > MAX_VARIANTS {
        bail!("at most {MAX_VARIANTS} variants at a time");
    }
    for (i, v) in variants.iter_mut().enumerate() {
        if v.label.trim().is_empty() {
            bail!("every variant needs a label");
        }
        let has = v.image.as_deref().is_some_and(|x| !x.trim().is_empty())
            || v.url.as_deref().is_some_and(|x| !x.trim().is_empty());
        if !has {
            bail!("variant `{}` needs an image or a url", v.label);
        }
        if v.id.trim().is_empty() {
            v.id = ((b'a' + i as u8) as char).to_string();
        }
    }
    let mut seen = std::collections::HashSet::new();
    for v in &variants {
        if !seen.insert(v.id.clone()) {
            bail!("two variants share the id `{}`", v.id);
        }
    }
    let id = format!("opt{}", s.options.len() + 1);
    let set = OptionSet {
        id,
        title: title.to_string(),
        note: note.trim().to_string(),
        variants,
        picked: None,
        at: crate::chat::now_ms(),
        by: by.to_string(),
    };
    s.options.push(set.clone());
    Ok(set)
}

pub fn get<'a>(s: &'a ProjectState, id: &str) -> Option<&'a OptionSet> {
    s.options.iter().find(|o| o.id == id)
}

/// Record the user's answer. `variant` None means they dismissed the set.
pub fn pick(s: &mut ProjectState, id: &str, variant: Option<&str>) -> Result<OptionSet> {
    let set = s
        .options
        .iter_mut()
        .find(|o| o.id == id)
        .ok_or_else(|| anyhow::anyhow!("no options set `{id}`"))?;
    set.picked = match variant {
        Some(v) if set.variants.iter().any(|x| x.id == v) => Some(v.to_string()),
        Some(v) => bail!("no variant `{v}` in `{id}`"),
        None => Some(String::new()),
    };
    Ok(set.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(label: &str) -> Variant {
        Variant {
            id: String::new(),
            label: label.into(),
            note: String::new(),
            image: Some(format!("{label}.png")),
            url: None,
        }
    }

    #[test]
    fn add_names_variants_and_pick_records_the_choice() {
        let mut s = ProjectState::empty("p");
        let set = add(&mut s, "main", "  Layouts  ", "", vec![variant("Now"), variant("Compact")]).unwrap();
        assert_eq!(set.id, "opt1");
        assert_eq!(set.title, "Layouts");
        assert_eq!(set.variants[0].id, "a");
        assert_eq!(set.variants[1].id, "b");
        assert_eq!(set.picked, None);
        assert_eq!(get(&s, "opt1").unwrap().variants.len(), 2);

        assert!(pick(&mut s, "opt1", Some("b")).is_ok());
        assert_eq!(get(&s, "opt1").unwrap().picked.as_deref(), Some("b"));
        // Dismissing records the empty string, so the shell can tell it apart.
        pick(&mut s, "opt1", None).unwrap();
        assert_eq!(get(&s, "opt1").unwrap().picked.as_deref(), Some(""));
        assert!(pick(&mut s, "opt1", Some("zz")).is_err());
        assert!(pick(&mut s, "nope", Some("a")).is_err());
    }

    #[test]
    fn bad_sets_are_refused() {
        let mut s = ProjectState::empty("p");
        assert!(add(&mut s, "main", "", "", vec![variant("a"), variant("b")]).is_err());
        assert!(add(&mut s, "main", "t", "", vec![variant("a")]).is_err());
        let mut no_img = variant("a");
        no_img.image = None;
        assert!(add(&mut s, "main", "t", "", vec![no_img, variant("b")]).is_err());
        let mut dup = variant("b");
        dup.id = "a".into();
        assert!(add(&mut s, "main", "t", "", vec![variant("a"), dup]).is_err());
        assert!(s.options.is_empty());
    }
}
