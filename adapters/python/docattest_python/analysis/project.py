"""The modules of one revision of a project, found and parsed without importing, and
what each exports and declares."""

from __future__ import annotations

import os
from collections.abc import Callable

from .modules import TOP_LEVEL, Binding, Declaration, Module, ModuleImport, NameImport


class Project:
    """Modules of one revision of a project, found and parsed without importing.

    ``read`` takes an absolute path and returns the text or None. ``search`` are the
    directories modules are looked up in, in order (what ``sys.path`` starts with when the
    attested code runs).
    """

    def __init__(self, root: str, read: Callable[[str], str | None], search: list[str]) -> None:
        self.root = root
        self.read = read
        self.search = search
        self._modules: dict[str, Module | None] = {}
        self._by_file: dict[str, Module] = {}
        self._exports: dict[str, list[str]] = {}

    def find(self, name: str) -> str | None:
        """The file of a module name: ``pkg/__init__.py`` or ``mod.py`` under a search dir."""
        if not name:
            return None
        parts = name.split(".")
        for base in self.search:
            directory = base
            ok = True
            for part in parts[:-1]:
                directory = os.path.join(directory, part)
                if self.read(os.path.join(directory, "__init__.py")) is None and not os.path.isdir(directory):
                    ok = False
                    break
            if not ok:
                continue
            package = os.path.join(directory, parts[-1], "__init__.py")
            if self.read(package) is not None:
                return package
            plain = os.path.join(directory, parts[-1] + ".py")
            if self.read(plain) is not None:
                return plain
        return None

    def module(self, name: str) -> Module | None:
        if name not in self._modules:
            file = self.find(name)
            self._modules[name] = self.load(name, file) if file else None
        return self._modules[name]

    def load(self, name: str, file: str) -> Module | None:
        """A module by the name it is imported as and the file it is read from."""
        cached = self._by_file.get(file)
        if cached is not None and cached.name == name:
            return cached
        text = self.read(file)
        if text is None:
            return None
        module = Module(name, file, text, os.path.basename(file) == "__init__.py")
        self._by_file[file] = module
        self._modules.setdefault(name, module)
        return module

    def module_name(self, file: str) -> tuple[str, str]:
        """The dotted name a file is imported as, and the directory that must be on the path:
        walk up while the directory is a package (has ``__init__.py``)."""
        directory, base = os.path.split(file)
        parts = [] if base == "__init__.py" else [base[:-3] if base.endswith(".py") else base]
        while self.read(os.path.join(directory, "__init__.py")) is not None:
            directory, package = os.path.split(directory)
            parts.insert(0, package)
            if not package:
                break
        return ".".join(parts), directory

    def is_submodule(self, module: Module, name: str) -> bool:
        if not module.is_package:
            return False
        directory = os.path.dirname(module.file)
        return (
            self.read(os.path.join(directory, name + ".py")) is not None
            or self.read(os.path.join(directory, name, "__init__.py")) is not None
        )

    def _names_submodule(self, package: Module, name: str) -> bool:
        """Whether ``from package import name`` gives a submodule rather than a value."""
        if not self.is_submodule(package, name):
            return False
        binding = package.bindings.get(name)
        return binding is None or isinstance(binding, (NameImport, ModuleImport))

    def own(self, module: Module, other: str) -> bool:
        """Whether ``other`` is a module of the same top-level package: a re-export."""
        return bool(other) and other.split(".")[0] == module.name.split(".")[0]

    def exports(self, module: Module, _seen: set[str] | None = None) -> list[str]:
        """What a module offers: ``__all__`` when it is written out; otherwise its public
        names: what it defines, and, for a package's ``__init__``, what it imports from its
        own package (a re-export), submodules excepted. Sorted."""
        key = module.file + "\0" + module.name
        if key in self._exports:
            return self._exports[key]
        if module.all is not None and not module.all_dynamic:
            names = set(module.all)
        else:
            seen = _seen or set()
            seen.add(key)
            names = self._public_names(module)
            names.update(self._star_exports(module, seen))
        out = sorted(names)
        if _seen is None:
            self._exports[key] = out
        return out

    def _public_names(self, module: Module) -> set[str]:
        """What a module defines, and what a package re-exports, not starting with ``_``."""
        names: set[str] = set()
        for b in module.bindings.values():
            if b.name.startswith("_"):
                continue
            if isinstance(b, Declaration) or self._reexports(module, b):
                names.add(b.name)
        return names

    def _reexports(self, module: Module, b: Binding) -> bool:
        """Whether a package's ``__init__`` offers what it imports by ``b``: a name from
        its own package that is not one of its submodules."""
        if not isinstance(b, NameImport) or not module.is_package or not self.own(module, b.module):
            return False
        target = self.module(b.module)
        return not (target is not None and self._names_submodule(target, b.original))

    def _star_exports(self, module: Module, seen: set[str]) -> set[str]:
        """What a module takes from the modules of its own package it imports ``*`` from."""
        names: set[str] = set()
        for star in module.stars:
            target = self.module(star) if self.own(module, star) else None
            if target is not None and (target.file + "\0" + target.name) not in seen:
                names.update(self.exports(target, seen))
        return names

    def origin(self, module: Module, name: str, _depth: int = 0) -> tuple[Module | None, str]:
        """The module and name that declare what ``module.name`` refers to, following
        ``from x import y`` and star imports through the project's own files."""
        if _depth > 20:
            return module, name
        b = module.bindings.get(name)
        if isinstance(b, NameImport):
            return self._imported_origin(b, name, _depth)
        if b is not None:
            return module, name
        for star in module.stars:
            target = self.module(star)
            if target is not None and name in self.exports(target):
                return self.origin(target, name, _depth + 1)
        return module, name

    def _imported_origin(self, b: NameImport, name: str, depth: int) -> tuple[Module | None, str]:
        """The origin of a name a module binds by ``from x import y``."""
        target = self.module(b.module)
        if target is None:
            return None, name
        if self._names_submodule(target, b.original):
            sub = self.module(f"{target.name}.{b.original}")
            return (sub, TOP_LEVEL) if sub else (None, name)
        return self.origin(target, b.original, depth + 1)

    def defines(self, module: Module, name: str) -> bool | None:
        """Whether ``from module import name`` can work; None when syntax cannot tell."""
        if name in module.bindings or self.is_submodule(module, name):
            return True
        if name in ("__all__", "__doc__", "__file__", "__name__", "__path__", "__version__"):
            return True if name != "__version__" else None
        if module.has_getattr or module.error:
            return None
        for star in module.stars:
            target = self.module(star)
            if target is None:
                return None
            found = self.defines(target, name)
            if found is not False:
                return found
        return False
