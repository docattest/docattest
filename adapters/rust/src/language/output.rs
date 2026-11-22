//! Trailing `// Output:` and `// Unordered output:` comments on Rust example blocks,
//! matching Go's testable example convention.

/// What a block's trailing output comment says the block prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpectedOutput {
    pub text: String,
    pub unordered: bool,
}

/// Extract a trailing `// Output:` or `// Unordered output:` comment block from `code`.
pub(crate) fn expected(code: &str) -> Option<ExpectedOutput> {
    let normalized = code.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut end = lines.len();
    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && lines[start - 1].trim_start().starts_with("//") {
        start -= 1;
    }
    if start == end {
        return None;
    }
    let comment_lines: Vec<&str> = lines[start..end].iter().map(|line| strip_prefix(line)).collect();
    let (idx, unordered, first_tail) =
        comment_lines.iter().enumerate().find_map(|(i, line)| parse_header(line).map(|(u, t)| (i, u, t)))?;
    let rest = &comment_lines[idx + 1..];
    let text = match (!first_tail.is_empty(), !rest.is_empty()) {
        (true, true) => format!("{first_tail}\n{}", rest.join("\n")),
        (true, false) => first_tail.to_string(),
        (false, _) => rest.join("\n"),
    };
    Some(ExpectedOutput { text, unordered })
}

fn strip_prefix(line: &str) -> &str {
    let body = line.trim_start().strip_prefix("//").unwrap_or(line);
    body.strip_prefix(' ').unwrap_or(body)
}

fn parse_header(line: &str) -> Option<(bool, &str)> {
    let (label, tail) = line.split_once(':')?;
    let lower = label.to_ascii_lowercase();
    let unordered = match lower.as_str() {
        "output" => false,
        "unordered output" => true,
        _ => return None,
    };
    Some((unordered, tail.strip_prefix(' ').unwrap_or(tail)))
}

/// Result of comparing captured stdout against an [`ExpectedOutput`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutputVerdict {
    Pass,
    Fail { message: String, expected: String, actual: String },
}

/// Compare captured stdout against what an `// Output:` comment states.
pub(crate) fn compare(got: &str, want: &ExpectedOutput) -> OutputVerdict {
    let mut actual = got.replace("\r\n", "\n").trim().to_string();
    let mut expected = want.text.trim().to_string();
    if want.unordered {
        actual = sort_lines(&actual);
        expected = sort_lines(&expected);
    }
    if actual == expected {
        return OutputVerdict::Pass;
    }
    let message = format!("expected output {expected:?}, got {actual:?}");
    OutputVerdict::Fail { message, expected, actual }
}

fn sort_lines(text: &str) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    lines.sort_unstable();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_multiline_and_unordered_output_comments() {
        assert_eq!(
            expected("println!(\"7.50 EUR\");\n// Output: 7.50 EUR\n"),
            Some(ExpectedOutput { text: "7.50 EUR".into(), unordered: false })
        );
        assert_eq!(
            expected("println!(\"a\");\nprintln!(\"b\");\n// Unordered output:\n// b\n// a\n"),
            Some(ExpectedOutput { text: "b\na".into(), unordered: true })
        );
        assert_eq!(expected("// Output: 1\nprintln!(\"1\");\n// remark\n"), None);
    }

    #[test]
    fn compares_ordered_and_unordered_stdout() {
        let want = ExpectedOutput { text: "a\nb".into(), unordered: true };
        assert_eq!(compare("b\na\n", &want), OutputVerdict::Pass);
        let ordered = ExpectedOutput { text: "a\nb".into(), unordered: false };
        assert!(matches!(compare("b\na\n", &ordered), OutputVerdict::Fail { .. }));
    }
}
