//! Which GFM autolink literals leave nodes without positions, so that their page fails.
//!
//! Pages are read as `mdast-util-from-markdown` reads them (see [`super::parse`]), and there
//! `mdast-util-gfm-autolink-literal` finishes autolink literals with a tree transform over
//! text nodes that creates link and text nodes *without positions*. No line can be given
//! for those, so such a page fails with an internal error ("markdown node "text" has no
//! position"), as the recordings hold. micromark's tokenizer links most literals itself,
//! with positions; the transform catches the rest: a `www.` or `http(s)://` literal after a
//! character the tokenizer refuses (`'www.a.com'`, `<www.a.com>`), and any literal after an
//! unbalanced `[`. markdown-rs links some of those itself (it allows a literal after a
//! bracket) and leaves others as text. This module answers, from markdown-rs's tree,
//! whether the transform would fire.

use std::sync::OnceLock;

use markdown::mdast::Node;

use crate::text::is_js_space;

/// Whether the autolink-literal transform would change any text among these phrasing
/// children (and so leave nodes without positions).
pub fn transform_fires(children: &[Node], source: &str) -> bool {
    let mut walker = Walker { source, open: 0, run: String::new(), fired: false };
    walker.children(children);
    walker.flush();
    walker.fired
}

struct Walker<'a> {
    source: &'a str,
    /// Label starts (`[`, `![`) not yet balanced by a `]`.
    open: usize,
    /// The text of the current run of adjacent text, as one mdast text node holds it.
    run: String,
    fired: bool,
}

impl Walker<'_> {
    fn children(&mut self, children: &[Node]) {
        for child in children {
            self.node(child);
        }
    }

    fn node(&mut self, node: &Node) {
        match node {
            Node::Text(text) => {
                self.brackets(node);
                self.run.push_str(&text.value);
            }
            Node::Link(link) if self.is_literal(node) => {
                // micromark's tokenizer refuses a literal after an unbalanced bracket; it
                // stays text, joined to the text around it.
                let text = link.children.iter().map(text_value).collect::<String>();
                if self.open > 0 {
                    self.run.push_str(&text);
                } else {
                    self.flush();
                }
            }
            // The transform ignores text inside links; their brackets are their own.
            Node::Link(_) | Node::LinkReference(_) | Node::Image(_) | Node::ImageReference(_) => {
                self.flush();
            }
            _ => {
                self.flush();
                if let Some(children) = node.children() {
                    self.children(children);
                    self.flush();
                }
            }
        }
    }

    // A literal is written as its text; a resource link starts with `[`, an autolink with `<`.
    fn is_literal(&self, node: &Node) -> bool {
        let Some(p) = node.position() else {
            return false;
        };
        let written = self.source.get(p.start.offset..p.end.offset).unwrap_or("");
        !written.starts_with('[') && !written.starts_with('<')
    }

    // Label starts and ends in a text node, from its source. A bracket after an odd number
    // of backslashes is escaped (markdown-rs starts a text node after a leading escape's
    // backslash, so the count looks before the node too).
    fn brackets(&mut self, node: &Node) {
        let Some(p) = node.position() else {
            return;
        };
        let bytes = self.source.as_bytes();
        for at in p.start.offset..p.end.offset.min(bytes.len()) {
            let escaped = bytes[..at].iter().rev().take_while(|&&b| b == b'\\').count() % 2 == 1;
            match bytes[at] {
                b'[' if !escaped => self.open += 1,
                b']' if !escaped => self.open = self.open.saturating_sub(1),
                _ => {}
            }
        }
    }

    fn flush(&mut self) {
        if !self.run.is_empty() {
            if finds_literal(&self.run) {
                self.fired = true;
            }
            self.run.clear();
        }
    }
}

fn text_value(node: &Node) -> String {
    match node {
        Node::Text(t) => t.value.clone(),
        _ => node.children().map_or(String::new(), |c| c.iter().map(text_value).collect()),
    }
}

/// Whether the transform accepts a URL or an email literal anywhere in one text node's
/// value.
pub fn finds_literal(value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    (0..chars.len()).any(|at| url_at(&chars, at) || email_at(&chars, at))
}

fn is_punctuation(c: char) -> bool {
    static PUNCTUATION: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let class = PUNCTUATION
        .get_or_init(|| fancy_regex::Regex::new(r"^[\p{P}\p{S}]$").expect("a constant pattern compiles"));
    // One character against one class: no backtracking, so matching cannot fail.
    class.is_match(c.encode_utf8(&mut [0; 4])).unwrap_or(false)
}

/// Whether a URL may start at `at`: at the start, or after whitespace or punctuation.
fn url_may_start(chars: &[char], at: usize) -> bool {
    char_before(chars, at).is_none_or(is_boundary)
}

/// Whether an email address may start at `at`: as a URL may, except after `/`.
fn email_may_start(chars: &[char], at: usize) -> bool {
    char_before(chars, at).is_none_or(|c| is_boundary(c) && c != '/')
}

fn char_before(chars: &[char], at: usize) -> Option<char> {
    at.checked_sub(1).and_then(|k| chars.get(k)).copied()
}

/// Whitespace or punctuation of one UTF-16 unit: an astral character is neither, since
/// the boundary is judged on the unit just before the match.
fn is_boundary(c: char) -> bool {
    c.len_utf16() == 1 && (is_js_space(c) || is_punctuation(c))
}

fn is_ascii_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn starts_with_ignore_case(chars: &[char], at: usize, word: &str) -> bool {
    let mut k = at;
    for w in word.chars() {
        match chars.get(k) {
            Some(c) if c.eq_ignore_ascii_case(&w) => k += 1,
            _ => return false,
        }
    }
    true
}

/// `/(https?:\/\/|www(?=\.))([-.\w]+)([^ \t\r\n]*)/i` anchored at `at`, then the
/// transform's checks of a URL.
fn url_at(chars: &[char], at: usize) -> bool {
    let Some((protocol_len, www)) = url_protocol(chars, at) else {
        return false;
    };
    let domain_start = at + protocol_len;
    let domain_end = run_end(chars, domain_start, |c| c == '-' || c == '.' || is_ascii_word(c));
    if domain_end == domain_start {
        return false;
    }
    let end = run_end(chars, domain_end, |c| !matches!(c, ' ' | '\t' | '\r' | '\n'));
    if !url_may_start(chars, at) {
        return false;
    }
    let domain_from = if www { at } else { domain_start };
    let domain: String = chars[domain_from..domain_end].iter().collect();
    if !is_correct_domain(&domain) {
        return false;
    }
    let url: String = chars[domain_from..end].iter().collect();
    !split_url(&url).is_empty()
}

/// `https://`, `http://` or `www` before a `.`: its length, and whether it is `www` (part
/// of the domain).
fn url_protocol(chars: &[char], at: usize) -> Option<(usize, bool)> {
    if starts_with_ignore_case(chars, at, "https://") {
        Some((8, false))
    } else if starts_with_ignore_case(chars, at, "http://") {
        Some((7, false))
    } else if starts_with_ignore_case(chars, at, "www") && chars.get(at + 3) == Some(&'.') {
        Some((3, true))
    } else {
        None
    }
}

/// Where the run of characters from `from` that `keep` accepts ends.
fn run_end(chars: &[char], from: usize, keep: impl Fn(char) -> bool) -> usize {
    let mut k = from;
    while chars.get(k).is_some_and(|&c| keep(c)) {
        k += 1;
    }
    k
}

fn is_correct_domain(domain: &str) -> bool {
    let parts: Vec<&str> = domain.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let bad = |part: &str| {
        !part.is_empty() && (part.contains('_') || !part.chars().any(|c| c.is_ascii_alphanumeric()))
    };
    !(bad(parts[parts.len() - 1]) || bad(parts[parts.len() - 2]))
}

/// The URL part of a literal: trailing punctuation removed, closing parens kept while they
/// balance an opening one.
fn split_url(url: &str) -> String {
    const TRAIL: &[char] = &['!', '"', '&', '\'', ')', ',', '.', ':', ';', '<', '>', '?', ']', '}'];
    let kept = url.trim_end_matches(TRAIL);
    if kept.len() == url.len() {
        return url.to_string();
    }
    let mut out = kept.to_string();
    let mut trail = &url[kept.len()..];
    let opening = out.matches('(').count();
    let mut closing = out.matches(')').count();
    while let Some(paren) = trail.find(')') {
        if opening <= closing {
            break;
        }
        out.push_str(&trail[..=paren]);
        trail = &trail[paren + 1..];
        closing += 1;
    }
    out
}

/// `/(?<=^|\s|\p{P}|\p{S})([-.\w+]+)@([-\w]+(?:\.[-\w]+)+)/u` anchored at `at`, then the
/// transform's checks of an email address.
fn email_at(chars: &[char], at: usize) -> bool {
    if !after_boundary(chars, at) {
        return false;
    }
    let local_end = run_end(chars, at, is_atext);
    if local_end == at || chars.get(local_end) != Some(&'@') {
        return false;
    }
    let Some(end) = email_domain_end(chars, local_end + 1) else {
        return false;
    };
    if !email_may_start(chars, at) {
        return false;
    }
    let last = chars[end - 1];
    !(last == '-' || last == '_' || last.is_ascii_digit())
}

/// `(?<=^|\s|\p{P}|\p{S})` at `at`.
fn after_boundary(chars: &[char], at: usize) -> bool {
    at.checked_sub(1)
        .and_then(|k| chars.get(k))
        .is_none_or(|&before| is_js_space(before) || is_punctuation(before))
}

/// `[-\w]+(?:\.[-\w]+)+` from `start`: where it ends.
fn email_domain_end(chars: &[char], start: usize) -> Option<usize> {
    let mut k = run_end(chars, start, is_label);
    if k == start {
        return None;
    }
    let mut dots = 0;
    while chars.get(k) == Some(&'.') && chars.get(k + 1).is_some_and(|&c| is_label(c)) {
        k = run_end(chars, k + 1, is_label);
        dots += 1;
    }
    (dots > 0).then_some(k)
}

/// `[-.\w+]`, a character of an address's local part.
fn is_atext(c: char) -> bool {
    c == '-' || c == '.' || c == '+' || is_ascii_word(c)
}

/// `[-\w]`, a character of a domain label.
fn is_label(c: char) -> bool {
    c == '-' || is_ascii_word(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fires(text: &str) -> bool {
        match markdown::to_mdast(text, &markdown::ParseOptions::gfm()) {
            Ok(Node::Root(root)) => {
                root.children.iter().any(|block| block.children().is_some_and(|c| transform_fires(c, text)))
            }
            _ => false,
        }
    }

    #[test]
    fn the_transform_fires_where_the_tokenizer_refused() {
        assert!(fires("[GitHub repo][https://github.com/x/y]"));
        assert!(fires("Copyright <www.openjsf.org>"));
        assert!(fires("say 'www.example.com' ok"));
        assert!(fires("[a https://z.com"));
        assert!(fires("[www.x.com]"));
    }

    #[test]
    fn it_does_not_fire_where_the_tokenizer_linked() {
        assert!(!fires("mail a@b.com"));
        assert!(!fires("x\"https://a.com\""));
        assert!(!fires("a(www.x.com)"));
        assert!(!fires("see [x](y) and https://z.com"));
        assert!(!fires("[a] https://z.com"));
        assert!(!fires("\\[ https://z.com"));
        assert!(!fires("xhttps://a.com"));
        assert!(!fires("/a@b.com"));
        assert!(!fires("plain text"));
    }
}
