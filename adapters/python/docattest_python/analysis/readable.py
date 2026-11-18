"""What a fact may read of a surface, decided from syntax: its constants, from source, and
its other exports, by importing it. A program (``__main__.py``) is never imported, since
importing it would run it, so of a program a fact reads only constants."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass

from .constants import constants
from .modules import Declaration, Module
from .surfaces import Surface

_KINDS = {"def": "a function", "class": "a class", "assign": "a variable"}


@dataclass(frozen=True)
class Readable:
    """What facts read of one surface."""

    #: The names read by importing the surface; None when they are not known, and then
    #: every fact imports it.
    imported: frozenset[str] | None
    #: The constants, by name, with their values read from source.
    constants: dict[str, object]
    #: The names the module declares that a fact cannot read, each with why.
    refused: dict[str, str]

    def imports(self, names: Iterable[str]) -> bool:
        """Whether a fact that reads these names needs the surface imported."""
        if self.imported is None:
            return True
        return any(name in self.imported and name not in self.constants for name in names)


#: What facts read of a surface whose source cannot be read: whatever importing it gives.
UNREAD = Readable(None, {}, {})


def readable(surface: Surface, module: Module, exports: list[str]) -> Readable:
    """What facts read of a surface, read from ``module``, whose exports are ``exports``."""
    found = constants(module)
    imported = frozenset[str]() if surface.is_program else frozenset(exports)
    refused: dict[str, str] = {}
    for name, binding in module.bindings.items():
        if isinstance(binding, Declaration) and name not in found and name not in imported:
            refused[name] = _refusal(surface, name, _KINDS[binding.kind])
    return Readable(imported, found, refused)


def _refusal(surface: Surface, name: str, kind: str) -> str:
    if surface.is_program:
        return (
            f'{name} is {kind} of surface "{surface.name}", a program (`__main__.py`); '
            "of a program, a fact reads only constants"
        )
    return (
        f'{name} is {kind} that surface "{surface.name}" does not export; '
        "of its unexported names, a fact reads only constants"
    )
