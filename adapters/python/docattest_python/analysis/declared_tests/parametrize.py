"""The cases of a test that ``@pytest.mark.parametrize`` writes, by the ids pytest prints
in brackets (``test_total[big discount]``), read from syntax.

An id is the one written out (``ids=[...]``, ``pytest.param(..., id="...")``) or the one
pytest makes of a literal value: a string or bytes, with what is not printable ASCII
escaped; a number, ``True``, ``False`` or ``None`` as ``str`` writes it; ``<argname><index>`` for a
literal tuple, list, set or dict. Stacked decorators join their ids with ``-``, the one
nearest the function first, then those of its classes. A parametrize whose ids cannot all
be read this way (a value that is not a literal, ids that a function makes, or ids that
repeat, which pytest renames by rules that change between its versions) gives no cases,
and nor does a test under a ``pytestmark`` that parametrizes.
"""

from __future__ import annotations

import ast
import itertools
from typing import TypeGuard

from .tables import NOT_LITERAL, Case, Scope, literal, table


def parametrized(
    function: ast.FunctionDef | ast.AsyncFunctionDef,
    classes: list[ast.ClassDef],
    module: ast.Module,
    scope: Scope,
) -> list[Case]:
    """The cases of a test function, nested in ``classes`` (outermost first), from the
    parametrize decorators on it and on its classes."""
    if any(_marks_parametrize(body) for body in [module.body, *(c.body for c in classes)]):
        return []
    per_mark = _ids_of_each(_marks(function, classes), scope)
    if not per_mark:
        return []
    return [
        Case("-".join(case.title for case in combination), combination[0].line)
        for combination in itertools.product(*per_mark)
    ]


def _marks(function: ast.FunctionDef | ast.AsyncFunctionDef, classes: list[ast.ClassDef]) -> list[ast.Call]:
    """The parametrize decorators of a test, in the order pytest joins their ids: the one
    nearest the function first, then those of its classes, innermost first."""
    owners: list[ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef] = [function, *reversed(classes)]
    return [d for owner in owners for d in reversed(owner.decorator_list) if _is_parametrize(d)]


def _ids_of_each(marks: list[ast.Call], scope: Scope) -> list[list[Case]]:
    """The ids of each parametrize, or none at all when those of one cannot be read."""
    per_mark = [_ids(mark, scope) for mark in marks]
    readable = [ids for ids in per_mark if ids is not None]
    return readable if len(readable) == len(per_mark) else []


def _is_parametrize(node: ast.expr) -> TypeGuard[ast.Call]:
    """Whether a decorator is ``pytest.mark.parametrize(...)`` or ``mark.parametrize(...)``."""
    if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Attribute):
        return False
    owner = node.func.value
    marked = (isinstance(owner, ast.Attribute) and owner.attr == "mark") or (
        isinstance(owner, ast.Name) and owner.id == "mark"
    )
    return node.func.attr == "parametrize" and marked


def _marks_parametrize(body: list[ast.stmt]) -> bool:
    """Whether a module or a class body assigns a ``pytestmark`` that parametrizes."""
    for statement in body:
        if isinstance(statement, (ast.Assign, ast.AnnAssign)):
            targets = statement.targets if isinstance(statement, ast.Assign) else [statement.target]
            named = any(isinstance(t, ast.Name) and t.id == "pytestmark" for t in targets)
            if named and any(
                isinstance(n, ast.Attribute) and n.attr == "parametrize" for n in ast.walk(statement)
            ):
                return True
    return False


def _ids(mark: ast.Call, scope: Scope) -> list[Case] | None:
    """The ids of one parametrize, or None when they cannot all be read."""
    names = _argnames(_argument(mark, 0, "argnames"))
    values = _argument(mark, 1, "argvalues")
    rows = table(values, [scope]) if values is not None else None
    if names is None or rows is None:
        return None
    written = _written_ids(_argument(mark, None, "ids"), scope, len(rows))
    if written is None:
        return None
    cases: list[Case] = []
    for index, (row, given) in enumerate(zip(rows, written)):
        title = _case_id(row, names, index, given)
        if title is None:
            return None
        cases.append(Case(title, row.lineno))
    titles = [case.title for case in cases]
    return cases if len(set(titles)) == len(titles) else None


def _argument(call: ast.Call, position: int | None, keyword: str) -> ast.expr | None:
    for k in call.keywords:
        if k.arg == keyword:
            return k.value
    if position is not None and position < len(call.args):
        return call.args[position]
    return None


def _argnames(node: ast.expr | None) -> list[str] | None:
    """``"a,b"``, or a list or tuple of names."""
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        return [name.strip() for name in node.value.split(",") if name.strip()]
    if isinstance(node, (ast.List, ast.Tuple)):
        names = [e.value for e in node.elts if isinstance(e, ast.Constant) and isinstance(e.value, str)]
        return names if len(names) == len(node.elts) else None
    return None


def _written_ids(node: ast.expr | None, scope: Scope, count: int) -> list[ast.expr | None] | None:
    """The id written for each row (None where it is left to pytest), or None when the
    ids are not a literal list as long as the rows."""
    if node is None:
        unwritten: list[ast.expr | None] = [None] * count
        return unwritten
    ids = table(node, [scope])
    if ids is None or len(ids) != count:
        return None
    return [None if _is_none(i) else i for i in ids]


def _case_id(row: ast.expr, names: list[str], index: int, written: ast.expr | None) -> str | None:
    """A row's id, as pytest chooses it: ``pytest.param``'s ``id``, then the one in
    ``ids``, then one made of the row's values."""
    explicit = _explicit_id(row)
    if explicit is not None:
        return _id_of_value(explicit)
    if written is not None:
        return _id_of_value(written)
    values = _row_values(row, names)
    if len(values) != len(names):
        return None
    parts = [_generated(value, name, index) for value, name in zip(values, names)]
    return None if None in parts else "-".join(p for p in parts if p is not None)


def _explicit_id(row: ast.expr) -> ast.expr | None:
    """The id a row's ``pytest.param(..., id=...)`` writes, unless that is ``None``."""
    if not _is_param(row):
        return None
    explicit = _argument(row, None, "id")
    return None if explicit is None or _is_none(explicit) else explicit


def _row_values(row: ast.expr, names: list[str]) -> list[ast.expr]:
    """The values a row gives the test's arguments: a ``pytest.param``'s arguments, the row
    itself when there is one argument, else the items of its tuple or list."""
    if _is_param(row):
        return list(row.args)
    if len(names) == 1:
        return [row]
    return list(row.elts) if isinstance(row, (ast.Tuple, ast.List)) else []


def _is_param(node: ast.expr) -> TypeGuard[ast.Call]:
    """Whether a row is ``pytest.param(...)`` or ``param(...)``."""
    if not isinstance(node, ast.Call):
        return False
    func = node.func
    return (isinstance(func, ast.Attribute) and func.attr == "param") or (
        isinstance(func, ast.Name) and func.id == "param"
    )


def _is_none(node: ast.expr) -> bool:
    """Whether an id is written as ``None``, which leaves it to pytest."""
    return isinstance(node, ast.Constant) and node.value is None


def _generated(node: ast.expr, name: str, index: int) -> str | None:
    """The id pytest makes of one value: of a scalar, by ``_id_of_value``; of a literal
    collection, ``<argname><index>``."""
    scalar = _id_of_value(node)
    if scalar is not None:
        return scalar
    return f"{name}{index}" if literal(node) is not NOT_LITERAL else None


def _id_of_value(node: ast.expr) -> str | None:
    """The id of a literal string, bytes, number, bool or None, as pytest writes it."""
    value = literal(node)
    if isinstance(value, str):
        return value.encode("unicode_escape").decode("ascii")
    if isinstance(value, bytes):
        return value.decode("ascii", "backslashreplace").translate(_NON_PRINTABLE)
    if value is None or isinstance(value, (bool, int, float, complex)):
        return str(value)
    return None


#: How pytest escapes what is not printable ASCII in bytes.
_NON_PRINTABLE = {i: f"\\x{i:02x}" for i in range(128) if i not in range(32, 127)} | {
    ord("\t"): "\\t",
    ord("\r"): "\\r",
    ord("\n"): "\\n",
}
