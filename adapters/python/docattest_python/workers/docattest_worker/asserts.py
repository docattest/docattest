"""Assertions: attested code is compiled so that a failed comparison keeps both sides,
and run, awaiting at its top level when it does."""

from __future__ import annotations

import ast
import operator
from collections.abc import Callable, Coroutine
from types import CodeType
from typing import Literal, NamedTuple, cast

CO_COROUTINE = 0x80
COMPARE = "__docattest_compare__"

#: Each comparison ``assert`` keeps both sides of: the test, and how it is written.
OPERATORS: dict[str, tuple[Callable[[object, object], object], str]] = {
    "Eq": (operator.eq, "=="),
    "NotEq": (operator.ne, "!="),
    "Lt": (operator.lt, "<"),
    "LtE": (operator.le, "<="),
    "Gt": (operator.gt, ">"),
    "GtE": (operator.ge, ">="),
    "Is": (operator.is_, "is"),
    "IsNot": (operator.is_not, "is not"),
    "In": (lambda a, b: a in b, "in"),  # type: ignore[operator]
    "NotIn": (lambda a, b: a not in b, "not in"),  # type: ignore[operator]
}


class Comparison(NamedTuple):
    """What a failed ``assert left <op> right`` remembers, on its ``AssertionError``."""

    op: str
    left: object
    right: object
    custom_message: bool


def compare(left: object, right: object, op: str, message: Callable[[], object] | None) -> None:
    """What ``assert left <op> right, message`` becomes: the same test, but a failure
    keeps both sides, so the report can show them."""
    if OPERATORS[op][0](left, right):
        return
    error = AssertionError(message()) if message is not None else AssertionError()
    error.__docattest__ = Comparison(op, left, right, message is not None)  # type: ignore[attr-defined]
    raise error


def comparison_of(error: AssertionError) -> Comparison | None:
    """The comparison a failed rewritten ``assert`` remembered, if it was one."""
    found: object = getattr(error, "__docattest__", None)
    return found if isinstance(found, Comparison) else None


class _Asserts(ast.NodeTransformer):
    """Rewrites ``assert a <op> b`` into a call that remembers ``a`` and ``b``."""

    def visit_Assert(self, node: ast.Assert) -> ast.stmt:
        test = node.test
        if not isinstance(test, ast.Compare) or len(test.ops) != 1:
            return node
        op = type(test.ops[0]).__name__
        if op not in OPERATORS:
            return node
        if node.msg is not None and any(
            isinstance(n, (ast.Await, ast.Yield, ast.YieldFrom, ast.NamedExpr)) for n in ast.walk(node.msg)
        ):
            return node
        if node.msg is None:
            message: ast.expr = ast.Constant(None)
        else:
            no_args = ast.arguments(posonlyargs=[], args=[], kwonlyargs=[], kw_defaults=[], defaults=[])
            message = ast.Lambda(args=no_args, body=node.msg)
        call = ast.Call(
            func=ast.Name(COMPARE, ast.Load()),
            args=[test.left, test.comparators[0], ast.Constant(op), message],
            keywords=[],
        )
        return ast.fix_missing_locations(ast.copy_location(ast.Expr(call), node))


def compile_attested(source: str, filename: str, mode: Literal["exec", "eval"] = "exec") -> CodeType:
    """Compile attested code: asserts keep their two sides, and ``await`` may stand at the
    top level."""
    tree = _Asserts().visit(ast.parse(source, filename, mode))
    ast.fix_missing_locations(tree)
    return compile(tree, filename, mode, flags=ast.PyCF_ALLOW_TOP_LEVEL_AWAIT, dont_inherit=True)


def run_code(code: CodeType, namespace: dict[str, object]) -> object:
    """Run a code object; one that awaits at its top level runs in a new event loop."""
    if code.co_flags & CO_COROUTINE:
        import asyncio

        awaitable = cast("Coroutine[object, object, object]", eval(code, namespace))  # noqa: S307
        return asyncio.run(awaitable)
    return eval(code, namespace)  # noqa: S307


def finish(value: object) -> object:
    """The value of a call that may have returned an awaitable."""
    import inspect

    if inspect.isawaitable(value):
        import asyncio

        async def wait() -> object:
            return await value

        return asyncio.run(wait())
    return value
