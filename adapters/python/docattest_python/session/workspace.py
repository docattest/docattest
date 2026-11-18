"""The project a session is about: its files, read once each, its modules, its configured
surfaces resolved, and its setup module, all from syntax."""

from __future__ import annotations

import ast
import os
import re

from ..analysis import surfaces as surface_rules
from ..analysis.modules import Module
from ..analysis.names import dict_entries
from ..analysis.project import Project
from ..analysis.readable import UNREAD, Readable, readable
from ..analysis.surfaces import ConfiguredSurface, FindInstalled, Surface

_WORD = re.compile(r"[A-Za-z_]\w*")
#: The name the setup module is read and run under.
SETUP_MODULE = "docattest_setup"


class Workspace:
    def __init__(
        self, root: str, configured: list[ConfiguredSurface], setup: str | None, installed: FindInstalled
    ) -> None:
        self.root = root
        self.configured = configured
        #: The setup module as the config wrote it, relative to the root.
        self.setup = setup
        self._texts: dict[str, str | None] = {}
        self._setup_module: Module | None = None
        self._readable: dict[str, Readable] = {}
        self.project = Project(root, self.read, surface_rules.search_dirs(root))
        self.surfaces = surface_rules.resolve(self.project, root, configured, installed)
        for s in self.surfaces:
            if s.base and s.base not in self.project.search:
                self.project.search.append(s.base)
        self.by_name = {s.name: s for s in self.surfaces}

    # --- files ---------------------------------------------------------------------------

    def read(self, path: str) -> str | None:
        """The text of a file by absolute path, or None; read once per session."""
        if path not in self._texts:
            try:
                with open(path, encoding="utf-8", errors="replace") as handle:
                    self._texts[path] = handle.read()
            except (OSError, ValueError):
                self._texts[path] = None
        return self._texts[path]

    def abs(self, rel: str) -> str:
        return os.path.normpath(os.path.join(self.root, rel))

    def rel(self, path: str) -> str:
        relative = os.path.relpath(path, self.root)
        return path if relative.startswith("..") else relative.replace(os.sep, "/")

    def in_code(self, names: list[str], files: list[str]) -> list[str]:
        """Which of ``names`` occur as words in ``files`` or in a surface's file."""
        wanted: set[str] = set(names)
        found: set[str] = set()
        paths = {self.abs(f) for f in files} | {s.file for s in self.surfaces if s.file}
        for path in paths:
            text = self.read(path)
            if text:
                found.update(w for w in _WORD.findall(text) if w in wanted)
        return [n for n in names if n in found]

    # --- surfaces ------------------------------------------------------------------------

    def exports(self, name: str) -> list[str] | None:
        """What a surface exports; None for an unknown surface or one that cannot be read."""
        surface = self.by_name.get(name)
        if surface is None or surface.module is None:
            return None
        return self.project.exports(surface.module)

    def readable(self, name: str) -> Readable:
        """What facts read of a configured surface: its constants from source, its other
        exports by importing it."""
        if name not in self._readable:
            surface = self.by_name[name]
            module = surface.module
            if module is None or module.error is not None:
                self._readable[name] = UNREAD
            else:
                self._readable[name] = readable(surface, module, self.project.exports(module))
        return self._readable[name]

    def origin(self, surface: Surface, name: str) -> tuple[str, str] | None:
        """(file, name) of the declaration an export of a surface refers to."""
        if surface.module is None:
            return None
        module, declared = self.project.origin(surface.module, name)
        return (module.file, declared) if module is not None else None

    def own_module(self, name: str) -> Module | None:
        """A module whose source the adapter reads: a surface, or a file of the project."""
        for s in self.surfaces:
            if s.import_name == name and s.module is not None:
                return s.module
        return self.project.module(name)

    # --- the setup module ----------------------------------------------------------------

    def setup_module(self) -> Module | None:
        if self.setup is None:
            return None
        if self._setup_module is None:
            file = self.abs(self.setup)
            text = self.read(file)
            if text is None:
                return None
            self._setup_module = Module(SETUP_MODULE, file, text, False)
        return self._setup_module

    def setup_entries(self, group: str) -> dict[str, ast.AST]:
        """The entries of the setup module's ``cases`` or ``facts`` dict, by key."""
        module = self.setup_module()
        return dict_entries(module, group) if module else {}
