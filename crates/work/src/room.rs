//! Rooms, their members and their messages.

use serde::{Deserialize, Serialize};

/// Who a member is.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Who {
    /// A person, by their account id.
    Human { user: String },
    /// An agent: a job (its system prompt) and the catalog model that runs
    /// it. `owner` is the person it works for, who pays for it and whose
    /// rights it never exceeds; `None` is a shared agent the workspace pays
    /// for out of its own allowance.
    Agent {
        job: String,
        model: String,
        #[serde(default)]
        owner: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub who: Who,
    /// The chip next to the name ("Analyst", "HR").
    #[serde(default)]
    pub tag: Option<String>,
}

impl Member {
    pub fn is_agent(&self) -> bool {
        matches!(self.who, Who::Agent { .. })
    }
}

/// What kind of conversation a room holds, from who is in it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// One person, nobody else: notes to self.
    Solo,
    /// People only.
    H2h,
    /// One person and their agents: Workbench's chat and groups.
    A2h,
    /// Agents only, no person in the room. Runs on a budget, see `turns`.
    A2a,
    /// Several people and agents.
    Mixed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Msg {
    pub id: String,
    /// The member who wrote it.
    pub author: String,
    pub text: String,
    pub at: u64,
    #[serde(default)]
    pub reply_to: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Room {
    pub id: String,
    pub title: String,
    /// What the room is for; agents read it before every turn.
    #[serde(default)]
    pub goal: Option<String>,
    /// In turn order: when a person speaks to a room of agents without
    /// naming one, they answer in this order.
    pub members: Vec<Member>,
    #[serde(default)]
    pub messages: Vec<Msg>,
}

impl Room {
    pub fn member(&self, id: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.id == id)
    }

    pub fn agents(&self) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(|m| m.is_agent())
    }

    pub fn humans(&self) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(|m| !m.is_agent())
    }

    pub fn mode(&self) -> Mode {
        match (self.humans().count(), self.agents().count()) {
            (0, _) => Mode::A2a,
            (1, 0) => Mode::Solo,
            (_, 0) => Mode::H2h,
            (1, _) => Mode::A2h,
            _ => Mode::Mixed,
        }
    }
}
