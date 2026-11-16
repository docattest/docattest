"""The setup module's named facts, called for ``observables``: the one place the adapter calls
code only to see what it returns, because named facts exist for pages to read. A surface's
functions are never called."""

from __future__ import annotations

import inspect
from collections.abc import Callable
from typing import cast

from .asserts import finish
from .environment import Environment
from .facts import setup_facts
from .jobs import SetupModule


def named_fact_values(
    env: Environment, setup: SetupModule, surface: dict[str, object]
) -> list[tuple[str, object]]:
    """Each named fact that takes no arguments, called: the fact expression that calls it, and
    what it returned. One that shares its name with an export of ``surface`` is left out, as a
    fact cannot use it; so is one that raises, as an export that cannot be read is."""
    helpers = setup_facts(env, setup)
    if isinstance(helpers, str):
        return []
    out: list[tuple[str, object]] = []
    for name, helper in helpers.items():
        call = _without_arguments(helper)
        if call is None or not name.isidentifier() or name in surface:
            continue
        try:
            returned = call()
            awaited = inspect.isawaitable(returned)
            value = finish(returned)
        except BaseException:  # noqa: BLE001 - a helper that raises offers nothing
            continue
        out.append(("(await %s())" % name if awaited else "%s()" % name, value))
    return out


def _without_arguments(helper: object) -> Callable[[], object] | None:
    """A function (not a class) that can be called without arguments, or None."""
    if not callable(helper) or inspect.isclass(helper):
        return None
    try:
        parameters = inspect.signature(helper).parameters.values()
    except (TypeError, ValueError):
        return None
    variadic = (inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD)
    if any(p.default is inspect.Parameter.empty and p.kind not in variadic for p in parameters):
        return None
    return cast("Callable[[], object]", helper)
