"""What a configured surface is, for Python.

A surface entry in ``docattest.config.json`` is one of:

- a module name, ``"mypkg"`` or ``"mypkg.retry"``: imported as written, found the way
  ``import`` finds it, the project root and its ``src`` directory first;
- a path, ``"./src/mypkg"`` or ``"./mypkg/retry.py"`` (it starts with ``.`` or ``/``,
  contains a ``/`` or ends in ``.py``): that file of the project, imported under the name
  Python gives it, which is the path from the first directory that is not a package;
- both, ``{"import": "mypkg", "path": "build/lib/mypkg/__init__.py"}``: examples import
  ``import``, and it is read from ``path``.
"""

from __future__ import annotations

import os
import re
import sys
from collections.abc import Callable
from dataclasses import dataclass
from typing import cast

from .modules import Module
from .project import Project


@dataclass(frozen=True)
class SurfaceAlias:
    """``{"import": ..., "path": ...}``: imported by one name, read from one file."""

    import_name: str
    path: str


@dataclass(frozen=True)
class ConfiguredSurface:
    """A surface as ``docattest.config.json`` names it."""

    name: str
    entry: str | SurfaceAlias


#: Finds the file of a module outside the project, by its name.
FindInstalled = Callable[[str], str | None]


@dataclass(frozen=True)
class Package:
    """The distribution a surface belongs to, for the report."""

    name: str
    version: str | None

    def to_json(self) -> dict[str, str]:
        """As the protocol's ``SurfaceResolution.package``. ``display`` is how Python
        writes one: ``name 1.2.0``, as ``pip list`` and ``pip show`` do."""
        if self.version is None:
            return {"name": self.name, "display": self.name}
        return {"name": self.name, "version": self.version, "display": f"{self.name} {self.version}"}


class Surface:
    """A configured surface, resolved against one revision of the project."""

    def __init__(self, name: str) -> None:
        self.name = name
        self.import_name = ""
        #: The file it is read from, absolute; None when it did not resolve.
        self.file: str | None = None
        #: The path as the config wrote it, for messages.
        self.written: str | None = None
        #: A directory that must be on the import path for ``import_name`` to find ``file``.
        self.base: str | None = None
        #: True when ``import_name`` is not what the file would be imported as by itself.
        self.alias = False
        self.module: Module | None = None


def is_path(entry: str) -> bool:
    return entry.startswith((".", "/")) or "/" in entry or "\\" in entry or entry.endswith(".py")


def _file_of(project: Project, root: str, written: str) -> tuple[str, bool]:
    """The module file a written path names, and whether it exists."""
    path = os.path.normpath(os.path.join(root, written))
    if project.read(path) is not None and path.endswith(".py"):
        return path, True
    init = os.path.join(path, "__init__.py")
    if project.read(init) is not None:
        return init, True
    return path, False


def resolve(
    project: Project, root: str, configured: list[ConfiguredSurface], installed: FindInstalled | None = None
) -> list[Surface]:
    """Resolve every configured surface against one revision of the project. ``installed``
    finds a module outside the project (None for a revision that is not on disk)."""
    return [_resolve_one(project, root, item, installed) for item in configured]


def _resolve_one(
    project: Project, root: str, item: ConfiguredSurface, installed: FindInstalled | None
) -> Surface:
    surface = Surface(item.name)
    entry = item.entry
    if isinstance(entry, SurfaceAlias):
        _resolve_both(surface, project, root, entry)
    elif is_path(entry):
        _resolve_path(surface, project, root, entry)
    else:
        _resolve_name(surface, project, entry, installed)
    if surface.file is not None and surface.file.endswith(".py"):
        surface.module = project.load(surface.import_name, surface.file)
    return surface


def _resolve_both(surface: Surface, project: Project, root: str, entry: SurfaceAlias) -> None:
    """``{"import": ..., "path": ...}``: imported by one name, read from one file."""
    surface.import_name = entry.import_name
    surface.written = entry.path
    file, exists = _file_of(project, root, surface.written)
    if not exists:
        return
    surface.file = file
    natural, base = project.module_name(file)
    if natural == surface.import_name:
        surface.base = base
    else:
        surface.alias = True


def _resolve_path(surface: Surface, project: Project, root: str, entry: str) -> None:
    """A path of the project, imported under the name Python gives it."""
    surface.written = entry
    file, exists = _file_of(project, root, entry)
    natural, base = project.module_name(file if exists else _guess(file))
    surface.import_name = natural
    if exists:
        surface.file = file
        surface.base = base


def _resolve_name(surface: Surface, project: Project, entry: str, installed: FindInstalled | None) -> None:
    """A module name, found the way ``import`` finds it, the project first."""
    surface.import_name = entry
    surface.file = project.find(entry)
    if surface.file is None and installed is not None:
        surface.file = installed(entry)


def _guess(path: str) -> str:
    return path if path.endswith(".py") else os.path.join(path, "__init__.py")


def search_dirs(root: str, isdir: Callable[[str], bool] = os.path.isdir) -> list[str]:
    """Where project modules are looked for, in order: the project root, then ``src``."""
    dirs = [root]
    src = os.path.join(root, "src")
    if isdir(src):
        dirs.append(src)
    return dirs


# --- the package a surface belongs to ---------------------------------------------------

_SECTION = re.compile(r"^\s*\[([^\]]+)\]\s*$")
_PAIR = re.compile(r"""^\s*(name|version)\s*=\s*(['"])(.*?)\2\s*(?:#.*)?$""")


def _pyproject(text: str) -> Package | None:
    name, version = _name_and_version(text)
    return Package(name, version) if name else None


def _name_and_version(text: str) -> tuple[str | None, str | None]:
    """The name and version of ``[project]`` or ``[tool.poetry]``; a pyproject that does
    not parse names no package."""
    if sys.version_info < (3, 11):
        return _scan_pyproject(text)
    import tomllib

    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError:
        return None, None
    tool: object = data.get("tool")
    poetry: object = cast("dict[str, object]", tool).get("poetry") if isinstance(tool, dict) else None
    project: object = data.get("project") or poetry
    if not isinstance(project, dict):
        return None, None
    table = cast("dict[str, object]", project)
    name, version = table.get("name"), table.get("version")
    return (name if isinstance(name, str) else None), (version if isinstance(version, str) else None)


def _scan_pyproject(text: str) -> tuple[str | None, str | None]:
    """The name and version of ``[project]`` or ``[tool.poetry]``, read line by line where
    there is no ``tomllib`` (Python 3.10)."""
    name = version = None
    section = ""
    for line in text.splitlines():
        head = _SECTION.match(line)
        if head:
            section = head.group(1).strip()
            continue
        pair = _PAIR.match(line) if section in ("project", "tool.poetry") else None
        if pair and pair.group(1) == "name":
            name = name or pair.group(3)
        elif pair:
            version = version or pair.group(3)
    return name, version


def _setup_cfg(text: str) -> Package | None:
    import configparser

    parser = configparser.ConfigParser()
    try:
        parser.read_string(text)
        name = parser.get("metadata", "name", fallback=None)
        version = parser.get("metadata", "version", fallback=None)
    except configparser.Error:
        return None
    if not name:
        return None
    literal = version and not version.startswith(("attr:", "file:"))
    return Package(name, version if literal else None)


def nearest_package(project: Project, root: str, file: str) -> Package | None:
    """The distribution a file of the project belongs to: the nearest ``pyproject.toml``
    (or ``setup.cfg``) at or above it that names one, inside the project."""
    directory = os.path.dirname(file)
    root = os.path.normpath(root)
    readers: list[tuple[str, Callable[[str], Package | None]]] = [
        ("pyproject.toml", _pyproject),
        ("setup.cfg", _setup_cfg),
    ]
    while True:
        for name, parse in readers:
            text = project.read(os.path.join(directory, name))
            found = parse(text) if text is not None else None
            if found:
                return found
        parent = os.path.dirname(directory)
        if os.path.normpath(directory) == root or parent == directory:
            return None
        if not os.path.normpath(directory).startswith(root):
            return None
        directory = parent


def installed_package(file: str) -> Package | None:
    """The installed distribution that owns a file outside the project, if any."""
    from importlib import metadata

    directory = os.path.dirname(file)
    site = directory
    while site and os.path.basename(site) not in ("site-packages", "dist-packages"):
        parent = os.path.dirname(site)
        if parent == site:
            return None
        site = parent
    relative = os.path.relpath(file, site).replace(os.sep, "/")
    try:
        for dist in metadata.distributions(path=[site]):
            if any(str(f) == relative for f in (dist.files or [])):
                return Package(dist.name, dist.version)
    except (OSError, ValueError):  # unreadable or broken metadata: the package is a courtesy
        return None
    return None
