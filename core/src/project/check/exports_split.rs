//! A surface's exports split three ways: attested by a page, left to attest, and not meant
//! to be documented. A symbol the config says is not meant to be documented that a page
//! attests anyway is attested: the config says what need not be written, not what must not.

use std::collections::HashSet;

use crate::api::errors::Result;
use crate::config::matcher;

/// What became of each name a surface exports, in the order it exports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExportsSplit {
    pub attested: usize,
    /// Neither attested nor matched by an `undocumented` glob: what `uncovered` lists.
    pub unattested: Vec<String>,
    /// Not attested, and matched by an `undocumented` glob.
    pub undocumented: Vec<String>,
}

/// Split `names` by whether `attested` holds each, then, of the rest, whether one of the
/// `undocumented` globs matches it.
pub(super) fn split_exports(
    names: Vec<String>,
    attested: Option<&HashSet<&str>>,
    undocumented: &[String],
) -> Result<ExportsSplit> {
    let skipped = matcher(undocumented)?;
    let total = names.len();
    let (unattested, undocumented): (Vec<String>, Vec<String>) = names
        .into_iter()
        .filter(|name| !attested.is_some_and(|attested| attested.contains(name.as_str())))
        .partition(|name| !skipped.is_match(name));
    Ok(ExportsSplit { attested: total - unattested.len() - undocumented.len(), unattested, undocumented })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(names: &[&str], attested: &[&str], undocumented: &[&str]) -> ExportsSplit {
        let names = names.iter().map(|n| n.to_string()).collect();
        let attested: HashSet<&str> = attested.iter().copied().collect();
        let undocumented: Vec<String> = undocumented.iter().map(|g| g.to_string()).collect();
        split_exports(names, Some(&attested), &undocumented).unwrap()
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn a_name_is_attested_left_to_attest_or_not_meant_to_be_documented() {
        let names = ["Quote", "ProgramMain", "ProgramFlags", "internalCache", "rate"];
        assert_eq!(
            split(&names, &["Quote", "ProgramMain"], &["Program*", "internal*"]),
            ExportsSplit {
                attested: 2,
                unattested: strings(&["rate"]),
                undocumented: strings(&["ProgramFlags", "internalCache"]),
            }
        );
        assert_eq!(
            split(&names, &[], &[]),
            ExportsSplit { attested: 0, unattested: strings(&names), undocumented: vec![] }
        );
        let none = split_exports(strings(&["a"]), None, &strings(&["a"])).unwrap();
        assert_eq!(none.undocumented, ["a"]);
    }
}
