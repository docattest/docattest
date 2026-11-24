//! What installing (or previewing) one `init` integration did: `--agents`, `--hooks`,
//! `--git-hook` or `--skill`.

use serde_json::{Value, json};

/// What an integration's file needs, or needed: the same in a dry run as in a real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Add,
    Update,
    Replace,
    Unchanged,
}

impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Action::Add => "add",
            Action::Update => "update",
            Action::Replace => "replace",
            Action::Unchanged => "unchanged",
        }
    }
}

/// The file an integration writes, and what it does to it.
pub struct Integration {
    pub file: String,
    pub action: Action,
}

impl Integration {
    /// The JSON form `init --json` prints under `integrations`.
    pub fn to_json(&self) -> Value {
        json!({ "file": self.file, "action": self.action.as_str() })
    }
}
