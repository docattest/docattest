"""``affected``: the surface exports a change to the code affects.

A module is cut into units: each top-level function, class and assigned name is one, and
the statements that are none of these form one more, the module's side effects at import.
A unit's fingerprint is its syntax tree without positions and without docstrings, plus the
units its names resolve to, so reformatting, a comment or a docstring edit changes nothing
and pointing an import elsewhere does. An export is affected when the unit it refers to
changed, when that unit reaches a changed unit through the names it uses (in the working
tree or at the base, so a dependency the change removed still counts), or when the export
appeared, disappeared or now refers to another declaration.

Unlike the TypeScript adapter, a renamed local or parameter is a change (a parameter's name
is part of a Python signature), and a unit renamed together with its callers is not
recognised as the same unit.
"""

from __future__ import annotations

import ast
import copy
import os
from collections.abc import Callable, Mapping
from dataclasses import dataclass

from . import surfaces as surface_rules
from .modules import TOP_LEVEL, Declaration, Module, ModuleImport
from .names import chains
from .project import Project
from .surfaces import ConfiguredSurface

Read = Callable[[str], str | None]


def _strip_docstrings(node: ast.AST) -> ast.AST:
    node = copy.deepcopy(node)
    for inner in ast.walk(node):
        body = getattr(inner, "body", None)
        if isinstance(inner, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and body:
            first = body[0]
            if (
                isinstance(first, ast.Expr)
                and isinstance(first.value, ast.Constant)
                and isinstance(first.value.value, str)
            ):
                inner.body = body[1:] or [ast.Pass()]
    return node


class Revision:
    """The project at one revision: its modules, its units and what they depend on."""

    def __init__(
        self, root: str, read: Read, configured: list[ConfiguredSurface], extra_search: list[str]
    ) -> None:
        self.root = root
        self.project = Project(root, read, surface_rules.search_dirs(root, lambda d: True))
        # `src` may not exist at one of the two revisions; a missing directory finds nothing.
        self.surfaces = surface_rules.resolve(self.project, root, configured)
        for base in [s.base for s in self.surfaces if s.base] + extra_search:
            if base not in self.project.search:
                self.project.search.append(base)
        self._deps: dict[str, set[str]] = {}

    def rel(self, file: str) -> str:
        return os.path.relpath(file, self.root).replace(os.sep, "/")

    def key(self, module: Module, name: str) -> str:
        return f"{self.rel(module.file)}#{name}"

    def module_at(self, rel: str) -> Module | None:
        file = os.path.normpath(os.path.join(self.root, rel))
        if self.project.read(file) is None:
            return None
        name, _ = self.project.module_name(file)
        return self.project.load(name, file)

    def units(self, module: Module) -> dict[str, list[ast.AST]]:
        out: dict[str, list[ast.AST]] = {}
        for binding in module.bindings.values():
            if isinstance(binding, Declaration):
                out[binding.name] = binding.nodes
        if module.loose:
            out[TOP_LEVEL] = list(module.loose)
        return out

    def depends(self, module: Module, name: str) -> set[str]:
        """Keys of the units a unit's names resolve to."""
        key = self.key(module, name)
        if key in self._deps:
            return self._deps[key]
        deps: set[str] = set()
        self._deps[key] = deps
        nodes = self.units(module).get(name, [])
        if module.loose and name != TOP_LEVEL:
            deps.add(self.key(module, TOP_LEVEL))
        for node in nodes:
            for chain in chains(node):
                deps.update(self._resolve(module, chain))
        deps.discard(key)
        return deps

    def _resolve(self, module: Module, chain: list[str]) -> list[str]:
        binding = module.bindings.get(chain[0])
        if binding is None:
            # Not bound by the module itself: a star import may bring it.
            target, declared = self.project.origin(module, chain[0])
            if target is not None and target is not module:
                return [self.key(target, declared)]
            return []
        if isinstance(binding, Declaration):
            return [self.key(module, chain[0])]
        if isinstance(binding, ModuleImport):
            return self._resolve_module(binding.module, chain[1:])
        return self._resolve_from(module, chain)

    def _resolve_from(self, module: Module, chain: list[str]) -> list[str]:
        """A chain that starts with a name the module imports by ``from x import y``."""
        target, declared = self.project.origin(module, chain[0])
        if target is None:
            return []
        if declared == TOP_LEVEL and len(chain) > 1:
            return self._member(target, chain[1:])
        return [self.key(target, declared)]

    def _resolve_module(self, dotted: str, rest: list[str]) -> list[str]:
        """``import a.b`` then ``a.b.name``: walk the chain down to the module that owns the
        name."""
        target = self.project.module(dotted)
        while rest and self.project.module(f"{dotted}.{rest[0]}") is not None:
            dotted = f"{dotted}.{rest[0]}"
            target = self.project.module(dotted)
            rest = rest[1:]
        return self._member(target, rest) if target is not None else []

    def _member(self, module: Module, rest: list[str]) -> list[str]:
        if not rest:
            return [self.key(module, TOP_LEVEL)] if module.loose else []
        target, declared = self.project.origin(module, rest[0])
        return [self.key(target, declared)] if target is not None else []

    def fingerprints(self, files: list[str]) -> dict[str, str]:
        out: dict[str, str] = {}
        for rel in files:
            module = self.module_at(rel)
            if module is None:
                continue
            if module.error:
                out[f"{rel}#{TOP_LEVEL}"] = "unparsable:" + module.text
                continue
            for name, nodes in self.units(module).items():
                shape = "\n".join(ast.dump(_strip_docstrings(n)) for n in nodes)
                out[self.key(module, name)] = shape + "|" + ",".join(sorted(self.depends(module, name)))
        return out

    def exports(self) -> dict[str, str | None]:
        """Each surface export → the key of the unit it refers to."""
        out: dict[str, str | None] = {}
        for s in self.surfaces:
            if s.module is None:
                continue
            for name in self.project.exports(s.module):
                target, declared = self.project.origin(s.module, name)
                out[f"{s.name}\0{name}"] = self.key(target, declared) if target is not None else None
        return out

    def reaching(self, exports: dict[str, str | None], changed: set[str]) -> set[str]:
        """The exports whose unit is changed or reaches a changed unit."""
        verdicts: dict[str, bool] = {}

        def reaches(start: str) -> bool:
            stack, seen = [start], set[str]()
            while stack:
                key = stack.pop()
                if key in seen:
                    continue
                seen.add(key)
                if key in changed or verdicts.get(key):
                    verdicts[start] = True
                    return True
                rel, _, name = key.partition("#")
                module = self.module_at(rel)
                if module is not None:
                    stack.extend(self.depends(module, name))
            verdicts[start] = False
            return False

        return {export for export, key in exports.items() if key is not None and reaches(key)}


@dataclass(frozen=True)
class WorkingTree:
    """The project as it is now: its root, how its files are read, its configured
    surfaces, and where its modules are looked up."""

    root: str
    read: Read
    configured: list[ConfiguredSurface]
    search: list[str]


def affected(tree: WorkingTree, base: dict[str, str | None], changed: list[str]) -> list[dict[str, str]]:
    """The surface exports a change affects: ``base`` holds the earlier text of the files
    that changed (None for one that did not exist); ``tree`` reads their text now."""
    code = [f for f in changed if f.endswith(".py")]
    if not code:
        return []
    now, then = _revisions(tree, base)
    before, after = then.fingerprints(code), now.fingerprints(code)
    changed_units = _differing(before, after)
    now_exports, then_exports = now.exports(), then.exports()
    hit = _differing(now_exports, then_exports, missing="")
    if changed_units:
        hit |= now.reaching(now_exports, changed_units)
        hit |= then.reaching(then_exports, changed_units)
    out: list[dict[str, str]] = []
    for export in sorted(hit):
        surface, _, name = export.partition("\0")
        out.append({"name": name, "surface": surface})
    return out


def _revisions(tree: WorkingTree, base: dict[str, str | None]) -> tuple[Revision, Revision]:
    """The project now, and as it was: ``base`` holds the earlier text of changed files."""
    overlay = {os.path.normpath(os.path.join(tree.root, rel)): text for rel, text in base.items()}

    def read_then(path: str) -> str | None:
        if path in overlay:
            return overlay[path]
        return tree.read(path)

    now = Revision(tree.root, tree.read, tree.configured, list(tree.search))
    then = Revision(tree.root, read_then, tree.configured, list(tree.search))
    return now, then


def _differing(
    a: Mapping[str, str | None], b: Mapping[str, str | None], missing: str | None = None
) -> set[str]:
    """The keys of either dict whose values differ, a missing key counting as ``missing``."""
    return {k for k in set(a) | set(b) if a.get(k, missing) != b.get(k, missing)}
