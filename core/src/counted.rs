//! A count and the noun it counts, as the core's sentences write them.

use std::fmt::Display;

/// `count` followed by `noun`, which takes an `s` unless the count is exactly one: `1 page`,
/// `0 pages`, `2.5 days`. Every noun the core counts makes its plural that way.
pub(crate) fn counted<N: Display + PartialEq + From<u8>>(count: N, noun: &str) -> String {
    let plural = if count == N::from(1) { "" } else { "s" };
    format!("{count} {noun}{plural}")
}

/// The form of a word that agrees with `count`: `one` for exactly one, else `many`, as a
/// verb follows the noun [`counted`] writes (`1 item matches`, `2 items match`).
pub(crate) fn agreeing<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_is_singular_and_every_other_count_plural() {
        assert_eq!(counted(1_usize, "living page"), "1 living page");
        assert_eq!(counted(0_usize, "living page"), "0 living pages");
        assert_eq!(counted(2_usize, "symbol"), "2 symbols");
        assert_eq!(counted(1.0_f64, "day"), "1 day");
        assert_eq!(counted(0.5_f64, "day"), "0.5 days");
    }
}
