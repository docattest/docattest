//! The checks of the suite, one file for each area, and the order they run in. The first
//! greeting check keeps the greeting the later checks compare with, so it runs first.

mod answer;
mod capabilities;
mod greeting;
mod running;
mod session;
mod text;

use super::kit::Kit;
use super::suite::Check;

/// Every check that applies to `kit`, in the order they run; the cases table only when the
/// kit has one.
pub fn checks(kit: &Kit) -> Vec<Check> {
    let cases = kit.cases.as_ref().map(|_| running::CASES);
    [&greeting::CHECKS[..], &text::CHECKS, &session::CHECKS, &capabilities::CHECKS]
        .concat()
        .into_iter()
        .chain(cases)
        .chain(running::CHECKS)
        .collect()
}
