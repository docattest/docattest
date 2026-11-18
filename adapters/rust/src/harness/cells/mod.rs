//! Reading the cells of a cases table. A runner gets each cell as the page writes it, code
//! spans with their backticks; [`code_spans`] takes out what docattest itself reads from a
//! cell. A setup file calls it as `docattest_harness::cells::code_spans`.

mod links;
mod raw;

/// The content of every code span in a cell, in order, as a fact over the cell's column
/// compares it: a span opens with a run of backticks and closes with the next run of as
/// many, and its content is trimmed of white space. As CommonMark has it, a backtick
/// escaped with a backslash opens nothing, and the backticks of an HTML tag or comment, of
/// an autolink, or of a link's destination or title belong to those. A cell with no code
/// span gives none.
pub fn code_spans(cell: &str) -> Vec<String> {
    let bytes = cell.as_bytes();
    let mut spans = Vec::new();
    let mut openers = links::Brackets::default();
    let mut at = 0;
    while at < bytes.len() {
        at += match bytes[at] {
            b'\\' if bytes.get(at + 1).is_some_and(u8::is_ascii_punctuation) => 2,
            b'<' => raw::length(&bytes[at..]).max(1),
            b'!' if bytes.get(at + 1) == Some(&b'[') => {
                openers.open(true);
                2
            }
            b'[' => {
                openers.open(false);
                1
            }
            b']' => 1 + openers.close(&bytes[at + 1..]),
            b'`' => {
                let (span, length) = code_span(&cell[at..]);
                spans.extend(span);
                length
            }
            _ => 1,
        };
    }
    spans
}

/// Reads the code span a text starts with, which begins with a backtick: its content,
/// trimmed, and its length. When no run closes it, the opening run is only text, and its
/// length is that run's.
fn code_span(text: &str) -> (Option<String>, usize) {
    let fence = run_length(text.as_bytes(), 0);
    match closing_run(text.as_bytes(), fence) {
        Some(end) => (Some(text[fence..end].trim_matches(is_space).to_string()), end + fence),
        None => (None, fence),
    }
}

/// The length of the run of backticks that starts at a position.
fn run_length(text: &[u8], at: usize) -> usize {
    text[at..].iter().take_while(|&&byte| byte == b'`').count()
}

/// Where the first run of exactly `fence` backticks after the opening one starts. A
/// longer or shorter run is part of the span's content.
fn closing_run(text: &[u8], fence: usize) -> Option<usize> {
    let mut at = fence;
    while at < text.len() {
        if text[at] != b'`' {
            at += 1;
            continue;
        }
        let run = run_length(text, at);
        if run == fence {
            return Some(at);
        }
        at += run;
    }
    None
}

/// The white space docattest trims from a value: JavaScript's `\s`, which differs from
/// `char::is_whitespace` in U+0085 (not space) and U+FEFF (space).
fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// protocol/test-data/code-spans.json is the contract every language's cell helper is
    /// tested against, and the core too.
    #[test]
    fn a_cells_code_spans_are_the_shared_tables() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../protocol/test-data/code-spans.json");
        let table: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for case in table["cases"].as_array().unwrap() {
            let want: Vec<&str> =
                case["spans"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
            assert_eq!(
                code_spans(case["cell"].as_str().unwrap()),
                want,
                "{}: {}",
                case["name"],
                case["cell"]
            );
        }
    }
}
