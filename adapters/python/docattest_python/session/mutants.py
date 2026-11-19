"""The edits that change what a proof reads (``mutants``): each symbol resolved to the
declaration it names, read from syntax, and the edits of that declaration's literals."""

from __future__ import annotations

import fnmatch
import os
from typing import NamedTuple

from .. import language
from ..analysis.literal_edits import literal_edits
from ..analysis.modules import Declaration, Module
from .workspace import Workspace


class MutatedSymbol(NamedTuple):
    """A symbol as ``symbols`` answered it: its name, and the surface that exports it."""

    name: str
    surface: str | None


class Mutants:
    """What answers ``mutants``."""

    def __init__(self, workspace: Workspace) -> None:
        self.workspace = workspace

    def mutants(self, symbols: list[MutatedSymbol], limit: int) -> list[dict[str, object]]:
        return [self._symbol_mutants(symbol, limit) for symbol in symbols]

    def _symbol_mutants(self, symbol: MutatedSymbol, limit: int) -> dict[str, object]:
        found = self._declaration(symbol)
        if found is None:
            return {"edits": [], "why": "notFound"}
        module, declaration = found
        if declaration.kind != "assign":
            return {"edits": [], "why": "function"}
        edits = literal_edits(module.text, declaration)[:limit]
        if not edits:
            return {"edits": [], "why": "noLiteral"}
        path = self.workspace.rel(module.file)
        return {"edits": [{"path": path, **edit._asdict()} for edit in edits]}

    def _declaration(self, symbol: MutatedSymbol) -> tuple[Module, Declaration] | None:
        """The declaration a symbol names in the project's own files: through its surface's
        module and any re-export; without a surface, through the first surface whose module
        declares the name, as a fact reads a constant from its surface's source."""
        for module in self._modules(symbol.surface):
            target, declared = self.workspace.project.origin(module, symbol.name)
            if target is None or not self._own(target.file):
                continue
            binding = target.bindings.get(declared)
            if isinstance(binding, Declaration):
                return target, binding
        return None

    def _modules(self, surface: str | None) -> list[Module]:
        if surface is None:
            return [s.module for s in self.workspace.surfaces if s.module is not None]
        configured = self.workspace.by_name.get(surface)
        return [configured.module] if configured is not None and configured.module is not None else []

    def _own(self, file: str) -> bool:
        """Whether a file is the project's own: inside it, and in no directory that never is
        (an installed package's ``site-packages``)."""
        relative = os.path.relpath(file, self.workspace.root)
        if relative.startswith(".."):
            return False
        segments = relative.split(os.sep)
        return not any(
            fnmatch.fnmatchcase(segment, pattern)
            for segment in segments
            for pattern in language.IGNORED_NAMES
        )
