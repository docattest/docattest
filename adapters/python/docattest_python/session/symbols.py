"""The surface symbols an attestation uses (``symbols``), and the tests a file declares
(``tests``), read from syntax."""

from __future__ import annotations

import ast
import os
from collections.abc import Callable, Collection
from typing import cast

from .. import language
from ..analysis.declared_tests.listing import Declared, cited, declared
from ..analysis.modules import Declaration, Module
from ..analysis.names import Imports, chains, computed_names, expression_names, free_names
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
            if isinstance(a, TestCitation):
                return self._citation_symbols(a)
            if isinstance(a, Fact):
                read = self._fact_symbols(a) if a.expression else []
            else:
                read = self._helper_symbols("cases", a.runner)
            return self._with_initializer_names(read)
        except RecursionError:
            return []

    def _with_initializer_names(self, refs: list[SymbolRef]) -> list[SymbolRef]:
        """``refs``, and the names their initializers use, at any depth: a value holds the
        values of those names, so a proof that reads it attests them too. A function's body
        is not followed: calling it reads what it returns, not what it is made of."""
        out = list(refs)
        seen = {(r["name"], r["surface"]) for r in refs}
        for ref in refs:
            for name in self._initializer_names(ref["name"], ref["surface"]):
                if (name, ref["surface"]) not in seen:
                    seen.add((name, ref["surface"]))
                    out.append({"name": name, "surface": ref["surface"]})
        return out

    def _initializer_names(self, name: str, surface: str) -> list[str]:
        """The names of the surface's module that a fact may attest and that the value
        assigned to ``name`` there is computed from, at any depth."""
        configured = self.workspace.by_name.get(surface)
        module = configured.module if configured else None
        if module is None:
            return []
        reached: list[str] = []
        queue = [name]
        while queue:
            binding = module.bindings.get(queue.pop())
            if not isinstance(binding, Declaration) or binding.kind != "assign":
                continue
            for used in _assigned_names(binding):
                if used != name and used not in reached:
                    reached.append(used)
                    queue.append(used)
        attestable = self._exports_and_constants(surface)
        return [n for n in reached if n in attestable]

    def _known(self, refs: list[Ref], attestable: Callable[[str], Collection[str]]) -> list[SymbolRef]:
        """The refs whose name is among those ``attestable`` gives for their surface, each
        once."""
        out: list[SymbolRef] = []
        seen: set[Ref] = set()
        for name, surface in refs:
            if (name, surface) not in seen and name in attestable(surface):
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

    def _constants(self, surface: str) -> dict[str, object]:
        if surface not in self.workspace.by_name:
            return {}
        return self.workspace.readable(surface).constants

    def _exports_and_constants(self, surface: str) -> set[str]:
        """What a fact, or a helper of the setup module, attests of a surface it reads: its
        exports, and its constants, exported or not. A fact reads a constant from source; a
        helper imports private names as any module may."""
        return set(self._exports(surface)) | set(self._constants(surface))

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
        known = self._known(refs, self._exports)
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
        """The exports a fact reads, and the constants it reads from source, which it
        attests although they may not be exports."""
        names = expression_names(f.expression)
        exported = set(self._exports(f.surface))
        read = self._exports_and_constants(f.surface)
        refs = [{"name": n, "surface": f.surface} for n in names if n in read]
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
        return self._known(self._refs(imports, nodes), self._exports_and_constants)

    def _citation_symbols(self, t: TestCitation) -> list[SymbolRef]:
        if t.error is not None or not language.is_source(t.file):
            return []
        file = self.workspace.abs(t.file)
        text = self.workspace.read(file)
        if text is None:
            return []
        name, _ = self.project.module_name(file)
        module = Module(name or "test", file, text, os.path.basename(file) == "__init__.py")
        found: list[ast.AST] = [
            d.node
            for d in declared(module.tree)
            if d.kind != "suite" and cited(d, t.title, list(t.describes))
        ]
        if not found:
            return []
        nodes = _reached_test_nodes(module, found, list(t.describes))
        return self._known(self._refs(Imports(module.tree, module), nodes), self._exports_and_constants)

    # --- tests ---------------------------------------------------------------------------

    def tests(self, file: str) -> dict[str, object]:
        found = self.declared(file)
        if isinstance(found, str):
            return {"ok": False, "message": found}
        tests: list[dict[str, object]] = []
        suites: list[dict[str, object]] = []
        for test in found:
            entry: dict[str, object] = {"title": test.title, "describes": test.describes, "line": test.line}
            if test.kind != "suite":
                tests.append(entry)
            else:
                # What it is, in Python's word: the core's message says "is a class, not a test".
                suites.append({**entry, "kind": "a class"})
        return {"ok": True, "tests": tests, "suites": suites}

    def declared(self, file: str) -> list[Declared] | str:
        """What a test file declares, or why it cannot be read."""
        if not language.is_source(file):
            return f"{file} is not a Python file, so its tests cannot be read"
        text = self.workspace.read(self.workspace.abs(file))
        if text is None:
            return f"{file} does not exist"
        try:
            tree = ast.parse(text)
        except (SyntaxError, ValueError) as err:
            return f"{file} does not parse: {type(err).__name__}: {getattr(err, 'msg', err)}"
        return declared(tree)


def _reached_test_nodes(module: Module, roots: list[ast.AST], describes: list[str]) -> list[ast.AST]:
    """``roots`` and the non-test helpers in ``module`` (and in any enclosing test class)
    that they reference, transitively."""
    class_methods = _class_helpers(module.tree, describes)
    nodes: list[ast.AST] = []
    seen: set[str] = set()
    queue = list(roots)
    while queue:
        current = queue.pop()
        nodes.append(current)
        for chain in chains(current):
            head = chain[0]
            if head in ("self", "cls") and len(chain) >= 2:
                method = class_methods.get(chain[1])
                key = f".{chain[1]}"
                if method is not None and key not in seen:
                    seen.add(key)
                    queue.append(method)
                continue
            if head.startswith("test_") or head in seen:
                continue
            binding = module.bindings.get(head)
            if isinstance(binding, Declaration):
                seen.add(head)
                queue.extend(binding.nodes)
    return nodes


def _class_helpers(tree: ast.Module, describes: list[str]) -> dict[str, ast.AST]:
    """Non-test helper methods on the class enclosing a cited test method, if any."""
    if not describes:
        return {}
    cls_name = describes[0]
    for node in tree.body:
        if isinstance(node, ast.ClassDef) and node.name == cls_name:
            return {
                item.name: item
                for item in node.body
                if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef))
                and not item.name.startswith("test_")
            }
    return {}


def _assigned_names(binding: Declaration) -> list[str]:
    """The names the values an assignment's statements give its name are computed from."""
    names: list[str] = []
    for node in binding.nodes:
        value = cast("ast.Assign | ast.AnnAssign | ast.AugAssign", node).value
        names.extend(computed_names(value) if value is not None else [])
    return names


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
