"""Literal tables in test code: a list or tuple written in place, or assigned once to a
name of the test function or of the module, which the cases of a test are read from."""

from __future__ import annotations

import ast
from collections import Counter
from collections.abc import Iterator
from typing import NamedTuple

#: The names of one scope that are assigned once, each to the expression it is assigned.
Scope = dict[str, ast.expr]
#: The methods by which a list, a dict or a set changes in place.
_MUTATORS = frozenset(
    "append extend insert remove pop clear sort reverse update setdefault popitem add discard".split()
)


def scope_of(statements: list[ast.stmt], whole: ast.AST) -> Scope:
    """The names one of ``statements`` assigns, alone, that nothing else in ``whole``
    assigns or changes in place (``CASES.append``, ``CASES[0] = …``)."""
    assigned: Scope = dict(filter(None, map(_assignment, statements)))
    if not assigned:
        return {}
    counts = Counter(_changed_names(whole))
    return {name: value for name, value in assigned.items() if counts.get(name, 0) == 1}


class Case(NamedTuple):
    """A case of a test, read from a table: its title, and the line that writes it."""

    title: str
    line: int


#: What ``literal`` gives for an expression that is not a literal.
NOT_LITERAL = object()


def literal(node: ast.expr) -> object:
    """The value of a literal, as ``ast.literal_eval`` reads it, or ``NOT_LITERAL``."""
    try:
        return ast.literal_eval(node)
    except (ValueError, SyntaxError, TypeError, RecursionError):
        return NOT_LITERAL


def table(node: ast.expr, scopes: list[Scope]) -> list[ast.expr] | None:
    """The elements of a literal table, ``node`` itself or the name it reads, looked up in
    ``scopes`` innermost first; None when it is no such table."""
    if isinstance(node, ast.Name):
        for scope in scopes:
            if node.id in scope:
                return table(scope[node.id], [])
        return None
    if isinstance(node, (ast.List, ast.Tuple)) and not any(isinstance(e, ast.Starred) for e in node.elts):
        return list(node.elts)
    return None


def mapping(node: ast.expr, scopes: list[Scope]) -> ast.Dict | None:
    """A literal dict, ``node`` itself or the name it reads, looked up as ``table`` does."""
    if isinstance(node, ast.Name):
        for scope in scopes:
            if node.id in scope:
                return mapping(scope[node.id], [])
        return None
    if isinstance(node, ast.Dict) and all(key is not None for key in node.keys):
        return node
    return None


def _assignment(statement: ast.stmt) -> tuple[str, ast.expr] | None:
    """The name a statement assigns, alone, and its value; None for any other statement."""
    if isinstance(statement, ast.Assign) and len(statement.targets) == 1:
        target = statement.targets[0]
        return (target.id, statement.value) if isinstance(target, ast.Name) else None
    if isinstance(statement, ast.AnnAssign) and isinstance(statement.target, ast.Name) and statement.value:
        return statement.target.id, statement.value
    return None


def _changed_names(whole: ast.AST) -> Iterator[str]:
    """Each time a name is bound or changed in place in ``whole``."""
    for node in ast.walk(whole):
        name = _changed_name(node)
        if name is not None:
            yield name


def _changed_name(node: ast.AST) -> str | None:
    """The name a node binds or changes in place: a name stored or deleted, one whose item
    is, or the receiver of a call of a method that changes it."""
    if isinstance(node, (ast.Name, ast.Subscript)) and isinstance(node.ctx, (ast.Store, ast.Del)):
        target = node if isinstance(node, ast.Name) else node.value
        return target.id if isinstance(target, ast.Name) else None
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr in _MUTATORS:
        receiver = node.func.value
        return receiver.id if isinstance(receiver, ast.Name) else None
    return None
