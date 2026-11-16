"""What ``compile`` reports about an example: what can be said of a block without
running it."""

from __future__ import annotations

import ast
import os
from collections.abc import Iterator
from typing import TYPE_CHECKING

from .. import language
from ..analysis.modules import BUILTIN_NAMES, Module
from ..analysis.names import bound_names, dotted_chain, free_names
from ..attestations import Example
from ..workers.interpreter import Interpreter
from .continued import ContinuedBlocks, inherited_names
from .workspace import Workspace

if TYPE_CHECKING:
    import doctest

#: A problem: the line within the block, and what is wrong there.
Problem = tuple[int, str]
#: A statement of a session: its line within the block, and its syntax.
Statement = tuple[int, ast.Module]


class Problems:
    """What answers ``compile``."""

    def __init__(self, workspace: Workspace, continued: ContinuedBlocks, interpreter: Interpreter) -> None:
        self.workspace = workspace
        self.project = workspace.project
        self.continued = continued
        self.interpreter = interpreter
        #: Whether ``import`` finds each top-level module that is not the project's own.
        self._found: dict[str, bool] = {}

    def compile(self, examples: list[Example]) -> list[dict[str, object]]:
        self.continued.remember(examples)
        out: list[dict[str, object]] = []
        for e in examples:
            for line, message in self._problems(e):
                out.append({"id": e.id, "message": message, "line": line})
        return out

    def _problems(self, e: Example) -> list[Problem]:
        code = e.code
        before = self.continued.before(e)
        if before is None:
            return [
                (1, "`continue` goes on from the block before it, and this page has none before this one")
            ]
        inherited = inherited_names(before)
        if language.is_session(e.lang, code):
            return self._session_problems(e, code, inherited)
        try:
            tree = ast.parse(code)
            compile(tree, "<block>", "exec", flags=ast.PyCF_ALLOW_TOP_LEVEL_AWAIT, dont_inherit=True)
        except SyntaxError as err:
            return [(err.lineno or 1, f"SyntaxError: {err.msg}")]
        except (ValueError, RecursionError, MemoryError) as err:
            return [(1, f"{type(err).__name__}: {err}")]
        return self._static_problems(tree, check_names=inherited is not None, inherited=inherited or set())

    def _session_problems(self, e: Example, code: str, inherited: set[str] | None) -> list[Problem]:
        import doctest  # only when the pages hold sessions: it is slow to import

        try:
            examples = doctest.DocTestParser().get_examples(code)
        except ValueError as err:
            return [(1, f"the session cannot be read: {err}")]
        trees, problems = _parse_statements(examples)
        if problems:
            return problems
        for lineno, tree in trees:
            for line, message in self._static_problems(tree, check_names=False):
                problems.append((lineno + line, message))
        if language.is_source(e.page):
            return problems
        problems += self._unbound_in_session(trees, inherited)
        return _unique(problems)

    def _unbound_in_session(self, trees: list[Statement], inherited: set[str] | None) -> list[Problem]:
        """A session on a page starts with nothing: a name it never binds is a mistake."""
        whole = ast.Module(body=[n for _, t in trees for n in t.body], type_ignores=[])
        if _imports_star(whole) or inherited is None:
            return []
        undefined = {n for n in free_names(whole) if n not in BUILTIN_NAMES}
        problems: list[Problem] = []
        for line, name in _reads(trees):
            if name in undefined and name not in inherited:
                problems.append((line, f"name '{name}' is not defined"))
        return problems

    def _static_problems(
        self, tree: ast.AST, check_names: bool, inherited: set[str] | frozenset[str] = frozenset()
    ) -> list[Problem]:
        """What can honestly be said without running or type-checking: every import must
        find its module; a name imported from a project module must be bound there; an
        attribute read off an imported project module must exist; and a name the block
        reads must be bound somewhere in it (or be a builtin)."""
        problems: list[Problem] = []
        project_modules: dict[str, Module] = {}
        has_star = False
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                problems += self._import_problems(node, project_modules)
            elif isinstance(node, ast.ImportFrom):
                found, star = self._import_from_problems(node)
                problems += found
                has_star = has_star or star
        problems += self._attribute_problems(tree, project_modules)
        if check_names and not has_star:
            problems += _unbound_names(tree, bound_names(tree), inherited)
        return _unique(sorted(problems))

    def _import_problems(self, node: ast.Import, project_modules: dict[str, Module]) -> list[Problem]:
        """``import a.b``: every module must exist. The project's own go into
        ``project_modules``, by the name the block uses for them."""
        problems: list[Problem] = []
        for alias in node.names:
            if not self.module_exists(alias.name):
                problems.append((node.lineno, f"No module named '{alias.name}'"))
                continue
            module = self.workspace.own_module(alias.name)
            if module is not None:
                project_modules[alias.asname or alias.name] = module
        return problems

    def _import_from_problems(self, node: ast.ImportFrom) -> tuple[list[Problem], bool]:
        """``from a import b``: the module must exist, and a project module must bind ``b``.
        Also says whether it imports ``*`` (from a module that exists)."""
        if node.level:
            return [(node.lineno, "attempted relative import with no known parent package")], False
        name = node.module or ""
        if not self.module_exists(name):
            return [(node.lineno, f"No module named '{name}'")], False
        module = self.workspace.own_module(name)
        problems: list[Problem] = []
        star = False
        for alias in node.names:
            if alias.name == "*":
                star = True
            elif module is not None and self.project.defines(module, alias.name) is False:
                where = self.workspace.rel(module.file)
                problems.append((node.lineno, f"cannot import name '{alias.name}' from '{name}' ({where})"))
        return problems, star

    def _attribute_problems(self, tree: ast.AST, project_modules: dict[str, Module]) -> list[Problem]:
        """An attribute read straight off an imported project module must exist."""
        problems: list[Problem] = []
        for node in ast.walk(tree):
            if isinstance(node, ast.Attribute):
                chain = dotted_chain(node)
                module = project_modules.get(".".join(chain[:-1])) if chain and len(chain) >= 2 else None
                if chain and module is not None and self._lacks(module, chain[-1]):
                    problems.append((node.lineno, f"module '{module.name}' has no attribute '{chain[-1]}'"))
        return problems

    def _lacks(self, module: Module, name: str) -> bool:
        """Whether a project module neither binds ``name`` nor has a submodule by that name."""
        return self.project.defines(module, name) is False and not self.project.find(f"{module.name}.{name}")

    # --- what import finds ---------------------------------------------------------------

    def module_exists(self, name: str) -> bool:
        """Whether ``import name`` can find a module, as far as can be told without importing."""
        top = name.split(".")[0]
        surfaces = self.workspace.surfaces
        if any(s.import_name == name or s.import_name.startswith(name + ".") for s in surfaces if s.file):
            return True
        if self.project.find(name):
            return True
        if self.project.find(top):
            # A package of the project: its modules are files, or directories without one.
            return any(os.path.isdir(os.path.join(base, *name.split("."))) for base in self.project.search)
        if top not in self._found:
            self._found[top] = self.interpreter.finds(top)
        return self._found[top]


def _parse_statements(examples: list[doctest.Example]) -> tuple[list[Statement], list[Problem]]:
    """Each statement of a session as ``(line, tree)``, and where one does not parse."""
    trees: list[Statement] = []
    problems: list[Problem] = []
    for example in examples:
        try:
            trees.append((example.lineno, ast.parse(example.source)))
        except SyntaxError as err:
            problems.append((example.lineno + (err.lineno or 1), f"SyntaxError: {err.msg}"))
    return trees, problems


def _imports_star(tree: ast.AST) -> bool:
    return any(isinstance(n, ast.ImportFrom) and any(a.name == "*" for a in n.names) for n in ast.walk(tree))


def _reads(trees: list[Statement]) -> Iterator[tuple[int, str]]:
    """Each name a session's statements read, with its line in the block."""
    for lineno, tree in trees:
        for node in ast.walk(tree):
            if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load):
                yield lineno + node.lineno, node.id


def _unbound_names(tree: ast.AST, bound: set[str], inherited: set[str] | frozenset[str]) -> list[Problem]:
    """The names a block reads that it never binds, that it did not inherit from the blocks
    it continues, and that are not builtins."""
    problems: list[Problem] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load):
            if node.id not in bound and node.id not in inherited and node.id not in BUILTIN_NAMES:
                problems.append((node.lineno, f"name '{node.id}' is not defined"))
    return problems


def _unique(items: list[Problem]) -> list[Problem]:
    out: list[Problem] = []
    seen: set[Problem] = set()
    for item in items:
        if item not in seen:
            seen.add(item)
            out.append(item)
    return out
