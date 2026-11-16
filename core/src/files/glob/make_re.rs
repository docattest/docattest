//! picomatch.makeRe / compileRe / parse.fastpaths.

use super::constants::{
    DOT_LITERAL, GLOBSTAR, NO_DOTS, NO_DOTS_SLASH, ONE_CHAR, QMARK, SLASH_LITERAL, STAR, apply_replacements,
    length_error, remove_prefix,
};
use super::parse::parse;
use super::units::{
    BACKSLASH, DOT, QMARK_CH, STAR_CH, cat, ext, is, is_line_terminator, is_word_unit, slice, w,
};
use super::{MAX_LENGTH, U};

/// `picomatch.makeRe(input, { dot: true })`'s source: what picomatch passes to `new RegExp`.
pub(super) fn make_re(input: &[u16]) -> Result<U, String> {
    if matches!(input.first(), Some(&c) if c == DOT || c == STAR_CH)
        && let Some(output) = fastpaths(input)?
        && !output.is_empty()
    {
        return Ok(compile_re(&output, false));
    }
    let parsed = parse(input, true)?;
    Ok(compile_re(&parsed.output, parsed.negated))
}

fn compile_re(output: &[u16], negated: bool) -> U {
    let source = cat(&[&w("^(?:"), output, &w(")$")]);
    if negated { cat(&[&w("^(?!"), &source, &w(").*$")]) } else { source }
}

/// `parse.fastpaths`.
pub(super) fn fastpaths(input: &[u16]) -> Result<Option<U>, String> {
    if input.len() > MAX_LENGTH {
        return Err(length_error(input.len()));
    }
    let input = apply_replacements(input);
    let output = remove_prefix(&input);
    Ok(fast_create(output).map(|mut source| {
        ext(&mut source, SLASH_LITERAL);
        ext(&mut source, "?");
        source
    }))
}

/// The patterns `parse.fastpaths` writes directly, with the pieces of their sources;
/// `**/` is `(?:{NO_DOTS}{GLOBSTAR}{SLASH_LITERAL})?`.
const FAST_PATHS: [(&str, &[&str]); 8] = [
    ("*", &[NO_DOTS, ONE_CHAR, STAR]),
    (".*", &[DOT_LITERAL, ONE_CHAR, STAR]),
    ("*.*", &[NO_DOTS, STAR, DOT_LITERAL, ONE_CHAR, STAR]),
    ("*/*", &[NO_DOTS, STAR, SLASH_LITERAL, ONE_CHAR, NO_DOTS_SLASH, STAR]),
    ("**", &[NO_DOTS, GLOBSTAR]),
    ("**/*", &["(?:", NO_DOTS, GLOBSTAR, SLASH_LITERAL, ")?", NO_DOTS_SLASH, ONE_CHAR, STAR]),
    (
        "**/*.*",
        &["(?:", NO_DOTS, GLOBSTAR, SLASH_LITERAL, ")?", NO_DOTS_SLASH, STAR, DOT_LITERAL, ONE_CHAR, STAR],
    ),
    ("**/.*", &["(?:", NO_DOTS, GLOBSTAR, SLASH_LITERAL, ")?", DOT_LITERAL, ONE_CHAR, STAR]),
];

fn fast_create(s: &[u16]) -> Option<U> {
    if let Some((_, pieces)) = FAST_PATHS.iter().find(|(pattern, _)| is(s, pattern)) {
        return Some(w(&pieces.concat()));
    }
    // `/^(.*?)\.(\w+)$/`: the last dot, followed by one or more word characters.
    let dot = s.iter().rposition(|&c| c == DOT)?;
    let (head, tail) = (slice(s, 0, dot), slice(s, dot + 1, s.len()));
    if tail.is_empty()
        || !tail.iter().all(|&c| is_word_unit(c))
        || head.iter().any(|&c| is_line_terminator(c))
    {
        return None;
    }
    let mut source = fast_create(head)?;
    ext(&mut source, DOT_LITERAL);
    source.extend_from_slice(tail);
    Some(source)
}

/// The regex fast path at the top of `parse` (`REGEX_SPECIAL_CHARS_BACKREF`).
pub(super) fn fast_regex(input: &[u16]) -> U {
    let mut out = U::with_capacity(input.len() * 2);
    let mut backslashes = false;
    let mut i = 0;
    while let Some(&c) = input.get(i) {
        let Some(run) = special_run(input, i) else {
            out.push(c);
            i += 1;
            continue;
        };
        if run.first == BACKSLASH {
            backslashes = true;
        }
        write_run(&mut out, &run);
        i = run.end;
    }
    if backslashes {
        out = collapse_backslashes(&out);
    }
    cat(&[&w("^(?:"), &out, &w(")$")])
}

/// A match of `(\\?)((\W)(\3*))`: an optional backslash, then a run of one non-word
/// character.
struct Run<'a> {
    whole: &'a [u16],
    first: u16,
    escaped: bool,
    /// How many times the character repeats after its first.
    repeats: usize,
    end: usize,
}

/// The run at `i`, or `None` at a word character.
fn special_run(input: &[u16], i: usize) -> Option<Run<'_>> {
    let c = *input.get(i)?;
    let next = input.get(i + 1).copied();
    let (escaped, first_at) = if c == BACKSLASH && next.is_some_and(|n| !is_word_unit(n)) {
        (true, i + 1)
    } else if !is_word_unit(c) {
        (false, i)
    } else {
        return None;
    };
    let first = input.get(first_at).copied().unwrap_or(c);
    let mut end = first_at + 1;
    while input.get(end) == Some(&first) {
        end += 1;
    }
    Some(Run { whole: slice(input, i, end), first, escaped, repeats: end - first_at - 1, end })
}

fn write_run(out: &mut U, run: &Run<'_>) {
    match run.first {
        BACKSLASH => out.extend_from_slice(run.whole),
        QMARK_CH => write_qmarks(out, run),
        DOT => ext(out, &DOT_LITERAL.repeat(run.repeats + 1)),
        STAR_CH => write_stars(out, run),
        _ => {
            if !run.escaped {
                out.push(BACKSLASH);
            }
            out.extend_from_slice(run.whole);
        }
    }
}

fn write_qmarks(out: &mut U, run: &Run<'_>) {
    if run.escaped {
        ext(out, r"\?");
        ext(out, &QMARK.repeat(run.repeats));
    } else {
        // At the start picomatch writes `qmarkNoDot`, which is QMARK with `dot: true`.
        ext(out, &QMARK.repeat(run.repeats + 1));
    }
}

fn write_stars(out: &mut U, run: &Run<'_>) {
    if run.escaped {
        ext(out, r"\*");
        if run.repeats > 0 {
            ext(out, STAR);
        }
    } else {
        ext(out, STAR);
    }
}

/// `output.replace(/\\+/g, m => m.length % 2 === 0 ? '\\\\' : '\\')`.
fn collapse_backslashes(out: &[u16]) -> U {
    let mut collapsed = U::with_capacity(out.len());
    let mut j = 0;
    while let Some(&c) = out.get(j) {
        if c != BACKSLASH {
            collapsed.push(c);
            j += 1;
            continue;
        }
        let run = out.get(j..).unwrap_or_default().iter().take_while(|&&x| x == BACKSLASH).count();
        collapsed.push(BACKSLASH);
        if run % 2 == 0 {
            collapsed.push(BACKSLASH);
        }
        j += run;
    }
    collapsed
}
