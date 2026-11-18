"""The cells of a cases table. A runner gets each cell as the page writes it, code spans
with their backticks; ``code_spans`` takes out what docattest itself reads from a cell.

Standard library only, so a setup module can import it wherever the package is installed.
"""

from __future__ import annotations

import re

__all__ = ["code_spans"]

# The white space docattest trims from a value: JavaScript's ``\s``, which differs from
# ``str.isspace`` in U+001C to U+001F and U+0085 (not space) and U+FEFF (space).
_SPACE = "\t\n\v\f\r \u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000\ufeff"
_TRIMMED = re.compile(f"^[{_SPACE}]+|[{_SPACE}]+$")
_ASCII_PUNCTUATION = frozenset("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~")


def code_spans(cell: str) -> list[str]:
    """The content of every code span in a cell, in order, as a fact over the cell's column
    compares it: a span opens with a run of backticks and closes with the next run of as
    many, and its content is trimmed of white space. As CommonMark has it, a backtick
    escaped with a backslash opens nothing, and the backticks of an HTML tag or comment, of
    an autolink, or of a link's destination or title belong to those. A cell with no code
    span gives none."""
    spans: list[str] = []
    openers = _Brackets()
    at = 0
    while at < len(cell):
        char = cell[at]
        if char == "\\" and cell[at + 1 : at + 2] in _ASCII_PUNCTUATION:
            at += 2
        elif char == "<":
            at += max(1, _raw_length(cell[at:]))
        elif cell.startswith("![", at):
            openers.open(image=True)
            at += 2
        elif char == "[":
            openers.open(image=False)
            at += 1
        elif char == "]":
            at += 1 + openers.close(cell[at + 1 :])
        elif char == "`":
            at += _code_span(cell[at:], spans)
        else:
            at += 1
    return spans


def _code_span(text: str, spans: list[str]) -> int:
    """Read the code span that ``text`` starts with, which begins with a backtick, into
    ``spans``, and return its length. When no run closes it, the opening run is only text,
    and its length is that run's."""
    fence = _run_length(text, 0)
    end = _closing_run(text, fence, fence)
    if end < 0:
        return fence
    spans.append(_TRIMMED.sub("", text[fence:end]))
    return end + fence


def _run_length(text: str, at: int) -> int:
    """The length of the run of backticks that starts at ``at``."""
    end = at
    while end < len(text) and text[end] == "`":
        end += 1
    return end - at


def _closing_run(text: str, start: int, fence: int) -> int:
    """Where the first run of exactly ``fence`` backticks at or after ``start`` begins, or -1
    when there is none. A longer or shorter run is part of the span's content."""
    at = start
    while at < len(text):
        if text[at] != "`":
            at += 1
            continue
        run = _run_length(text, at)
        if run == fence:
            return at
        at += run
    return -1


# --- what CommonMark reads instead of a code span when it starts first ------------------

# An autolink, or an HTML tag, comment, processing instruction, declaration or CDATA
# section. The patterns are the spec's own.
_TAG_NAME = r"[A-Za-z][A-Za-z0-9-]*"
_WHITE = r"[ \t\n]"
_ATTRIBUTE_VALUE = r"""(?:[^"'=<>`\x00-\x20]+|'[^']*'|"[^"]*")"""
_ATTRIBUTE = rf"{_WHITE}+[A-Za-z_:][A-Za-z0-9_.:-]*(?:{_WHITE}*={_WHITE}*{_ATTRIBUTE_VALUE})?"
_DOMAIN_LABEL = r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?"
_RAW = re.compile(
    "|".join(
        [
            r"<[A-Za-z][A-Za-z0-9+.-]{1,31}:[^<>\x00-\x20]*>",
            rf"<[A-Za-z0-9.!#$%&'*+/=?^_`{{|}}~-]+@{_DOMAIN_LABEL}(?:\.{_DOMAIN_LABEL})*>",
            rf"<{_TAG_NAME}(?:{_ATTRIBUTE})*{_WHITE}*/?>",
            rf"</{_TAG_NAME}{_WHITE}*>",
            r"<!-->",
            r"<!--->",
            r"<!--(?s:.)*?-->",
            r"<\?(?s:.)*?\?>",
            r"<![A-Za-z][^>]*>",
            r"<!\[CDATA\[(?s:.)*?\]\]>",
        ]
    )
)


def _raw_length(text: str) -> int:
    """The length of the autolink or the HTML that starts ``text``, or 0."""
    found = _RAW.match(text)
    return found.end() if found else 0


# --- links --------------------------------------------------------------------------------

# What makes a bracketed text a link: ``(destination "title")``, either part optional, as
# CommonMark has it. Its backticks are the link's, not a code span's.
_ESCAPED = r"\\[!-/:-@\[-`{-~]|\\"
_DESTINATION = (
    rf"<(?:[^<>\n\\]|{_ESCAPED})*>|"
    rf"(?:[^\x00-\x20()\\]|{_ESCAPED}|\((?:[^\x00-\x20()\\]|{_ESCAPED})*\))+"
)
_TITLE = rf""""(?:[^"\\]|{_ESCAPED})*"|'(?:[^'\\]|{_ESCAPED})*'|\((?:[^()\\]|{_ESCAPED})*\)"""
_LINK_TAIL = re.compile(rf"\({_WHITE}*(?:{_DESTINATION})?(?:{_WHITE}+(?:{_TITLE}))?{_WHITE}*\)")


class _Brackets:
    """The ``[`` and ``![`` not yet closed, innermost last: each a link's or an image's
    opener, and whether a later ``]`` may still close it into one."""

    def __init__(self) -> None:
        self._openers: list[tuple[bool, bool]] = []

    def open(self, *, image: bool) -> None:
        self._openers.append((image, True))

    def close(self, rest: str) -> int:
        """Close the innermost opener with the ``]`` that ``rest`` follows; the length of
        the link's tail when the two make a link, else 0. A link cannot hold a link, so the
        link openers around one that closes into a link become text."""
        if not self._openers:
            return 0
        image, active = self._openers.pop()
        tail = _LINK_TAIL.match(rest)
        if not active or tail is None:
            return 0
        if not image:
            self._openers = [(outer_image, outer_image and open_) for outer_image, open_ in self._openers]
        return tail.end()
