"""What the worker writes back to the adapter: one JSON line per message, on the pipe that
was its stdout.

The worker builds these types and the pool reads them back with ``read_message``. Nothing
but the worker writes to that pipe, so a line that is not one of these messages is a fault
in the worker, and the pool stops the job rather than skip it.

An evaluation, an observation and an observable are the protocol's own shapes (the
``Evaluation``, ``Observation`` and ``Observable`` of ``results.json``): the adapter passes
them on to the core as they are.
"""

from __future__ import annotations

import json
from typing import Literal, TypedDict

from .fields import Fields, MalformedMessage


class _ObservationExtras(TypedDict, total=False):
    items: list[str]
    kind: Literal["text"]


class Observation(_ObservationExtras):
    text: str
    description: str


class Observable(TypedDict):
    surface: str
    expression: str
    path: list[str]
    value: Observation


class EvaluationOk(TypedDict):
    ok: Literal[True]
    value: Observation


class EvaluationFailed(TypedDict):
    ok: Literal[False]
    message: str


Evaluation = EvaluationOk | EvaluationFailed


class Sides(TypedDict, total=False):
    """Both sides of a failed comparison, when there was one."""

    expected: str
    actual: str


class Verdict(Sides):
    status: Literal["pass", "fail"]
    message: str


class HelloMessage(TypedDict):
    t: Literal["hello"]
    #: Whether each job runs in a forked child; without, the worker runs one job and exits.
    fork: bool


class ReadyMessage(TypedDict):
    """The job is prepared (its surface or setup module loaded); its items follow."""

    t: Literal["ready"]


class VerdictItem(TypedDict):
    t: Literal["verdict"]
    verdict: Verdict


class EvaluationItem(TypedDict):
    t: Literal["evaluation"]
    evaluation: Evaluation


class ObservablesItem(TypedDict):
    t: Literal["observables"]
    observables: list[Observable]


class FailedMessage(TypedDict):
    """The job could not be prepared, or the worker itself failed."""

    t: Literal["failed"]
    message: str
    #: A traceback for the adapter's stderr, when the worker itself failed.
    internal: str | None


class ExitMessage(TypedDict):
    """The child that ran the job exited, with this code."""

    t: Literal["exit"]
    code: int


Item = VerdictItem | EvaluationItem | ObservablesItem
Message = (
    HelloMessage | ReadyMessage | VerdictItem | EvaluationItem | ObservablesItem | FailedMessage | ExitMessage
)


def read_message(line: bytes) -> Message:
    """One line the worker wrote, as its message."""
    try:
        value: object = json.loads(line)
    except ValueError as err:
        raise MalformedMessage(f"a line from the worker is not JSON ({err}): {line[:80]!r}") from err
    fields = Fields(value, "a message from the worker")
    kind = fields.string("t")
    if kind == "hello":
        return {"t": "hello", "fork": fields.boolean("fork")}
    if kind == "ready":
        return {"t": "ready"}
    if kind == "verdict":
        return {"t": "verdict", "verdict": _read_verdict(fields.object("verdict"))}
    if kind == "evaluation":
        return {"t": "evaluation", "evaluation": _read_evaluation(fields.object("evaluation"))}
    if kind == "observables":
        return {
            "t": "observables",
            "observables": [_read_observable(o) for o in fields.objects("observables")],
        }
    if kind == "failed":
        return {
            "t": "failed",
            "message": fields.string("message"),
            "internal": fields.optional_string("internal"),
        }
    if kind == "exit":
        return {"t": "exit", "code": fields.integer("code")}
    raise MalformedMessage(f'{fields.where}: unknown message "{kind}"')


def _read_verdict(fields: Fields) -> Verdict:
    status = fields.string("status")
    if status != "pass" and status != "fail":
        raise MalformedMessage(f'{fields.where}: "status" must be "pass" or "fail", not "{status}"')
    verdict: Verdict = {"status": status, "message": fields.string("message")}
    if fields.has("expected"):
        verdict["expected"] = fields.string("expected")
    if fields.has("actual"):
        verdict["actual"] = fields.string("actual")
    return verdict


def _read_evaluation(fields: Fields) -> Evaluation:
    if fields.boolean("ok"):
        return {"ok": True, "value": _read_observation(fields.object("value"))}
    return {"ok": False, "message": fields.string("message")}


def _read_observation(fields: Fields) -> Observation:
    observation: Observation = {"text": fields.string("text"), "description": fields.string("description")}
    if fields.has("items"):
        observation["items"] = fields.strings("items")
    if fields.has("kind"):
        if fields.string("kind") != "text":
            raise MalformedMessage(f'{fields.where}: "kind" must be "text"')
        observation["kind"] = "text"
    return observation


def _read_observable(fields: Fields) -> Observable:
    return {
        "surface": fields.string("surface"),
        "expression": fields.string("expression"),
        "path": fields.strings("path"),
        "value": _read_observation(fields.object("value")),
    }
