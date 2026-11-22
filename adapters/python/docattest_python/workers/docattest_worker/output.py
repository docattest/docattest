"""Trailing ``# Output:`` and ``# Unordered output:`` comments on Python example blocks,
matching Go's testable example convention."""

from __future__ import annotations

import json
import re
from typing import NamedTuple

from .messages import Verdict

_HEADER = re.compile(r"^(unordered output|output):(.*)$", re.I)


class ExpectedOutput(NamedTuple):
    text: str
    unordered: bool


def expected_output(code: str) -> ExpectedOutput | None:
    """Extract a trailing ``# Output:`` or ``# Unordered output:`` comment block from code."""
    comments = _trailing_comments(code)
    for idx, line in enumerate(comments):
        match = _HEADER.match(line)
        if match is not None:
            return _build_expected(match, comments[idx + 1 :])
    return None


def _trailing_comments(code: str) -> list[str]:
    lines = code.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    end = len(lines)
    while end > 0 and not lines[end - 1].strip():
        end -= 1
    start = end
    while start > 0 and lines[start - 1].lstrip().startswith("#"):
        start -= 1
    return [_strip_hash(line) for line in lines[start:end]]


def _build_expected(match: re.Match[str], rest: list[str]) -> ExpectedOutput:
    unordered = match.group(1).lower().startswith("unordered")
    first = match.group(2).removeprefix(" ")
    parts = [first, *rest] if first else rest
    return ExpectedOutput("\n".join(parts), unordered)


def _strip_hash(line: str) -> str:
    body = line.lstrip()[1:]
    return body[1:] if body.startswith(" ") else body


def compare_output(got: str, want: ExpectedOutput) -> Verdict:
    """Compare captured stdout against what a ``# Output:`` comment states."""
    actual = got.replace("\r\n", "\n").strip()
    expected = want.text.strip()
    if want.unordered:
        actual = "\n".join(sorted(actual.split("\n")))
        expected = "\n".join(sorted(expected.split("\n")))
    if actual == expected:
        return {"status": "pass", "message": "printed the output it states"}
    return {
        "status": "fail",
        "message": f"expected output {json.dumps(expected)}, got {json.dumps(actual)}",
        "expected": expected,
        "actual": actual,
    }
