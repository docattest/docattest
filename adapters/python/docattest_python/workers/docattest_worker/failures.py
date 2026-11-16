"""Describing a failure: a raised exception as a result."""

from __future__ import annotations

from types import TracebackType

from .asserts import OPERATORS, Comparison, comparison_of
from .messages import Sides
from .values import safe_str, show


class Failure(Sides):
    message: str


def first_line(text: str) -> str:
    return text.split("\n")[0] if text else ""


def line_in(tb: TracebackType | None, filename: str) -> int | None:
    """The last line of ``filename`` a traceback passed through."""
    line = None
    while tb is not None:
        if tb.tb_frame.f_code.co_filename == filename:
            line = tb.tb_lineno
        tb = tb.tb_next
    return line


def describe_error(
    err: BaseException,
    filename: str | None = None,
    source: str | None = None,
    where: str | None = "block line",
) -> Failure:
    """A raised exception as a result: a message, and both sides of a failed comparison."""
    line = _failing_line(err, filename)
    at = " (%s %d)" % (where, line) if line and where else ""
    if isinstance(err, AssertionError):
        return _describe_assertion(err, line, source, at)
    if isinstance(err, SyntaxError):
        return {"message": "SyntaxError: %s%s" % (err.msg, at)}
    name = type(err).__name__
    text = first_line(safe_str(err))
    return {"message": "%s: %s%s" % (name, text, at) if text else "%s%s" % (name, at)}


def failure_message(err: BaseException, filename: str | None = None, where: str | None = None) -> str:
    """What a raised exception is said to be, in one line."""
    return describe_error(err, filename, where=where)["message"]


def _failing_line(err: BaseException, filename: str | None) -> int | None:
    """The line of ``filename`` where the error was raised, or where its syntax broke."""
    line = line_in(err.__traceback__, filename) if filename else None
    if isinstance(err, SyntaxError) and err.filename == filename and err.lineno:
        line = err.lineno
    return line


def _describe_assertion(err: AssertionError, line: int | None, source: str | None, at: str) -> Failure:
    comparison = comparison_of(err)
    if comparison is not None:
        return _describe_comparison(err, comparison, at)
    text = first_line(safe_str(err))
    if not text and line and source is not None:
        lines = source.split("\n")
        text = lines[line - 1].strip() if 0 < line <= len(lines) else ""
    return {"message": "AssertionError: %s%s" % (text, at) if text else "AssertionError%s" % at}


def _describe_comparison(err: AssertionError, comparison: Comparison, at: str) -> Failure:
    """A failed ``assert a <op> b`` the worker rewrote: both sides, and what was wrong."""
    expected, actual = show(comparison.right), show(comparison.left)
    if comparison.custom_message:
        text = first_line(safe_str(err))
    elif comparison.op == "Eq":
        text = "expected %s, got %s" % (expected, actual)
    elif comparison.op == "NotEq":
        text = "expected anything but %s, got %s" % (expected, actual)
    else:
        text = "%s %s %s is false" % (actual, OPERATORS[comparison.op][1], expected)
    return {"expected": expected, "actual": actual, "message": "AssertionError: %s%s" % (text, at)}
