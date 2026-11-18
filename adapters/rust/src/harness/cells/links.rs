//! Links and images, whose destination and title hold no code span: what follows a `]`
//! that closes a bracket into a link, `(destination "title")` with either part optional,
//! as CommonMark has it.

/// A `[` or `![` that a later `]` may close into a link or an image.
struct Opener {
    image: bool,
    active: bool,
}

/// The openers not yet closed, innermost last.
#[derive(Default)]
pub(super) struct Brackets {
    open: Vec<Opener>,
}

impl Brackets {
    /// Records the opener of a link (`[`) or an image (`![`).
    pub(super) fn open(&mut self, image: bool) {
        self.open.push(Opener { image, active: true });
    }

    /// Closes the innermost opener with the `]` that `rest` follows, and gives the length
    /// of the link's tail when the two make a link, else 0. A link cannot hold a link, so
    /// the link openers around one that closes into a link become text.
    pub(super) fn close(&mut self, rest: &[u8]) -> usize {
        let Some(innermost) = self.open.pop() else { return 0 };
        let tail = tail_length(rest);
        if !innermost.active || tail == 0 {
            return 0;
        }
        if !innermost.image {
            self.open.iter_mut().filter(|opener| !opener.image).for_each(|opener| opener.active = false);
        }
        tail
    }
}

/// The length of a link's tail that a text starts with, or 0. Of the ways to read it, the
/// one that reads the most is the one CommonMark's rules choose.
fn tail_length(text: &[u8]) -> usize {
    if text.first() != Some(&b'(') {
        return 0;
    }
    let start = 1 + spaces(&text[1..]);
    let destinations = std::iter::once(start).chain(destination(text, start));
    let ends = destinations.flat_map(|at| {
        let titled =
            Some(spaces(&text[at..])).filter(|&n| n > 0).into_iter().flat_map(move |n| title(text, at + n));
        std::iter::once(at).chain(titled.collect::<Vec<_>>())
    });
    let closed = ends.filter_map(|at| {
        let at = at + spaces(&text[at..]);
        (text.get(at) == Some(&b')')).then_some(at + 1)
    });
    closed.max().unwrap_or(0)
}

/// Where a destination that starts at a position may end: `<…>` with no line break, or
/// a run of what is not space, a control character or a parenthesis, with parentheses one
/// deep.
fn destination(text: &[u8], at: usize) -> Vec<usize> {
    if text.get(at) == Some(&b'<') {
        let angled = |byte: u8| !b"<>\n".contains(&byte);
        let inside = repeated(text, vec![at + 1], |k| char_or_escape(text, k, angled));
        return inside.into_iter().filter(|&k| text.get(k) == Some(&b'>')).map(|k| k + 1).collect();
    }
    let plain = |k: usize| char_or_escape(text, k, |byte| byte > b' ' && byte != b'(' && byte != b')');
    let unit = |k: usize| {
        let mut ends = plain(k);
        if text.get(k) == Some(&b'(') {
            let inner = repeated(text, vec![k + 1], plain);
            ends.extend(inner.into_iter().filter(|&j| text.get(j) == Some(&b')')).map(|j| j + 1));
        }
        ends
    };
    repeated(text, unit(at), unit)
}

/// Where a title that starts at a position may end: `"…"`, `'…'` or `(…)`.
fn title(text: &[u8], at: usize) -> Vec<usize> {
    let close = match text.get(at) {
        Some(b'"') => b'"',
        Some(b'\'') => b'\'',
        Some(b'(') => b')',
        _ => return Vec::new(),
    };
    let inner = |byte: u8| byte != close && !(close == b')' && byte == b'(');
    let inside = repeated(text, vec![at + 1], |k| char_or_escape(text, k, inner));
    inside.into_iter().filter(|&k| text.get(k) == Some(&close)).map(|k| k + 1).collect()
}

/// Where one character may end: one that is allowed, or a backslash's escape.
fn char_or_escape(text: &[u8], at: usize, allowed: impl Fn(u8) -> bool) -> Vec<usize> {
    match text.get(at) {
        Some(b'\\') => escape(text, at),
        Some(&byte) if allowed(byte) => vec![at + 1],
        _ => Vec::new(),
    }
}

/// Where a backslash may end: with the punctuation it escapes, or alone.
fn escape(text: &[u8], at: usize) -> Vec<usize> {
    let escapes = text.get(at + 1).is_some_and(u8::is_ascii_punctuation);
    if escapes { vec![at + 2, at + 1] } else { vec![at + 1] }
}

/// Every position reached from some by repeating a step any number of times, the
/// starting ones among them.
fn repeated(text: &[u8], starts: Vec<usize>, step: impl Fn(usize) -> Vec<usize>) -> Vec<usize> {
    let mut reached = starts.clone();
    let mut pending = starts;
    while let Some(at) = pending.pop() {
        for next in step(at).into_iter().filter(|&next| next <= text.len()) {
            if !reached.contains(&next) {
                reached.push(next);
                pending.push(next);
            }
        }
    }
    reached
}

/// The length of the white space a link allows: spaces, tabs and line feeds.
fn spaces(text: &[u8]) -> usize {
    text.iter().take_while(|&&b| b == b' ' || b == b'\t' || b == b'\n').count()
}
