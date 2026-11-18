"""The constants of a module, with their values read from its source: nothing of it runs.

A constant is a name the module assigns once, in a statement of its own top level (not
inside an ``if``, ``try`` or ``with``), with ``=`` or an annotated ``=``, to a constant
expression. A constant expression is a literal as ``ast.literal_eval`` reads one (numbers,
strings, bytes, ``True``, ``False``, ``None``, and tuples, lists, sets and dicts of constant
expressions), the name of a constant the same module assigns before it, ``+`` and ``-`` before a
number, ``+ - * / // %`` between numbers, or ``+`` between two strings, two bytes, two
tuples or two lists. Its value must be one Python writes as a literal, which it is sent as.
"""

from __future__ import annotations

import ast
import operator
from collections.abc import Callable
from typing import NamedTuple

from .modules import Declaration, Module

_NUMBER = (int, float, complex)
_CONCATENATED = (str, bytes, tuple, list)
#: The operators a constant expression may use, by the name of their ``operator`` function.
_ARITHMETIC: dict[type[ast.operator], str] = {
    ast.Add: "add",
    ast.Sub: "sub",
    ast.Mult: "mul",
    ast.Div: "truediv",
    ast.FloorDiv: "floordiv",
    ast.Mod: "mod",
}
_SIGNS: dict[type[ast.unaryop], str] = {ast.UAdd: "pos", ast.USub: "neg"}


class NotConstant(Exception):
    """An expression whose value cannot be read from source."""


def constants(module: Module) -> dict[str, object]:
    """Every constant of the module, by name, with its value."""
    assigned = _assigned_once(module)
    reader = _Reader(assigned)
    found: dict[str, object] = {}
    for name in assigned:
        try:
            found[name] = reader.value(name)
        except NotConstant:
            continue
    return found


def literal(value: object) -> str:
    """A constant's value as Python writes it, which ``ast.literal_eval`` reads back."""
    return repr(value)


class _Assigned(NamedTuple):
    """A name's one assignment: where its statement is in the module's body, and its value."""

    position: int
    value: ast.expr


def _assigned_once(module: Module) -> dict[str, _Assigned]:
    """The names a statement of the module's own top level assigns, once, to one value."""
    positions = {id(node): k for k, node in enumerate(module.tree.body)}
    out: dict[str, _Assigned] = {}
    for name, binding in module.bindings.items():
        if not isinstance(binding, Declaration) or binding.kind != "assign" or len(binding.nodes) != 1:
            continue
        node = binding.nodes[0]
        position = positions.get(id(node))
        value = _assigned_value(node)
        if position is not None and value is not None:
            out[name] = _Assigned(position, value)
    return out


def _assigned_value(node: ast.AST) -> ast.expr | None:
    """The value of ``A = value``, ``A = B = value`` or ``A: T = value``; None for any other
    assignment, such as one that unpacks or augments."""
    if isinstance(node, ast.Assign) and all(isinstance(t, ast.Name) for t in node.targets):
        return node.value
    if isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
        return node.value
    return None


class _Reader:
    """Reads the constant expressions of one module, each name once."""

    def __init__(self, assigned: dict[str, _Assigned]) -> None:
        self._assigned = assigned
        self._values: dict[str, object] = {}
        #: The names being read, the innermost last.
        self._reading: list[str] = []

    def value(self, name: str) -> object:
        if name in self._values:
            return self._values[name]
        self._reading.append(name)
        try:
            value = self._evaluate(self._assigned[name].value)
        finally:
            self._reading.pop()
        _check_literal(value)
        self._values[name] = value
        return value

    def _reference(self, name: str) -> object:
        """Another constant, which must be assigned before the one being read, as the
        module runs."""
        referred = self._assigned.get(name)
        if referred is None or referred.position >= self._assigned[self._reading[-1]].position:
            raise NotConstant(name)
        return self.value(name)

    def _evaluate(self, node: ast.expr) -> object:
        if isinstance(node, ast.Constant):
            return node.value
        if isinstance(node, ast.Name):
            return self._reference(node.id)
        if isinstance(node, (ast.Tuple, ast.List, ast.Set)):
            return self._collection(node)
        if isinstance(node, ast.Dict):
            return self._dict(node)
        if isinstance(node, ast.UnaryOp):
            return self._signed(node)
        if isinstance(node, ast.BinOp):
            return self._operation(node)
        raise NotConstant(ast.dump(node))

    def _collection(self, node: ast.Tuple | ast.List | ast.Set) -> object:
        if any(isinstance(element, ast.Starred) for element in node.elts):
            raise NotConstant("unpacking")
        items = [self._evaluate(element) for element in node.elts]
        if isinstance(node, ast.Tuple):
            return tuple(items)
        if isinstance(node, ast.List):
            return items
        return _hashed(lambda: set(items))

    def _dict(self, node: ast.Dict) -> object:
        keys = [key for key in node.keys if key is not None]
        if len(keys) != len(node.keys):
            raise NotConstant("unpacking")
        pairs = [(self._evaluate(k), self._evaluate(v)) for k, v in zip(keys, node.values)]
        return _hashed(lambda: dict(pairs))

    def _signed(self, node: ast.UnaryOp) -> object:
        name = _SIGNS.get(type(node.op))
        operand = self._evaluate(node.operand)
        if name is None or not isinstance(operand, _NUMBER):
            raise NotConstant("operator")
        sign: Callable[[object], object] = getattr(operator, name)
        return sign(operand)

    def _operation(self, node: ast.BinOp) -> object:
        name = _ARITHMETIC.get(type(node.op))
        operands = (self._evaluate(node.left), self._evaluate(node.right))
        if name is None or not _combines(node.op, *operands):
            raise NotConstant("operator")
        apply: Callable[[object, object], object] = getattr(operator, name)
        try:
            return apply(*operands)
        except ArithmeticError as err:
            raise NotConstant("arithmetic") from err


def _combines(op: ast.operator, left: object, right: object) -> bool:
    """Whether an operator applies to two values in a constant expression: any of them to
    two numbers, ``+`` to two strings, bytes, tuples or lists."""
    if isinstance(left, _NUMBER) and isinstance(right, _NUMBER):
        return True
    return isinstance(op, ast.Add) and type(left) is type(right) and isinstance(left, _CONCATENATED)


def _hashed(build: Callable[[], object]) -> object:
    """A set or a dict, unless an item cannot be hashed (a list in a set)."""
    try:
        return build()
    except TypeError as err:
        raise NotConstant("unhashable") from err


def _check_literal(value: object) -> None:
    """Refuse a value Python cannot write as a literal: an infinite float, say."""
    try:
        ast.literal_eval(literal(value))
    except (ValueError, SyntaxError, RecursionError) as err:
        raise NotConstant("not a literal") from err
