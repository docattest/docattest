"""The tests a test file declares, as pytest and unittest find them, read from syntax: a
function or method whose name starts with ``test`` is a test, titled by its name, with the
classes that enclose it named first; each case of it that ``parametrize`` or ``subTest``
writes is a test too, titled by its id, with the test named before it. A class is a suite."""

from __future__ import annotations

import ast
from collections.abc import Iterator
from typing import Literal, NamedTuple

from .parametrize import parametrized
from .subtests import subtests
from .tables import Scope, scope_of

#: What a declared thing is: a test function or method, a case of one that ``parametrize``
#: or ``subTest`` writes, or a class that holds tests (a suite).
DeclaredKind = Literal["test", "parametrized", "subtest", "suite"]


class Declared(NamedTuple):
    """A test, a case of one, or a class that holds tests (a suite), as a test file declares it."""

    kind: DeclaredKind
    title: str
    #: The classes and the test that enclose it, outermost first.
    describes: list[str]
    line: int
    #: The function a test or a case runs, or the class.
    node: ast.AST


def declared(tree: ast.Module) -> list[Declared]:
    """Every test, case and suite of a test file, in source order."""
    return list(_visit(tree.body, [], tree, scope_of(tree.body, tree)))


def cited(declared: Declared, title: str, describes: list[str]) -> bool:
    """Whether a citation of ``describes > title`` names a declared test; a citation that
    names no enclosing classes names the title wherever it is."""
    if declared.title != title:
        return False
    return not describes or describes == declared.describes


def _visit(
    body: list[ast.stmt], classes: list[ast.ClassDef], tree: ast.Module, scope: Scope
) -> Iterator[Declared]:
    describes = [c.name for c in classes]
    for node in body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.startswith("test"):
                yield Declared("test", node.name, describes, node.lineno, node)
                inside = [*describes, node.name]
                for case in parametrized(node, classes, tree, scope):
                    yield Declared("parametrized", case.title, inside, case.line, node)
                for case in subtests(node, scope):
                    yield Declared("subtest", case.title, inside, case.line, node)
        elif isinstance(node, ast.ClassDef):
            yield Declared("suite", node.name, describes, node.lineno, node)
            yield from _visit(node.body, [*classes, node], tree, scope)
        elif isinstance(node, (ast.If, ast.Try, ast.With)):
            yield from _visit(node.body, classes, tree, scope)
            yield from _visit(
                node.orelse if isinstance(node, (ast.If, ast.Try)) else [], classes, tree, scope
            )
