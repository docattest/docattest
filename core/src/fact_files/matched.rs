//! What `matched("<path>", "<regex>")` gives: the text a regex matches in a file, decided here
//! once, so that every language's fact gets the same string from the same regex.

use fancy_regex::Regex;

/// The text of the first group of the first match of `pattern` in `text`, or the whole match
/// when the regex has no group; or why there is none, as a phrase. `^` and `$` match at the
/// start and end of each line, and the text is matched with each `\r\n` read as `\n`, so that
/// a file checked out with either line ending gives the same value.
pub fn matched_text(text: &str, pattern: &str) -> Result<String, String> {
    let invalid = |why: fancy_regex::Error| format!("\"{pattern}\" is not a valid regex: {why}");
    // Compiled alone first, so that a parsing error counts its position in the regex as
    // written. The flag is written into the regex, not set on a builder: the builder's
    // flag cannot be turned off inside the regex with `(?-m)`.
    Regex::new(pattern).map_err(invalid)?;
    let regex = Regex::new(&format!("(?m){pattern}")).map_err(invalid)?;
    let text = text.replace("\r\n", "\n");
    let found = regex.captures(&text).map_err(|why| format!("\"{pattern}\" cannot be matched: {why}"))?;
    let Some(captures) = found else {
        return Err(format!("\"{pattern}\" matches nothing"));
    };
    let group = if captures.len() > 1 { 1 } else { 0 };
    match captures.get(group) {
        Some(text) => Ok(text.as_str().to_string()),
        None => Err(format!("\"{pattern}\" matches without its first group")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCKERFILE: &str = "FROM node:22\nUSER 10001\nCMD [\"node\"]\n";

    #[test]
    fn the_value_is_the_first_group_of_the_first_match_or_the_whole_match() {
        assert_eq!(matched_text(DOCKERFILE, r"USER (\d+)"), Ok("10001".into()));
        assert_eq!(matched_text(DOCKERFILE, r"node:\d+"), Ok("node:22".into()));
        assert_eq!(matched_text(DOCKERFILE, r"(\w+) (\S+)"), Ok("FROM".into()));
        assert_eq!(matched_text("v1 v2", r"v(\d)"), Ok("1".into()));
        // Look-around is part of the regex flavour.
        assert_eq!(matched_text(DOCKERFILE, r"(?<=USER )\d+"), Ok("10001".into()));
    }

    #[test]
    fn anchors_are_lines_and_a_crlf_ending_is_a_line_ending() {
        assert_eq!(matched_text(DOCKERFILE, r"^USER (\d+)$"), Ok("10001".into()));
        assert_eq!(matched_text("USER 10001\r\nCMD x\r\n", r"^USER (.+)$"), Ok("10001".into()));
        assert_eq!(matched_text(DOCKERFILE, r"(?-m)^USER"), Err("\"(?-m)^USER\" matches nothing".into()));
        // A dot stops at the end of a line unless the regex says otherwise.
        assert_eq!(matched_text("a\nb", r"a.b"), Err("\"a.b\" matches nothing".into()));
        assert_eq!(matched_text("a\nb", r"(?s)a.b"), Ok("a\nb".into()));
    }

    #[test]
    fn no_match_an_invalid_regex_and_a_group_that_takes_no_part_are_errors() {
        assert_eq!(matched_text(DOCKERFILE, r"USER (\d+):"), Err(r#""USER (\d+):" matches nothing"#.into()));
        let invalid = matched_text(DOCKERFILE, r"USER (\d+").unwrap_err();
        assert!(invalid.starts_with(r#""USER (\d+" is not a valid regex: "#), "{invalid}");
        assert_eq!(
            matched_text(DOCKERFILE, r"(ROOT)?USER"),
            Err(r#""(ROOT)?USER" matches without its first group"#.into())
        );
    }
}
