"""Values: how the core is told a value, and how a message shows one."""

from __future__ import annotations

import enum
from collections.abc import Iterable
from typing import cast

from .messages import Observation


def safe_str(value: object) -> str:
    try:
        return str(value)
    except BaseException:  # noqa: BLE001 - a broken __str__ must not lose the fact
        return object.__repr__(value)


def describe_value(value: object) -> str:
    if value is None:
        return "None"
    if isinstance(value, str):
        return "str " + repr(value)
    if isinstance(value, (bool, int, float, complex)):
        return "%s %s" % (type(value).__name__, safe_str(value))
    name = type(value).__name__
    return ("an " if name[:1].lower() in "aeiou" else "a ") + name


def observe(value: object) -> Observation:
    """A value as the core compares it: its ``str``, and its items when it is iterable.

    A string is a scalar on a page, so it has no items; neither have bytes. A dict's items
    are its keys, as iterating it gives them.
    """
    observation: Observation = {"text": safe_str(value), "description": describe_value(value)}
    if isinstance(value, str):
        # The one thing about a value's type the core reads: only text can be what a
        # fenced block copies (`audit`).
        observation["kind"] = "text"
    if value is None or isinstance(value, (str, bytes, bytearray)):
        return observation
    try:
        # Whether it iterates is known only by trying: ``iter`` raises when it does not.
        iterator = iter(cast("Iterable[object]", value))
    except TypeError:
        return observation
    observation["items"] = [safe_str(item) for item in iterator]
    return observation


def is_enum(value: object) -> bool:
    return isinstance(value, type) and issubclass(value, enum.Enum)


def show(value: object) -> str:
    """A value for a message: its repr on one line, bounded."""
    try:
        text = repr(value)
    except BaseException:  # noqa: BLE001
        text = object.__repr__(value)
    text = " ".join(text.split("\n"))
    return text if len(text) <= 400 else text[:397] + "..."
