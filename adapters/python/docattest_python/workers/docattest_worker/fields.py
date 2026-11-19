"""Typed reads of a JSON object's fields: where JSON enters, a missing or mistyped field is
an error that names it, never a silent default.

The adapter reads the core's requests with it, the worker reads its jobs, and the pool
reads what the worker writes back. It lives with the worker, which may run on another
interpreter and imports nothing from outside this package.
"""

from __future__ import annotations

from typing import NoReturn, cast


class MalformedMessage(ValueError):
    """A JSON value that is not the shape its reader expects."""


class Fields:
    """The fields of one JSON object; ``where`` says which object, for messages."""

    def __init__(self, value: object, where: str) -> None:
        if not isinstance(value, dict):
            raise MalformedMessage(f"{where} must be an object")
        self._fields = cast("dict[str, object]", value)
        self.where = where

    def has(self, name: str) -> bool:
        return name in self._fields

    def holds_object(self, name: str) -> bool:
        """Whether the field is an object, for a field that may take one of two shapes."""
        return isinstance(self._fields.get(name), dict)

    def is_null(self, name: str) -> bool:
        """Whether the field is null or missing."""
        return self._fields.get(name) is None

    def string(self, name: str) -> str:
        value = self._fields.get(name)
        return value if isinstance(value, str) else self._fail(name, "a string")

    def optional_string(self, name: str) -> str | None:
        """A string, or None when the field is missing or null."""
        return None if self.is_null(name) else self.string(name)

    def boolean(self, name: str) -> bool:
        value = self._fields.get(name)
        return value if isinstance(value, bool) else self._fail(name, "a boolean")

    def number(self, name: str) -> float:
        value = self._fields.get(name)
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            return self._fail(name, "a number")
        return value

    def integer(self, name: str) -> int:
        value = self._fields.get(name)
        if isinstance(value, bool) or not isinstance(value, int):
            return self._fail(name, "an integer")
        return value

    def array(self, name: str) -> list[object]:
        value = self._fields.get(name)
        return cast("list[object]", value) if isinstance(value, list) else self._fail(name, "an array")

    def strings(self, name: str) -> list[str]:
        items = self.array(name)
        strings = [item for item in items if isinstance(item, str)]
        return strings if len(strings) == len(items) else self._fail(name, "an array of strings")

    def objects(self, name: str) -> list[Fields]:
        """Each item of an array of objects, as fields of its own."""
        return [Fields(item, f'{self.where}: "{name}"[{k}]') for k, item in enumerate(self.array(name))]

    def object(self, name: str) -> Fields:
        return Fields(self.json_object(name), f'{self.where}: "{name}"')

    def json_object(self, name: str) -> dict[str, object]:
        """An object whose contents are passed on, not read here."""
        value = self._fields.get(name)
        if not isinstance(value, dict):
            return self._fail(name, "an object")
        return cast("dict[str, object]", value)

    def string_map(self, name: str) -> dict[str, str]:
        found = self.json_object(name)
        strings = {k: v for k, v in found.items() if isinstance(v, str)}
        return strings if len(strings) == len(found) else self._fail(name, "a map of strings")

    def string_maps(self, name: str) -> dict[str, dict[str, str]]:
        """An object whose values are each a map of strings."""
        found = self.object(name)
        return {key: found.string_map(key) for key in found.keys()}

    def json_objects(self, name: str) -> dict[str, dict[str, object]]:
        """An object whose values are each an object passed on, not read here."""
        found = self.object(name)
        return {key: found.json_object(key) for key in found.keys()}

    def keys(self) -> list[str]:
        """The object's keys, in order."""
        return list(self._fields)

    def _fail(self, name: str, what: str) -> NoReturn:
        raise MalformedMessage(f'{self.where}: "{name}" must be {what}')
