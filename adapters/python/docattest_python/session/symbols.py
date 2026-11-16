"""The surface symbols an attestation uses (``symbols``), and the tests a file declares
(``tests``), read from syntax."""

from __future__ import annotations

import ast
import os
from collections.abc import Iterator
from typing import NamedTuple

from .. import language
from ..analysis.modules import Declaration, Module
from ..analysis.names import Imports, chains, expression_names, free_names
from ..attestations import Attestation, Example, Fact, TestCitation
from .continued import ContinuedBlocks, example_tree
from .workspace import Workspace

#: A surface symbol: its name, and the surface that exports it.
Ref = tuple[str, str]
#: A symbol as ``symbols`` answers it: ``name``, and ``surface`` when one exports it.
SymbolRef = dict[str, str]


class Symbols:
    """What answers ``symbols`` and ``tests``."""

    def __init__(self, workspace: Workspace, continued: ContinuedBlocks) -> None:
        self.workspace = workspace
        self.project = workspace.project
        self.surfaces = workspace.surfaces
        self.continued = continued

    # --- symbols -------------------------------------------------------------------------

    def symbols(self, attestations: list[Attestation]) -> dict[str, list[SymbolRef]]:
        self.continued.remember(attestations)
        return {a.id: self._symbols(a) for a in attestations}

    def _symbols(self, a: Attestation) -> list[SymbolRef]:
        try:
            if isinstance(a, Example):
                return self._example_symbols(a)
            if isinstance(a, Fact):
                return self._fact_symbols(a) if a.expression else []
            if isinstance(a, TestCitation):
                return self._citation_symbols(a)
            return self._helper_symbols("cases", a.runner)
        except RecursionError:
            return []

    def _known(self, refs: list[Ref]) -> list[SymbolRef]:
        out: list[SymbolRef] = []
        seen: set[Ref] = set()
        for name, surface in refs:
            if (name, surface) in seen:
                continue
            exported = self.workspace.exports(surface)
            if exported is not None and name in exported:
                seen.add((name, surface))
                out.append({"name": name, "surface": surface})
        return out

    def _named(self, source: str, name: str) -> list[Ref]:
        """Surface symbols that ``from source import name`` reaches."""
        refs = [(name, s.name) for s in self.surfaces if s.import_name == source]
        if refs:
            return refs
        # A module of the project that is not a surface: the name still attests the
        # surface export that refers to the same declaration.
        module = self.project.module(source)
        if module is None:
            return []
        target, declared = self.project.origin(module, name)
        if target is None:
            return []
        for s in self.surfaces:
            if name in self._exports(s.name) and self.workspace.origin(s, name) == (target.file, declared):
                refs.append((name, s.name))
        return refs

    def _exports(self, surface: str) -> list[str]:
        return self.workspace.exports(surface) or []

    def _refs(self, imports: Imports, nodes: list[ast.AST] | None) -> list[Ref]:
        """Surface symbols reached through ``imports``: all of them when ``nodes`` is None
        (an example attests what it imports), else those the nodes use."""
        by_import = {s.import_name: s.name for s in self.surfaces if s.import_name}
        if nodes is None:
            refs = self._all_imported(imports)
            used: list[list[str]] = []
        else:
            used = [chain for node in nodes for chain in chains(node)]
            refs = self._imported_uses(imports, used)
        refs.extend(self._star_uses(imports.stars, by_import, used))
        refs.extend(_module_uses(_modules(imports, by_import), by_import, used))
        return refs

    def _all_imported(self, imports: Imports) -> list[Ref]:
        refs: list[Ref] = []
        for _local, (source, name) in imports.named.items():
            refs.extend(self._named(source, name))
        return refs

    def _imported_uses(self, imports: Imports, used: list[list[str]]) -> list[Ref]:
        """What the chains read of the names imported one by one."""
        refs: list[Ref] = []
        for chain in used:
            if chain[0] in imports.named:
                refs.extend(self._named(*imports.named[chain[0]]))
        return refs

    def _star_uses(self, stars: list[str], by_import: dict[str, str], used: list[list[str]]) -> list[Ref]:
        """What the chains read of the exports of a surface imported with ``*``."""
        refs: list[Ref] = []
        for star in stars:
            surface = by_import.get(star)
            if surface:
                exported = set(self._exports(surface))
                refs.extend((chain[0], surface) for chain in used if chain[0] in exported)
        return refs

    def _example_symbols(self, e: Example) -> list[SymbolRef]:
        tree = _parsed_or_empty(e)
        imports = Imports(tree)
        refs = self._refs(imports, None) + self._refs(imports, [tree])
        refs += self._continued_refs(e, tree)
        own: list[SymbolRef] = []
        if language.is_source(e.page):
            page = self.workspace.abs(e.page)
            if e.owner:
                own = self._owner(page, e.owner)
            refs += self._module_names_used(tree, page)
        known = self._known(refs)
        seen = {(r["name"], r.get("surface")) for r in own}
        return own + [r for r in known if (r["name"], r["surface"]) not in seen]

    def _continued_refs(self, e: Example, tree: ast.AST) -> list[Ref]:
        """What the block uses of the names its session imported earlier on the page."""
        before = self.continued.before(e)
        if not before:
            return []
        earlier = [t for t in map(example_tree, before) if t is not None]
        session_imports = Imports(ast.Module(body=[n for t in earlier for n in t.body], type_ignores=[]))
        return self._refs(session_imports, [tree])

    def _module_names_used(self, tree: ast.AST, page: str) -> list[Ref]:
        """A docstring's session runs with its module's names: using one attests the export
        it is."""
        refs: list[Ref] = []
        for name in free_names(tree):
            for s in self.surfaces:
                origin = self.workspace.origin(s, name) if name in self._exports(s.name) else None
                if origin and origin[0] == page:
                    refs.append((name, s.name))
        return refs

    def _owner(self, page: str, name: str) -> list[SymbolRef]:
        """A docstring's example attests its declaration on the surfaces that export that
        very declaration; when none does, on every surface that exports the name;
        otherwise it names the symbol without a surface."""
        declaring: list[str] = []
        exporting: list[str] = []
        for s in self.surfaces:
            if name not in self._exports(s.name):
                continue
            exporting.append(s.name)
            origin = self.workspace.origin(s, name)
            if origin and origin[0] == page:
                declaring.append(s.name)
        chosen = declaring or exporting
        return [{"name": name, "surface": s} for s in chosen] if chosen else [{"name": name}]

    def _fact_symbols(self, f: Fact) -> list[SymbolRef]:
        names = expression_names(f.expression)
        exported = set(self._exports(f.surface))
        refs = [{"name": n, "surface": f.surface} for n in names if n in exported]
        seen = {(r["name"], r["surface"]) for r in refs}
        for key in self.workspace.setup_entries("facts"):
            if key in names and key not in exported:
                for r in self._helper_symbols("facts", key):
                    if (r["name"], r["surface"]) not in seen:
                        seen.add((r["name"], r["surface"]))
                        refs.append(r)
        return refs

    def _helper_symbols(self, group: str, key: str) -> list[SymbolRef]:
        """Surface symbols a setup helper uses: its own body, and the functions of the
        setup module it calls."""
        module = self.workspace.setup_module()
        node = self.workspace.setup_entries(group).get(key)
        if module is None or node is None:
            return []
        imports = Imports(module.tree, module)
        nodes: list[ast.AST] = []
        seen: set[str] = set()
        queue = [node]
        while queue:
            current = queue.pop()
            nodes.append(current)
            for chain in chains(current):
                binding = module.bindings.get(chain[0])
                if isinstance(binding, Declaration) and chain[0] not in seen:
                    seen.add(chain[0])
                    queue.extend(binding.nodes)
        return self._known(self._refs(imports, nodes))

    def _citation_symbols(self, t: TestCitation) -> list[SymbolRef]:
        if t.error is not None or not language.is_source(t.file):
            return []
        file = self.workspace.abs(t.file)
        text = self.workspace.read(file)
        if text is None:
            return []
        name, _ = self.project.module_name(file)
        module = Module(name or "test", file, text, os.path.basename(file) == "__init__.py")
        found: list[ast.AST] = [d.node for d in _declared(module.tree) if d.kind == "test" and _cited(d, t)]
        if not found:
            return []
        return self._known(self._refs(Imports(module.tree, module), found))

    # --- tests ---------------------------------------------------------------------------

    def tests(self, file: str) -> dict[str, object]:
        if not language.is_source(file):
            return {"ok": False, "message": f"{file} is not a Python file, so its tests cannot be read"}
        text = self.workspace.read(self.workspace.abs(file))
        if text is None:
            return {"ok": False, "message": f"{file} does not exist"}
        try:
            tree = ast.parse(text)
        except (SyntaxError, ValueError) as err:
            return {
                "ok": False,
                "message": f"{file} does not parse: {type(err).__name__}: {getattr(err, 'msg', err)}",
            }
        tests: list[dict[str, object]] = []
        suites: list[dict[str, object]] = []
        for declared in _declared(tree):
            entry: dict[str, object] = {
                "title": declared.title,
                "describes": declared.describes,
                "line": declared.line,
            }
            if declared.kind == "test":
                tests.append(entry)
            else:
                # What it is, in Python's word: the core's message says "is a class, not a test".
                suites.append({**entry, "kind": "a class"})
        return {"ok": True, "tests": tests, "suites": suites}


def _parsed_or_empty(e: Example) -> ast.Module:
    """An example's code as a tree; an empty one when it does not parse."""
    return example_tree(e) or ast.Module(body=[], type_ignores=[])


def _modules(imports: Imports, by_import: dict[str, str]) -> dict[str, str]:
    """What each local name that stands for a module stands for, a surface module imported
    by name among them: ``from pkg import retry_module``."""
    modules = dict(imports.modules)
    for local, (source, name) in imports.named.items():
        if f"{source}.{name}" in by_import:
            modules[local] = f"{source}.{name}"
    return modules


def _module_uses(modules: dict[str, str], by_import: dict[str, str], used: list[list[str]]) -> list[Ref]:
    """What the chains read through a module: the name below the deepest surface module
    along each chain."""
    refs: list[Ref] = []
    for chain in used:
        dotted = modules.get(chain[0])
        if dotted is None:
            continue
        best = None
        for k in range(1, len(chain)):
            if dotted in by_import:
                best = (chain[k], by_import[dotted])
            dotted = f"{dotted}.{chain[k]}"
        if best:
            refs.append(best)
    return refs


class _Declared(NamedTuple):
    """A test, or a class that holds tests (a suite), as a test file declares it."""

    kind: str
    title: str
    #: The classes that enclose it, outermost first.
    describes: list[str]
    line: int
    node: ast.AST


def _declared(tree: ast.Module) -> list[_Declared]:
    """Tests and the classes that hold them, as pytest and unittest find them: a function
    or method whose name starts with ``test`` is a test, titled by its name; a class is a
    suite."""
    return list(_visit(tree.body, []))


def _visit(body: list[ast.stmt], describes: list[str]) -> Iterator[_Declared]:
    for node in body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.startswith("test"):
                yield _Declared("test", node.name, list(describes), node.lineno, node)
        elif isinstance(node, ast.ClassDef):
            yield _Declared("suite", node.name, list(describes), node.lineno, node)
            yield from _visit(node.body, describes + [node.name])
        elif isinstance(node, (ast.If, ast.Try, ast.With)):
            yield from _visit(node.body, describes)
            yield from _visit(node.orelse if isinstance(node, (ast.If, ast.Try)) else [], describes)


def _cited(declared: _Declared, cited: TestCitation) -> bool:
    if declared.title != cited.title:
        return False
    return not cited.describes or list(cited.describes) == declared.describes
