//! Orchestration: from a directory to a report (`src/project/`, with `src/index/`).
//!
//! `state` loads and plans; `check` runs a check through the adapter and answers the
//! parsing-only queries (`index`, `where`, `uncovered`). Later slices add `impact`, `audit`,
//! `inventory` and `check --base` here.

pub mod check;
pub mod launches;
pub mod plan;
pub mod state;
