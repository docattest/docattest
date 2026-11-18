//! What CommonMark reads instead of a code span when it starts first: an autolink, or an
//! HTML tag, comment, processing instruction, declaration or CDATA section, as the spec
//! defines each. Their backticks are theirs.

/// A form a text may start with: its length when it does.
type Form = fn(&[u8]) -> Option<usize>;

/// The length of the autolink or the HTML that a text starts with, or 0. The forms are
/// tried in the spec's order, and the first that matches is the one read.
pub(super) fn length(text: &[u8]) -> usize {
    let forms: [Form; 9] =
        [autolink, email, open_tag, closing_tag, empty_comment, comment, instruction, declaration, cdata];
    forms.iter().find_map(|form| form(text)).unwrap_or(0)
}

/// `<scheme:…>`: a scheme of 2 to 32 characters, and no space, `<` or `>` after it.
fn autolink(text: &[u8]) -> Option<usize> {
    let rest = text.strip_prefix(b"<")?;
    let scheme = scheme_length(rest)?;
    let target = rest[scheme + 1..].iter().take_while(|&&b| b > b' ' && b != b'<' && b != b'>').count();
    (rest.get(scheme + 1 + target) == Some(&b'>')).then_some(scheme + target + 3)
}

/// The length of the scheme a text starts with, when a `:` follows it.
fn scheme_length(text: &[u8]) -> Option<usize> {
    if !text.first()?.is_ascii_alphabetic() {
        return None;
    }
    let scheme =
        1 + text[1..].iter().take_while(|&&b| b.is_ascii_alphanumeric() || b"+.-".contains(&b)).count();
    ((2..=32).contains(&scheme) && text.get(scheme) == Some(&b':')).then_some(scheme)
}

/// `<local@domain>`, with the domain's labels as the spec bounds them.
fn email(text: &[u8]) -> Option<usize> {
    let rest = text.strip_prefix(b"<")?;
    let local = rest
        .iter()
        .take_while(|&&b| b.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(&b))
        .count();
    if local == 0 || rest.get(local) != Some(&b'@') {
        return None;
    }
    let mut at = local + 1;
    loop {
        at += label_length(&rest[at..])?;
        match rest.get(at) {
            Some(b'.') => at += 1,
            Some(b'>') => return Some(at + 2),
            _ => return None,
        }
    }
}

/// The length of the domain label a text starts with: 1 to 63 letters, digits and `-`,
/// neither first nor last a `-`.
fn label_length(text: &[u8]) -> Option<usize> {
    let label = text.iter().take_while(|&&b| b.is_ascii_alphanumeric() || b == b'-').count();
    let edges = [text.first()?, text.get(label.checked_sub(1)?)?];
    (label <= 63 && !edges.contains(&&b'-')).then_some(label)
}

/// `<name attribute="value" …>` or `<name … />`.
fn open_tag(text: &[u8]) -> Option<usize> {
    let rest = text.strip_prefix(b"<")?;
    let mut at = tag_name(rest)?;
    while let Some(end) = attribute(rest, at) {
        at = end;
    }
    at += spaces(&rest[at..]);
    at += usize::from(rest.get(at) == Some(&b'/'));
    (rest.get(at) == Some(&b'>')).then_some(at + 2)
}

/// `</name >`.
fn closing_tag(text: &[u8]) -> Option<usize> {
    let rest = text.strip_prefix(b"</")?;
    let at = tag_name(rest)?;
    let at = at + spaces(&rest[at..]);
    (rest.get(at) == Some(&b'>')).then_some(at + 3)
}

/// `<!-->` and `<!--->`, which are comments that end where they begin.
fn empty_comment(text: &[u8]) -> Option<usize> {
    [b"<!-->".as_slice(), b"<!--->"].into_iter().find(|form| text.starts_with(form)).map(<[u8]>::len)
}

fn comment(text: &[u8]) -> Option<usize> {
    until(text, b"<!--", b"-->")
}

fn instruction(text: &[u8]) -> Option<usize> {
    until(text, b"<?", b"?>")
}

/// `<!X…>`: a letter after `<!`, and anything up to the first `>`.
fn declaration(text: &[u8]) -> Option<usize> {
    let rest = text.strip_prefix(b"<!")?;
    if !rest.first()?.is_ascii_alphabetic() {
        return None;
    }
    rest.iter().position(|&b| b == b'>').map(|end| end + 3)
}

fn cdata(text: &[u8]) -> Option<usize> {
    until(text, b"<![CDATA[", b"]]>")
}

/// The length of a text that opens with `open` up to the first `close` after it.
fn until(text: &[u8], open: &[u8], close: &[u8]) -> Option<usize> {
    let rest = text.strip_prefix(open)?;
    let end = rest.windows(close.len()).position(|window| window == close)?;
    Some(open.len() + end + close.len())
}

/// The length of a tag's name: a letter, then letters, digits and `-`.
fn tag_name(text: &[u8]) -> Option<usize> {
    if !text.first()?.is_ascii_alphabetic() {
        return None;
    }
    Some(1 + text[1..].iter().take_while(|&&b| b.is_ascii_alphanumeric() || b == b'-').count())
}

/// Where an attribute that starts at a position ends: white space, a name, and perhaps
/// `=` and a value.
fn attribute(text: &[u8], at: usize) -> Option<usize> {
    let lead = spaces(&text[at..]);
    let name_at = at + lead;
    let first = *text.get(name_at)?;
    if lead == 0 || !(first.is_ascii_alphabetic() || first == b'_' || first == b':') {
        return None;
    }
    let name = 1 + text[name_at + 1..]
        .iter()
        .take_while(|&&b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
        .count();
    let end = name_at + name;
    Some(value(text, end).unwrap_or(end))
}

/// Where `= value` after an attribute's name ends, when it is there.
fn value(text: &[u8], at: usize) -> Option<usize> {
    let mut at = at + spaces(&text[at..]);
    if text.get(at) != Some(&b'=') {
        return None;
    }
    at += 1;
    at += spaces(&text[at..]);
    let quote = *text.get(at)?;
    if quote == b'"' || quote == b'\'' {
        let close = text[at + 1..].iter().position(|&b| b == quote)?;
        return Some(at + close + 2);
    }
    let unquoted = text[at..].iter().take_while(|&&b| b > b' ' && !b"\"'=<>`".contains(&b)).count();
    (unquoted > 0).then_some(at + unquoted)
}

/// The length of the white space a tag allows: spaces, tabs and line feeds.
fn spaces(text: &[u8]) -> usize {
    text.iter().take_while(|&&b| b == b' ' || b == b'\t' || b == b'\n').count()
}
