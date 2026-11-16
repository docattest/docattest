"""The top level of one module, from its syntax: what it binds, what ``__all__`` says,
what it imports by ``*``."""

from __future__ import annotations

import ast
import builtins
from collections.abc import Iterable
from dataclasses import dataclass
from typing import Literal, TypeGuard, cast

TOP_LEVEL = "<top-level>"
BUILTIN_NAMES = frozenset(dir(builtins)) | {
    "__file__",
    "__name__",
    "__doc__",
    "__builtins__",
    "__spec__",
    "__class__",
    "__annotations__",
    "__dict__",
    "__module__",
    "__qualname__",
    "__path__",
    "__package__",
    "__loader__",
}


@dataclass
class Declaration:
    """A top-level name the module itself declares: a ``def``, a ``class`` or an assignment.
    A name defined again (overloads, re-assignment) is one declaration with every node."""

    name: str
    kind: Literal["def", "class", "assign"]
    nodes: list[ast.AST]


@dataclass(frozen=True)
class ModuleImport:
    """``import x`` or ``import x.y as z``: the name stands for a module."""

    name: str
    module: str


@dataclass(frozen=True)
class NameImport:
    """``from m import x``: the name stands for ``x`` of ``m`` (an absolute module name)."""

    name: str
    module: str
    original: str


#: One top-level name of a module.
Binding = Declaration | ModuleImport | NameImport


class Module:
    """The top level of one module, from its syntax."""

    def __init__(self, name: str, file: str, text: str, is_package: bool) -> None:
        self.name = name
        self.file = file
        self.is_package = is_package
        self.text = text
        self.error: str | None = None
        self.bindings: dict[str, Binding] = {}
        self.stars: list[str] = []
        self.all: list[str] | None = None
        self.all_dynamic = False
        self.has_getattr = False
        self.loose: list[ast.stmt] = []
        try:
            self.tree = ast.parse(text)
        except (SyntaxError, ValueError) as err:
            self.tree = ast.Module(body=[], type_ignores=[])
            self.error = f"{type(err).__name__}: {getattr(err, 'msg', err)}"
        self._collect(self.tree.body)

    # The package relative imports start from.
    def _package(self) -> str:
        return self.name if self.is_package else self.name.rpartition(".")[0]

    def absolute(self, module: str | None, level: int) -> str:
        if not level:
            return module or ""
        parts = self._package().split(".") if self._package() else []
        if level > 1:
            parts = parts[: len(parts) - (level - 1)] if level - 1 <= len(parts) else []
        return ".".join(parts + ([module] if module else []))

    def _declare(self, name: str, kind: Literal["def", "class", "assign"], node: ast.AST) -> None:
        existing = self.bindings.get(name)
        if isinstance(existing, Declaration) and existing.kind == kind and kind in ("def", "assign"):
            existing.nodes.append(node)  # overloads, re-assignment: one unit
        else:
            self.bindings[name] = Declaration(name, kind, [node])

    def _collect(self, body: Iterable[ast.stmt]) -> None:
        for node in body:
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                self._function(node)
            elif isinstance(node, ast.ClassDef):
                self._declare(node.name, "class", node)
            elif isinstance(node, ast.Import):
                self._import(node)
            elif isinstance(node, ast.ImportFrom):
                self._import_from(node)
            elif isinstance(node, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
                self._assign(node)
            elif _is_block(node):
                self._collect_block(node)
            else:
                self._loose(node)

    def _function(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
        self._declare(node.name, "def", node)
        if node.name == "__getattr__":
            self.has_getattr = True

    def _import(self, node: ast.Import) -> None:
        for alias in node.names:
            if alias.asname:
                self.bindings[alias.asname] = ModuleImport(alias.asname, alias.name)
            else:
                top = alias.name.split(".")[0]
                self.bindings[top] = ModuleImport(top, top)

    def _import_from(self, node: ast.ImportFrom) -> None:
        module = self.absolute(node.module, node.level)
        if node.module == "__future__":
            return
        for alias in node.names:
            if alias.name == "*":
                self.stars.append(module)
            else:
                local = alias.asname or alias.name
                self.bindings[local] = NameImport(local, module, alias.name)

    def _collect_block(self, node: ast.stmt) -> None:
        """The statements inside an ``if``, ``try`` or ``with`` at the top level."""
        if isinstance(node, ast.If):
            if _is_type_checking(node.test):
                self._collect(node.orelse)
            else:
                self._collect(node.body)
                self._collect(node.orelse)
        elif isinstance(node, (ast.With, ast.AsyncWith)):
            self._collect(node.body)
        else:
            # A ``try``, or a ``try`` with ``except*`` (``ast.TryStar``, which Python 3.10
            # lacks): both have these fields.
            block = cast("ast.Try", node)
            self._collect(block.body)
            for handler in block.handlers:
                self._collect(handler.body)
            self._collect(block.orelse)
            self._collect(block.finalbody)

    def _loose(self, node: ast.stmt) -> None:
        """Any other top-level statement is loose code, but the docstring; a call that adds
        to ``__all__`` is read for it too."""
        if isinstance(node, ast.Expr) and _all_call(node.value):
            self._all_extend(node.value)
            self.loose.append(node)
        elif not _is_docstring(node):
            self.loose.append(node)

    def _assign(self, node: ast.Assign | ast.AnnAssign | ast.AugAssign) -> None:
        targets = node.targets if isinstance(node, ast.Assign) else [node.target]
        value = node.value
        if isinstance(node, ast.AnnAssign) and value is None:
            return
        for target in targets:
            for name in _target_names(target):
                if name == "__all__":
                    self._all_assign(node, value)
                else:
                    self._declare(name, "assign", node)

    def _all_assign(self, node: ast.Assign | ast.AnnAssign | ast.AugAssign, value: ast.expr | None) -> None:
        names = _string_list(value)
        if names is None:
            self.all_dynamic = True
            return
        if isinstance(node, ast.AugAssign):
            self.all = (self.all or []) + names
        else:
            self.all = names

    def _all_extend(self, call: ast.Call) -> None:
        """``__all__.extend([...])`` or ``__all__.append("name")`` (``_all_call`` said which)."""
        if not call.args:
            self.all_dynamic = True
            return
        arg = call.args[0]
        method = cast("ast.Attribute", call.func).attr
        names = (
            _string_list(arg) if method == "extend" else _string_list(ast.List(elts=[arg], ctx=ast.Load()))
        )
        if names is None:
            self.all_dynamic = True
        else:
            self.all = (self.all or []) + names


def _is_docstring(node: ast.stmt) -> bool:
    return (
        isinstance(node, ast.Expr)
        and isinstance(node.value, ast.Constant)
        and isinstance(node.value.value, str)
    )


def _is_block(node: ast.stmt) -> bool:
    """An ``if``, ``try`` (``try``/``except*`` too) or ``with`` statement."""
    return (
        isinstance(node, (ast.If, ast.Try, ast.With, ast.AsyncWith)) or node.__class__.__name__ == "TryStar"
    )


def _is_type_checking(test: ast.expr) -> bool:
    return (isinstance(test, ast.Name) and test.id == "TYPE_CHECKING") or (
        isinstance(test, ast.Attribute) and test.attr == "TYPE_CHECKING"
    )


def _all_call(value: ast.expr) -> TypeGuard[ast.Call]:
    return (
        isinstance(value, ast.Call)
        and isinstance(value.func, ast.Attribute)
        and isinstance(value.func.value, ast.Name)
        and value.func.value.id == "__all__"
        and value.func.attr in ("extend", "append")
    )


def _string_list(value: ast.expr | None) -> list[str] | None:
    if isinstance(value, (ast.List, ast.Tuple, ast.Set)):
        out: list[str] = []
        for el in value.elts:
            if isinstance(el, ast.Constant) and isinstance(el.value, str):
                out.append(el.value)
            else:
                return None
        return out
    if isinstance(value, ast.BinOp) and isinstance(value.op, ast.Add):
        left, right = _string_list(value.left), _string_list(value.right)
        return None if left is None or right is None else left + right
    return None


def _target_names(target: ast.expr) -> list[str]:
    if isinstance(target, ast.Name):
        return [target.id]
    if isinstance(target, (ast.Tuple, ast.List)):
        return [n for el in target.elts for n in _target_names(el)]
    if isinstance(target, ast.Starred):
        return _target_names(target.value)
    return []
