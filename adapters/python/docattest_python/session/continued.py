"""Blocks that go on from the ones before them on their page (``continue``).

The core knows nothing of this. It passes the word on in ``tags`` because ``hello``
declared it, and it names every example of a page (in ``open``, and again in ``compile``
and ``symbols``), each with its page and line: that is all this needs.
"""

from __future__ import annotations

import ast

from .. import language
from ..analysis.names import bound_names
from ..attestations import Attestation, Example


class ContinuedBlocks:
    """Every example the core has named, by page and line."""

    def __init__(self) -> None:
        self._examples: dict[str, dict[int, Example]] = {}

    def remember(self, attestations: list[Attestation] | list[Example]) -> None:
        for e in attestations:
            if isinstance(e, Example) and e.error is None:
                self._examples.setdefault(e.page, {})[e.line] = e

    def before(self, e: Example) -> list[Example] | None:
        """The blocks ``e`` goes on from, in page order: the examples before it on its
        page, back to the nearest one that is not itself tagged ``continue``. Empty when
        ``e`` is not tagged ``continue``; None when it is and nothing comes before it."""
        if language.CONTINUE not in e.tags:
            return []
        on_page = self._examples.get(e.page, {})
        chain: list[Example] = []
        for line in sorted((n for n in on_page if n < e.line), reverse=True):
            chain.append(on_page[line])
            if language.CONTINUE not in on_page[line].tags:
                break
        chain.reverse()
        return chain or None


def example_tree(e: Example) -> ast.Module | None:
    """The statements of an example, a session's among them; None when it does not parse."""
    code = e.code
    try:
        return ast.parse(language.session_source(code) if language.is_session(e.lang, code) else code)
    except (SyntaxError, ValueError, RecursionError):
        return None


def inherited_names(before: list[Example]) -> set[str] | None:
    """The names the earlier blocks of a session bind; None when that cannot be told (one
    of them does not parse, or imports ``*``), and then no name is questioned."""
    names: set[str] = set()
    for earlier in before:
        tree = example_tree(earlier)
        if tree is None:
            return None
        if any(isinstance(n, ast.ImportFrom) and any(a.name == "*" for a in n.names) for n in ast.walk(tree)):
            return None
        names |= bound_names(tree)
    return names
