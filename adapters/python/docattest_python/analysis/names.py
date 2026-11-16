"""The names a piece of code uses: its imports, the chains it reads, the names it binds
and those it does not."""

from __future__ import annotations

import ast
from collections.abc import Callable
from typing import cast

from .modules import Module


class Imports:
    """The imports of a piece of code: what each local name stands for."""

    def __init__(self, tree: ast.AST, module: Module | None = None) -> None:
        #: local name → (absolute module, original name)
        self.named: dict[str, tuple[str, str]] = {}
        #: local name → module the name is bound to (``import a.b as c`` → c: a.b)
        self.modules: dict[str, str] = {}
        #: modules imported by ``import a.b.c`` (every prefix is reachable through ``a``)
        self.dotted: set[str] = set()
        self.stars: list[str] = []
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                self._import(node)
            elif isinstance(node, ast.ImportFrom):
                self._import_from(node, module)

    def _import(self, node: ast.Import) -> None:
        for alias in node.names:
            if alias.asname:
                self.modules[alias.asname] = alias.name
            else:
                self.modules[alias.name.split(".")[0]] = alias.name.split(".")[0]
                self.dotted.add(alias.name)

    def _import_from(self, node: ast.ImportFrom, module: Module | None) -> None:
        if module is not None:
            source = module.absolute(node.module, node.level)
        elif node.level:
            # A relative import, and no module to say what it is relative to.
            return
        else:
            source = node.module or ""
        for alias in node.names:
            if alias.name == "*":
                self.stars.append(source)
            else:
                self.named[alias.asname or alias.name] = (source, alias.name)


def dotted_chain(node: ast.AST) -> list[str] | None:
    """``a.b.c`` as ['a', 'b', 'c'], or None when the expression is not a plain chain."""
    parts: list[str] = []
    while isinstance(node, ast.Attribute):
        parts.append(node.attr)
        node = node.value
    if isinstance(node, ast.Name):
        parts.append(node.id)
        return parts[::-1]
    return None


def chains(tree: ast.AST) -> list[list[str]]:
    """Every outermost name or attribute chain the code reads."""
    out: list[list[str]] = []

    def visit(node: ast.AST) -> None:
        if isinstance(node, (ast.Attribute, ast.Name)):
            chain = dotted_chain(node)
            if chain is not None:
                out.append(chain)
                return
        for child in ast.iter_child_nodes(node):
            visit(child)

    visit(tree)
    return out


def bound_names(tree: ast.AST) -> set[str]:
    """Every name the code binds, in any scope."""
    names: set[str] = set()
    for node in ast.walk(tree):
        binds = _BINDERS.get(node.__class__.__name__)
        if binds is not None:
            names.update(binds(node))
    return names


# Each binder is given a node of the class it is listed under in ``_BINDERS``.


def _stored(node: ast.AST) -> list[str]:
    name = cast("ast.Name", node)
    return [name.id] if isinstance(name.ctx, (ast.Store, ast.Del)) else []


def _named(node: ast.AST) -> list[str]:
    """A function, a class or a type parameter: the name it declares."""
    return [cast("ast.FunctionDef", node).name]


def _argument(node: ast.AST) -> list[str]:
    return [cast("ast.arg", node).arg]


def _imported(node: ast.AST) -> list[str]:
    return [alias.asname or alias.name.split(".")[0] for alias in cast("ast.Import", node).names]


def _named_if_any(node: ast.AST) -> list[str]:
    """An ``except ... as name``, or a pattern's ``as name`` or ``*name``: a name, or none."""
    name: str | None = getattr(node, "name", None)
    return [name] if name else []


def _rest(node: ast.AST) -> list[str]:
    """A mapping pattern's ``**rest``."""
    rest: str | None = getattr(node, "rest", None)
    return [rest] if rest else []


def _declared(node: ast.AST) -> list[str]:
    return cast("ast.Global", node).names


# The nodes that bind a name, by their class's name: ``ast`` has no type parameters
# (``TypeVar`` and the rest) before Python 3.12.
_BINDERS: dict[str, Callable[[ast.AST], list[str]]] = {
    "Name": _stored,
    "FunctionDef": _named,
    "AsyncFunctionDef": _named,
    "ClassDef": _named,
    "arg": _argument,
    "Import": _imported,
    "ImportFrom": _imported,
    "ExceptHandler": _named_if_any,
    "Global": _declared,
    "Nonlocal": _declared,
    "MatchAs": _named_if_any,
    "MatchStar": _named_if_any,
    "MatchMapping": _rest,
    "TypeVar": _named,
    "ParamSpec": _named,
    "TypeVarTuple": _named,
}


def free_names(tree: ast.AST) -> list[str]:
    """Names the code reads and never binds itself (builtins among them), in order, once each."""
    bound = bound_names(tree)
    out: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load):
            if node.id not in bound and node.id not in out:
                out.append(node.id)
    # ast.walk is breadth-first; order by position instead.
    order: dict[str, tuple[int, int]] = {}
    for node in ast.walk(tree):
        if isinstance(node, ast.Name) and node.id in out:
            pos = (node.lineno, node.col_offset)
            order[node.id] = min(order.get(node.id, pos), pos)
    return sorted(out, key=lambda n: order.get(n, (0, 0)))


def expression_names(expression: str) -> list[str]:
    """Free names of a fact expression (builtins included: a surface may shadow one)."""
    try:
        tree = ast.parse(f"(\n{expression}\n)", mode="eval")
    except (SyntaxError, ValueError):
        return []
    return free_names(tree)


def dict_entries(module: Module, group: str) -> dict[str, ast.AST]:
    """The entries of a top-level dict of a setup module (``cases = {...}``,
    ``cases["x"] = f``, ``cases = dict(x=f)``): key → the value's node."""
    out: dict[str, ast.AST] = {}
    for node in ast.walk(module.tree):
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            _assigned_entries(node, group, out)
    return out


def _assigned_entries(node: ast.Assign | ast.AnnAssign, group: str, out: dict[str, ast.AST]) -> None:
    """The entries one assignment gives the dict named ``group``, into ``out``."""
    targets = node.targets if isinstance(node, ast.Assign) else [node.target]
    value = node.value
    if value is None:
        return
    for target in targets:
        if isinstance(target, ast.Name) and target.id == group:
            _literal_entries(value, out)
        else:
            key = _entry_key(target, group)
            if key is not None:
                out[key] = value


def _literal_entries(value: ast.AST, out: dict[str, ast.AST]) -> None:
    """The entries of ``{"x": f}`` or ``dict(x=f)``, into ``out``."""
    if isinstance(value, ast.Dict):
        for k, v in zip(value.keys, value.values):  # a None key is a ``**`` entry
            if isinstance(k, ast.Constant) and isinstance(k.value, str):
                out[k.value] = v
    elif isinstance(value, ast.Call) and getattr(value.func, "id", "") == "dict":
        for kw in value.keywords:
            if kw.arg:
                out[kw.arg] = kw.value


def _entry_key(target: ast.AST, group: str) -> str | None:
    """``key`` when an assignment target is ``group["key"]``, else None."""
    if not (
        isinstance(target, ast.Subscript) and isinstance(target.value, ast.Name) and target.value.id == group
    ):
        return None
    key = target.slice
    return key.value if isinstance(key, ast.Constant) and isinstance(key.value, str) else None
