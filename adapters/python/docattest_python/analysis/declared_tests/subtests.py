"""The cases of a test that ``with self.subTest(...)`` writes, by what unittest prints for
each (``[big discount]``, ``(name='big discount')``), read from syntax.

A case's title is its message (``subTest("big discount")``), its parameters as unittest
writes them (``subTest(name="big discount")`` is ``name='big discount'``), or both
(``big discount (name='big discount')``). Each argument is a literal, or is read from the
row of a ``for`` loop over a literal table: the loop's names (``for name, price in
CASES``), an item of one (``case["name"]``, ``case[0]``), or a key of a dict's
``.items()``. A subTest inside another, or one whose title cannot be read for every row,
gives no cases.
"""

from __future__ import annotations

import ast
from collections.abc import Iterator
from typing import NamedTuple, cast

from .tables import NOT_LITERAL, Case, Scope, literal, mapping, scope_of, table

#: The nodes a loop's names stand for in one row of its table.
Bindings = dict[str, ast.expr]


class _SubTest(NamedTuple):
    """A ``with self.subTest(...)``: its call, the line of the ``with``, and the innermost
    ``for`` loop around it, if any."""

    call: ast.Call
    line: int
    loop: ast.For | None


def subtests(function: ast.FunctionDef | ast.AsyncFunctionDef, module: Scope) -> list[Case]:
    """The cases of a test method, from the subtests it starts."""
    params = function.args.posonlyargs + function.args.args
    if not params:
        return []
    scopes = [scope_of(function.body, function), module]
    found: dict[str, Case] = {}
    for subtest in _subtests(function.body, params[0].arg, None):
        for case in _cases(subtest, scopes):
            found.setdefault(case.title, case)
    return list(found.values())


def _subtests(body: list[ast.stmt], receiver: str, loop: ast.For | None) -> Iterator[_SubTest]:
    """The subtests a body starts, outside any other subtest and any nested function."""
    for statement in body:
        if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            continue
        call = _subtest_call(statement, receiver)
        if call is not None:
            yield _SubTest(call, statement.lineno, loop)
            continue
        inner = statement if isinstance(statement, ast.For) else loop
        for field in ("body", "orelse", "finalbody", "handlers"):
            children: object = getattr(statement, field, None)
            if isinstance(children, list):
                yield from _subtests(_statements(cast("list[object]", children)), receiver, inner)


def _statements(children: list[object]) -> list[ast.stmt]:
    """A block's statements; an ``except`` clause stands for its own."""
    out: list[ast.stmt] = []
    for child in children:
        if isinstance(child, ast.ExceptHandler):
            out.extend(child.body)
        elif isinstance(child, ast.stmt):
            out.append(child)
    return out


def _subtest_call(statement: ast.stmt, receiver: str) -> ast.Call | None:
    """The ``self.subTest(...)`` call a ``with`` statement enters, if it is one."""
    if not isinstance(statement, (ast.With, ast.AsyncWith)):
        return None
    for item in statement.items:
        call = item.context_expr
        if (
            isinstance(call, ast.Call)
            and isinstance(call.func, ast.Attribute)
            and call.func.attr == "subTest"
            and isinstance(call.func.value, ast.Name)
            and call.func.value.id == receiver
        ):
            return call
    return None


def _cases(subtest: _SubTest, scopes: list[Scope]) -> list[Case]:
    """A subtest's case for each row of its loop's table, or its one case outside a loop."""
    if subtest.loop is None:
        title = _title(subtest.call, {})
        return [] if title is None else [Case(title, subtest.line)]
    rows = _rows(subtest.loop.iter, scopes)
    if rows is None:
        return []
    cases: list[Case] = []
    for row in rows:
        bindings = _bind(subtest.loop.target, row)
        title = None if bindings is None else _title(subtest.call, bindings)
        if title is None:
            return []
        cases.append(Case(title, row.lineno))
    return cases


def _rows(node: ast.expr, scopes: list[Scope]) -> list[ast.expr] | None:
    """The rows a loop goes through: a literal table's, or ``(key, value)`` of a literal
    dict's ``.items()``."""
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr == "items":
        found = mapping(node.func.value, scopes) if not node.args else None
        if found is None:
            return None
        pairs = zip([key for key in found.keys if key is not None], found.values)
        return [
            ast.Tuple(elts=[k, v], ctx=ast.Load(), lineno=k.lineno, col_offset=k.col_offset) for k, v in pairs
        ]
    return table(node, scopes)


def _bind(target: ast.expr, row: ast.expr) -> Bindings | None:
    """What the loop's names stand for in one row; None when the row has another shape."""
    if isinstance(target, ast.Name):
        return {target.id: row}
    if not isinstance(target, (ast.Tuple, ast.List)) or not isinstance(row, (ast.Tuple, ast.List)):
        return None
    if len(target.elts) != len(row.elts):
        return None
    bindings: Bindings = {}
    for name, element in zip(target.elts, row.elts):
        inner = _bind(name, element)
        if inner is None:
            return None
        bindings.update(inner)
    return bindings


def _title(call: ast.Call, bindings: Bindings) -> str | None:
    """What unittest prints for a subtest, or None when an argument cannot be read."""
    message_node = call.args[0] if call.args else None
    params: list[str] = []
    for keyword in call.keywords:
        if keyword.arg is None:
            return None
        if keyword.arg == "msg":
            message_node = keyword.value
            continue
        value = _value(keyword.value, bindings)
        if value is NOT_LITERAL:
            return None
        params.append(f"{keyword.arg}={value!r}")
    if message_node is None:
        return subtest_title(None, params)
    message = _value(message_node, bindings)
    if message is NOT_LITERAL:
        return None
    return subtest_title(str(message), params)


def subtest_title(message: str | None, params: list[str]) -> str | None:
    """What unittest prints for a subtest, without its brackets: its message, its parameters
    (each ``name=repr(value)``), or both; None for one with neither, which has no title."""
    if message is None:
        return ", ".join(params) or None
    return f"{message} ({', '.join(params)})" if params else message


def _value(node: ast.expr, bindings: Bindings) -> object:
    """An argument's value: a literal, a loop name's, or an item of a loop name's."""
    if isinstance(node, ast.Name) and node.id in bindings:
        return literal(bindings[node.id])
    if isinstance(node, ast.Subscript) and isinstance(node.value, ast.Name) and node.value.id in bindings:
        item = _item(bindings[node.value.id], literal(node.slice))
        return NOT_LITERAL if item is None else literal(item)
    return literal(node)


def _item(container: ast.expr, key: object) -> ast.expr | None:
    """The element of a literal dict, tuple or list that ``key`` picks."""
    if isinstance(container, ast.Dict):
        for k, v in zip(container.keys, container.values):
            if k is not None and literal(k) == key:
                return v
        return None
    if isinstance(container, (ast.Tuple, ast.List)) and isinstance(key, int) and not isinstance(key, bool):
        return container.elts[key] if -len(container.elts) <= key < len(container.elts) else None
    return None
