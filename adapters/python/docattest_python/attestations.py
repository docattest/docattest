"""The attestations the core sends (``attestations.json``), as typed values.

``server/params.py`` reads them from a request, where a missing or mistyped field is the
core's mistake; everything after trusts these types. Only the fields this adapter reads are
kept.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal


@dataclass(frozen=True)
class Example:
    """A fenced block, or an example in a docstring."""

    id: str
    page: str
    line: int
    lang: str
    code: str
    network: bool
    weak: bool
    #: For a docstring's example: the function or class it documents.
    owner: str | None
    #: The adapter's own info-string words the block carries (``continue``).
    tags: tuple[str, ...]
    #: A problem found while planning; such an example is never compiled or run.
    error: str | None
    kind: Literal["example"] = "example"


@dataclass(frozen=True)
class Fact:
    """A literal on the page checked against an expression over a surface."""

    id: str
    page: str
    line: int
    expression: str
    surface: str
    error: str | None
    kind: Literal["fact"] = "fact"


@dataclass(frozen=True)
class CasesRow:
    line: int
    name: str
    values: dict[str, str]


@dataclass(frozen=True)
class Cases:
    """A table run row by row by a runner of the setup module."""

    id: str
    page: str
    line: int
    runner: str
    header: tuple[str, ...]
    rows: tuple[CasesRow, ...]
    error: str | None
    kind: Literal["cases"] = "cases"


@dataclass(frozen=True)
class TestCitation:
    """A sentence tied to a test that already exists; it is never run."""

    id: str
    page: str
    line: int
    file: str
    title: str
    #: Titles that enclose the test, outermost first; empty matches any.
    describes: tuple[str, ...]
    error: str | None
    kind: Literal["test"] = "test"


Attestation = Example | Fact | Cases | TestCitation
