"""The facts job: each fact of a page evaluated against its surface and the setup
module's named facts."""

from __future__ import annotations

import ast
import builtins
import sys
from typing import NamedTuple, cast

from .asserts import run_code
from .channel import Channel
from .environment import Environment, source_constants, surface_namespace
from .failures import failure_message
from .jobs import FactsJob, FactToEvaluate, SetupModule, SurfaceToLoad
from .messages import Evaluation
from .values import observe

#: What every fact expression may call besides the surface: methods of the environment, by
#: the names facts call them.
FACT_FUNCTIONS = ("snapshot", "manifest", "file")


def run_facts(job: FactsJob, env: Environment, channel: Channel, start: int) -> None:
    """Each fact from ``start``. The surface is imported only when a fact reads a name that
    nothing else gives, so a failure to import it fails only those facts."""
    env.enter(False)
    facts = job["facts"][start:]
    imported, failure = _imported(job["surface"], facts)
    channel.post({"t": "ready"})
    namespace = {**imported, **source_constants(job["surface"])}
    surface = _Surface(job["surface"]["name"], job["refused"])
    helpers = _Helpers(env, job["setup"], namespace)
    for fact in facts:
        if fact["imports"] and failure is not None:
            evaluation: Evaluation = {"ok": False, "message": failure}
        else:
            evaluation = evaluate(fact, surface, helpers, env)
        channel.post({"t": "evaluation", "evaluation": evaluation})


def _imported(surface: SurfaceToLoad, facts: list[FactToEvaluate]) -> tuple[dict[str, object], str | None]:
    """The surface's exports when a fact needs them imported, or why they could not be."""
    if not any(fact["imports"] for fact in facts):
        return {}, None
    try:
        return surface_namespace(surface), None
    except BaseException as err:  # noqa: BLE001
        return {}, 'surface "%s" failed to load: %s' % (surface["name"], failure_message(err))


class _Surface(NamedTuple):
    """A fact's surface: its name, and the names of it a fact may not read, each with why."""

    name: str
    refused: dict[str, str]


def ambiguous(name: str, surface: str) -> str:
    return '"%s" is both an export of surface "%s" and a setup fact; rename the setup fact' % (name, surface)


class _Helpers:
    """Named facts of the setup module, loaded only when an expression needs one."""

    def __init__(self, env: Environment, setup: SetupModule | None, surface: dict[str, object]) -> None:
        self.env = env
        self.setup = setup
        self.surface = surface
        self.loaded: dict[str, object] | str | None = None

    def bindings(self, fact: FactToEvaluate, surface_name: str) -> dict[str, object] | str:
        names = fact["names"]
        needs = any(self._only_in_setup(n) for n in names)
        if not needs or self.setup is None:
            return self.surface
        helpers = self.load(self.setup)
        if isinstance(helpers, str):
            return helpers
        for name in names:
            if name in self.surface and name in helpers:
                return ambiguous(name, surface_name)
        merged = dict(helpers)
        merged.update(self.surface)
        return merged

    def _only_in_setup(self, name: str) -> bool:
        """Whether a name the expression reads can come from nowhere but the setup facts."""
        return (
            name not in self.surface
            and name not in FACT_FUNCTIONS
            and not hasattr(builtins, name)
            and not _stdlib(name)
        )

    def load(self, setup: SetupModule) -> dict[str, object] | str:
        if self.loaded is None:
            self.loaded = setup_facts(self.env, setup)
        return self.loaded


def setup_facts(env: Environment, setup: SetupModule) -> dict[str, object] | str:
    """The setup module's named facts, or why they cannot be had."""
    label = setup["label"]
    try:
        facts: object = getattr(env.load_setup(setup), "facts", None)
    except BaseException as err:  # noqa: BLE001
        reason = failure_message(err, setup["file"], "line")
        return "setup module %s failed to load: %s" % (label, reason)
    if facts is None:
        return {}
    if isinstance(facts, dict):
        return {str(k): v for k, v in cast("dict[object, object]", facts).items()}
    return "%s defines `facts`, but it is not a dict" % label


def _stdlib(name: str) -> bool:
    """Whether a name is a module of the standard library (``math``, ``inspect``, ``re``)."""
    return name in getattr(sys, "stdlib_module_names", ()) and not name.startswith("_")


def evaluate(fact: FactToEvaluate, surface: _Surface, helpers: _Helpers, env: Environment) -> Evaluation:
    expression = fact["expression"]
    label = "fact `%s`" % expression
    bindings = helpers.bindings(fact, surface.name)
    if isinstance(bindings, str):
        return {"ok": False, "message": "%s: %s" % (label, bindings)}
    scope = _scope(fact, bindings, env)
    try:
        code = compile(
            "(\n%s\n)" % expression, "<fact>", "eval", flags=ast.PyCF_ALLOW_TOP_LEVEL_AWAIT, dont_inherit=True
        )
        value = run_code(code, scope)
        return {"ok": True, "value": observe(value)}
    except NameError as err:
        name = getattr(err, "name", None)
        if name in surface.refused:
            return {"ok": False, "message": "%s: %s" % (label, surface.refused[name])}
        if name and name not in scope:
            where = 'surface "%s"' % surface.name
            return {
                "ok": False,
                "message": "%s: %s is not exported by %s (NameError: %s)" % (label, name, where, err),
            }
        return _raised(label, err)
    except BaseException as err:  # noqa: BLE001
        return _raised(label, err)


def _scope(fact: FactToEvaluate, bindings: dict[str, object], env: Environment) -> dict[str, object]:
    """The globals an expression is evaluated in."""
    scope = dict(bindings)
    scope["__builtins__"] = builtins
    for name in FACT_FUNCTIONS:
        scope[name] = getattr(env, name)
    # A fact cannot import, so a standard-library module is there by its name, as `Math`
    # and `JSON` are in JavaScript. The surface and the setup facts come first.
    for name in fact["names"]:
        if name not in scope and not hasattr(builtins, name) and _stdlib(name):
            try:
                scope[name] = __import__(name)
            except ImportError:
                pass
    return scope


def _raised(label: str, err: BaseException) -> Evaluation:
    return {"ok": False, "message": "%s raised %s" % (label, failure_message(err))}
