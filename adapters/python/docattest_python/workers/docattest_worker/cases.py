"""The cases job: each row of a table, run by a runner of the setup module."""

from __future__ import annotations

import types
from collections.abc import Callable, Iterable
from typing import cast

from .asserts import finish
from .channel import Channel
from .environment import Environment
from .failures import describe_error, failure_message
from .jobs import CasesJob, CasesRow
from .messages import Verdict

Runner = Callable[[dict[str, str]], object]


def run_cases(job: CasesJob, env: Environment, channel: Channel, start: int) -> None:
    env.enter(False)
    at = 'cases "%s" at L%d' % (job["runner"], job["line"])
    setup = job["setup"]
    label = setup["label"]
    try:
        module = env.load_setup(setup)
    except BaseException as err:  # noqa: BLE001
        reason = failure_message(err, setup["file"], "line")
        message = "%s: setup module %s failed to load: %s" % (at, label, reason)
        channel.post({"t": "failed", "message": message, "internal": None})
        return
    runner = _runner(job, module, label, at)
    if isinstance(runner, str):
        channel.post({"t": "failed", "message": runner, "internal": None})
        return
    channel.post({"t": "ready"})
    for row in job["rows"][start:]:
        channel.post({"t": "verdict", "verdict": _run_row(job["page"], row, runner, env, setup["file"])})


def _runner(job: CasesJob, module: types.ModuleType, label: str, at: str) -> Runner | str:
    """The table's runner, or why it cannot run."""
    cases: object = getattr(module, "cases", None)
    if not isinstance(cases, dict):
        return "%s: %s defines no `cases` dict" % (at, label)
    runners = cast("dict[object, object]", cases)
    runner = runners.get(job["runner"])
    if not callable(runner):
        known = ", ".join(str(k) for k in runners) or "none"
        return 'cases runner "%s" at L%d is not in `cases` of %s (known: %s)' % (
            job["runner"],
            job["line"],
            label,
            known,
        )
    missing = _missing_columns(runner, job["header"])
    if missing:
        noun = "columns" if len(missing) > 1 else "column"
        listed = ", ".join('"%s"' % m for m in missing)
        return "%s: the table is missing %s %s declared by the runner" % (at, noun, listed)
    return runner


def _missing_columns(runner: object, header: list[str]) -> list[str]:
    """The columns the runner declares (``runner.columns``) that the table does not have."""
    columns: object = getattr(runner, "columns", None)
    declared = (
        [str(c) for c in cast("Iterable[object]", columns)] if isinstance(columns, (list, tuple)) else []
    )
    return [c for c in declared if c not in header]


def _run_row(page: str, row: CasesRow, runner: Runner, env: Environment, filename: str) -> Verdict:
    """One row's verdict: it passes when its runner returns and nothing went unhandled."""
    name = "%s:%d %s" % (page, row["line"], row["name"])
    del env.strays[:]
    try:
        finish(runner(dict(row["values"])))
    except BaseException as err:  # noqa: BLE001
        failure = describe_error(err, filename, where=None)
        return {**failure, "status": "fail", "message": "%s: %s" % (name, failure["message"])}
    if env.strays:
        return {
            "status": "fail",
            "message": "%s: unhandled error from attested code: %s" % (name, "; ".join(env.strays)),
        }
    return {"status": "pass", "message": name}
