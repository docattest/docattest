// Package cells reads the cells of a cases table. A runner gets each cell as the page
// writes it, code spans with their backticks; this package takes out what docattest itself
// reads from a cell. It imports only the standard library, so a module whose setup package
// imports it needs no other module for it.
package cells

import "strings"

// CodeSpans returns the content of every code span in a cell, in order, as a fact over the
// cell's column compares it: a span opens with a run of backticks and closes with the next
// run of as many, and its content is trimmed of white space. As CommonMark has it, a
// backtick escaped with a backslash opens nothing, and the backticks of an HTML tag or
// comment, of an autolink, or of a link's destination or title belong to those. A cell
// with no code span gives none.
func CodeSpans(cell string) []string {
	var spans []string
	var openers brackets
	for at := 0; at < len(cell); {
		switch {
		case cell[at] == '\\' && at+1 < len(cell) && isASCIIPunctuation(cell[at+1]):
			at += 2
		case cell[at] == '<':
			at += max(1, rawLength(cell[at:]))
		case strings.HasPrefix(cell[at:], "!["):
			openers.open(true)
			at += 2
		case cell[at] == '[':
			openers.open(false)
			at++
		case cell[at] == ']':
			at += 1 + openers.close(cell[at+1:])
		case cell[at] == '`':
			span, length, ok := codeSpan(cell[at:])
			if ok {
				spans = append(spans, span)
			}
			at += length
		default:
			at++
		}
	}
	return spans
}

// codeSpan reads the code span that `text` starts with, which begins with a backtick: its
// content, trimmed, and its length. When no run closes it, the opening run is only text,
// and its length is that run's.
func codeSpan(text string) (content string, length int, ok bool) {
	fence := runLength(text, 0)
	end := closingRun(text, fence, fence)
	if end < 0 {
		return "", fence, false
	}
	return strings.TrimFunc(text[fence:end], isSpace), end + fence, true
}

// runLength is the length of the run of backticks that starts at `at`.
func runLength(text string, at int) int {
	end := at
	for end < len(text) && text[end] == '`' {
		end++
	}
	return end - at
}

// closingRun is where the first run of exactly `fence` backticks at or after `from` starts,
// or -1 when there is none. A longer or shorter run is part of the span's content.
func closingRun(text string, from, fence int) int {
	for at := from; at < len(text); {
		if text[at] != '`' {
			at++
			continue
		}
		run := runLength(text, at)
		if run == fence {
			return at
		}
		at += run
	}
	return -1
}

// isASCIIPunctuation says whether a backslash before c escapes it, as CommonMark has it.
func isASCIIPunctuation(c byte) bool {
	return strings.IndexByte("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~", c) >= 0
}

// isSpace is the white space docattest trims from a value: JavaScript's `\s`, which
// differs from unicode.IsSpace in U+0085 (not space) and U+FEFF (space).
func isSpace(r rune) bool {
	switch r {
	case '\t', '\n', '\v', '\f', '\r', ' ', '\u00a0', '\u1680', '\u2028', '\u2029', '\u202f',
		'\u205f', '\u3000', '\ufeff':
		return true
	}
	return r >= '\u2000' && r <= '\u200a'
}
