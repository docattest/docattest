//! MDX preprocessing: line-preserving translation of `.mdx` files so the CommonMark/GFM
//! parser sees JSX comments `{/* … */}` as directives and reads Markdown inside JSX wrappers.

use std::path::Path;

use crate::markdown::comments::opening;

/// Whether `page` is an MDX file (`.mdx`, case-insensitive).
pub fn is_mdx(page: &str) -> bool {
    Path::new(page)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mdx"))
}

/// Format a directive comment for `page`: `{/* … */}` in MDX, `<!-- … -->` in Markdown.
pub fn format_directive(page: &str, body: &str) -> String {
    if is_mdx(page) { format!("{{/* {body} */}}") } else { format!("<!-- {body} -->") }
}

/// Rewrite MDX-only syntax into GFM-compatible text while keeping every line on its exact
/// original line number. Fenced code blocks and YAML frontmatter pass through verbatim.
pub fn preprocess(text: &str) -> String {
    let source =
        if text.contains('\r') { text.replace("\r\n", "\n").replace('\r', "\n") } else { text.to_string() };
    let mut out = String::with_capacity(source.len() + 32);
    let mut state = State::default();
    let lines: Vec<&str> = source.split('\n').collect();
    let fm_end = frontmatter_end(&lines);
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if idx < fm_end {
            out.push_str(line);
            continue;
        }
        process_line(line, &mut state, &mut out);
    }
    out
}

#[derive(Default)]
struct State {
    fence: Option<(char, usize)>,
    in_jsx_comment: bool,
    in_esm_block: bool,
}

fn frontmatter_end(lines: &[&str]) -> usize {
    let is_fence = |line: &str| {
        let line = line.strip_prefix('\u{FEFF}').unwrap_or(line);
        line.strip_prefix("---").is_some_and(|rest| rest.chars().all(|c| c == ' ' || c == '\t'))
    };
    if lines.len() < 2 || !is_fence(lines[0]) {
        return 0;
    }
    lines.iter().enumerate().skip(1).find(|(_, line)| is_fence(line)).map_or(0, |(idx, _)| idx + 1)
}

fn process_line(line: &str, state: &mut State, out: &mut String) {
    if continue_multiline(line, state, out) {
        return;
    }
    if let Some(opened) = opens_fence(line) {
        state.fence = Some(opened);
        out.push_str(line);
        return;
    }
    if is_esm_line(line) {
        state.in_esm_block = line.contains('{') && !line.contains('}');
        return;
    }
    if !is_standalone_jsx_tag(line) {
        rewrite_jsx_comments(line, state, out);
    }
}

fn continue_multiline(line: &str, state: &mut State, out: &mut String) -> bool {
    if let Some((ch, len)) = state.fence {
        out.push_str(line);
        if closes_fence(line, ch, len) {
            state.fence = None;
        }
        return true;
    }
    if state.in_jsx_comment {
        rewrite_comment_continuation(line, state, out);
        return true;
    }
    if state.in_esm_block {
        if line.contains(';') || line.contains("from ") || line.trim().ends_with('}') {
            state.in_esm_block = false;
        }
        return true;
    }
    false
}

fn opens_fence(line: &str) -> Option<(char, usize)> {
    let trimmed = trim_up_to_three_spaces(line);
    let ch = trimmed.chars().next().filter(|&c| c == '`' || c == '~')?;
    let count = trimmed.chars().take_while(|&c| c == ch).count();
    if count < 3 {
        return None;
    }
    let rest = &trimmed[count..];
    if ch == '`' && rest.contains('`') {
        return None;
    }
    Some((ch, count))
}

fn closes_fence(line: &str, ch: char, min_len: usize) -> bool {
    let trimmed = trim_up_to_three_spaces(line);
    let count = trimmed.chars().take_while(|&c| c == ch).count();
    count >= min_len && trimmed[count..].chars().all(|c| c == ' ' || c == '\t')
}

fn trim_up_to_three_spaces(line: &str) -> &str {
    let spaces = line.bytes().take(3).take_while(|&b| b == b' ').count();
    &line[spaces..]
}

fn is_esm_line(line: &str) -> bool {
    line.starts_with("import ") || line.starts_with("export ")
}

/// True when a line consists solely of a JSX component tag (`<Tabs>`, `</Tabs>`,
/// `<TabItem label="A">`, `<Card />`), so GFM parses the Markdown inside it as normal blocks.
fn is_standalone_jsx_tag(line: &str) -> bool {
    let t = line.trim();
    if !t.starts_with('<') || !t.ends_with('>') || t.len() < 3 {
        return false;
    }
    let after = t.strip_prefix("</").unwrap_or(&t[1..]);
    after.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && closes_only_at_end(t)
}

/// Reject lines with prose between multiple tags like `<Badge>foo</Badge>`.
fn closes_only_at_end(tag: &str) -> bool {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (i, c) in tag.char_indices() {
        if quote.is_some() {
            if quote == Some(c) {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '>' if depth == 0 => return i == tag.len() - 1,
            _ => {}
        }
    }
    false
}

fn rewrite_comment_continuation(line: &str, state: &mut State, out: &mut String) {
    if let Some(close) = line.find("*/}") {
        out.push_str(&line[..close]);
        out.push_str("-->");
        state.in_jsx_comment = false;
        rewrite_jsx_comments(&line[close + 3..], state, out);
    } else {
        out.push_str(line);
    }
}

/// Replace `{/* … */}` outside inline code spans with `<!-- … -->`, and track multi-line
/// or unclosed directive comments.
fn rewrite_jsx_comments(line: &str, state: &mut State, out: &mut String) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let end = skip_code_span(line, i);
            out.push_str(&line[i..end]);
            i = end;
            continue;
        }
        if line[i..].starts_with("{/*") {
            let body_start = i + 3;
            if let Some(rel_close) = line[body_start..].find("*/}") {
                let close = body_start + rel_close;
                out.push_str("<!--");
                out.push_str(&line[body_start..close]);
                out.push_str("-->");
                i = close + 3;
                continue;
            }
            let tail = &line[body_start..];
            let looks_like_directive = opening(&format!("<!--{tail}")).is_some();
            if looks_like_directive || tail.trim().is_empty() {
                out.push_str("<!--");
                out.push_str(tail);
                state.in_jsx_comment = true;
                return;
            }
        }
        let ch = line[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
}

fn skip_code_span(line: &str, start: usize) -> usize {
    let ticks = line[start..].bytes().take_while(|&b| b == b'`').count();
    let mut i = start + ticks;
    while i < line.len() {
        if line.as_bytes()[i] == b'`' {
            let run = line[i..].bytes().take_while(|&b| b == b'`').count();
            if run == ticks {
                return i + run;
            }
            i += run;
        } else {
            let ch = line[i..].chars().next().unwrap_or(' ');
            i += ch.len_utf8();
        }
    }
    start + ticks
}

#[cfg(test)]
mod tests {
    use crate::markdown::parse::parse_page_for;

    #[test]
    fn rewrites_jsx_comments_and_preserves_lines_and_code() {
        let mdx = [
            "import { Tabs, TabItem } from '@astrojs/starlight/components';",
            "",
            "# Retries",
            "",
            "<Tabs>",
            "<TabItem label=\"Default\">",
            "At most `3` retries. {/* fact: RETRIES */}",
            "</TabItem>",
            "</Tabs>",
            "",
            "```tsx",
            "{/* not a directive */}",
            "```",
            "Inline `{/* not a directive */}` stays.",
        ]
        .join("\n");
        let page = parse_page_for("docs/retries.mdx", &mdx).unwrap();
        assert_eq!(page.comments.len(), 1);
        assert_eq!(page.comments[0].line, 7);
        assert_eq!(page.comments[0].body.trim(), "fact: RETRIES");
        assert!(page.comments[0].lead.contains("At most `3` retries."));
    }

    #[test]
    fn detects_unclosed_and_multiline_jsx_directives() {
        let multi = "{/*\n  fact: KEYS\n*/}\n- `A`\n- `B`\n";
        let page = parse_page_for("docs/keys.mdx", multi).unwrap();
        assert_eq!(page.comments.len(), 1);
        assert_eq!(page.comments[0].line, 1);
        assert_eq!(page.comments[0].end_line, 3);

        let unclosed = "At most `3`. {/* fact: N\n";
        let page = parse_page_for("docs/bad.mdx", unclosed).unwrap();
        assert_eq!(page.unclosed.len(), 1);
        assert_eq!(page.unclosed[0].line, 1);
    }
}
