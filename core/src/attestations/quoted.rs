//! Quoting in a directive's values: a column name or a field's key may be written in double
//! or single quotes to hold what would otherwise end it, such as a space, a colon or a dot.

const QUOTES: [char; 2] = ['"', '\''];

/// Where in `text` the first character that `stops` holds for is, outside quotes; None when
/// there is none. A quote with no closing match is an ordinary character.
pub fn first_outside_quotes(text: &str, stops: impl Fn(char) -> bool) -> Option<usize> {
    let mut at = 0;
    while let Some(c) = text[at..].chars().next() {
        if stops(c) {
            return Some(at);
        }
        let closing = if QUOTES.contains(&c) { text[at + 1..].find(c) } else { None };
        at += closing.map_or(c.len_utf8(), |close| close + 2);
    }
    None
}

/// `text` without the quotes around it, when it is one quoted run; None when it is not.
pub fn unquoted(text: &str) -> Option<&str> {
    QUOTES.iter().find_map(|&quote| {
        let inner = text.strip_prefix(quote)?.strip_suffix(quote)?;
        (!inner.contains(quote)).then_some(inner)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_hide_what_would_stop_a_value() {
        let space = |c: char| c == ' ';
        assert_eq!(first_outside_quotes("a b", space), Some(1));
        assert_eq!(first_outside_quotes("\"a b\" c", space), Some(5));
        assert_eq!(first_outside_quotes("x.'a b'.y z", space), Some(9));
        assert_eq!(first_outside_quotes("\"a b", space), Some(2));
        assert_eq!(first_outside_quotes("ab", space), None);
    }

    #[test]
    fn only_one_quoted_run_is_unquoted() {
        assert_eq!(unquoted("\"route path\""), Some("route path"));
        assert_eq!(unquoted("'a.b'"), Some("a.b"));
        assert_eq!(unquoted("\"a\".\"b\""), None);
        assert_eq!(unquoted("a"), None);
        assert_eq!(unquoted("\""), None);
    }
}
