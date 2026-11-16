"""The session job: an interpreter session (``>>>`` prompts), run as doctest runs it.

The worker imports this module only when it has a session to run: doctest is slow to
import, and the adapter preloads it before any job is forked when the pages hold sessions.
"""

from __future__ import annotations

import doctest
import sys
from collections.abc import Callable
from types import TracebackType
from typing import NamedTuple

from .channel import Channel, Sink
from .environment import Environment
from .examples import completed, run_before
from .failures import describe_error, failure_message
from .jobs import BlockJob
from .messages import Verdict

Output = Callable[[str], object]
ExcInfo = tuple[type[BaseException], BaseException, TracebackType]


class SessionFailure(NamedTuple):
    """A statement of a session that failed: what it printed, or what it raised."""

    example: doctest.Example
    got: str | None
    raised: BaseException | None


def run_session(job: BlockJob, env: Environment, channel: Channel) -> None:
    """An interpreter session, as doctest runs it. A docstring's session starts with the
    names of its module, as ``python -m doctest`` gives it."""
    filename, source = job["file"], job["code"]
    env.enter(job["network"])
    namespace = env.fresh_main(filename)
    broken = _import_names(job["module"], namespace)
    if broken is None:
        broken = run_before(job["before"], namespace)
    if broken:
        channel.post({"t": "verdict", "verdict": {"status": "fail", "message": broken}})
        return
    try:
        test, failures = _run_doctest(source, namespace, filename)
    except BaseException as err:  # noqa: BLE001
        channel.post({"t": "verdict", "verdict": {**describe_error(err, where=None), "status": "fail"}})
        return
    sys.stdout = Sink()  # type: ignore[assignment]  # a file in all that print uses
    if not failures:
        ran = any(not e.options.get(doctest.SKIP) for e in test.examples)
        verdict = completed(env, job["weak"] or not ran, "every output matched")
    else:
        verdict = _session_failure(failures)
    channel.post({"t": "verdict", "verdict": verdict})


def run_quietly(source: str, namespace: dict[str, object], filename: str) -> None:
    """Run a session in ``namespace`` and compare nothing: what it leaves behind is the
    point."""
    test = doctest.DocTestParser().get_doctest(source, namespace, "session", filename, 0)
    test.globs = namespace  # doctest runs in a copy otherwise
    _QuietRunner(verbose=False, optionflags=doctest.ELLIPSIS).run(test, out=_discard, clear_globs=False)


def _discard(text: str) -> None:
    pass


class _QuietRunner(doctest.DocTestRunner):
    """Runs a session and reports nothing."""

    def report_start(self, out: Output, test: doctest.DocTest, example: doctest.Example) -> None:
        pass

    def report_success(self, out: Output, test: doctest.DocTest, example: doctest.Example, got: str) -> None:
        pass

    def report_failure(self, out: Output, test: doctest.DocTest, example: doctest.Example, got: str) -> None:
        pass

    def report_unexpected_exception(
        self, out: Output, test: doctest.DocTest, example: doctest.Example, exc_info: ExcInfo
    ) -> None:
        pass


class _CollectingRunner(_QuietRunner):
    """Runs a session and keeps each statement that failed."""

    def __init__(self) -> None:
        super().__init__(verbose=False, optionflags=doctest.ELLIPSIS)
        self.failed: list[SessionFailure] = []

    def report_failure(self, out: Output, test: doctest.DocTest, example: doctest.Example, got: str) -> None:
        self.failed.append(SessionFailure(example, got, None))

    def report_unexpected_exception(
        self, out: Output, test: doctest.DocTest, example: doctest.Example, exc_info: ExcInfo
    ) -> None:
        self.failed.append(SessionFailure(example, None, exc_info[1]))


def _import_names(module: str | None, namespace: dict[str, object]) -> str | None:
    """Give a docstring's session the names of its module; a message when it cannot be
    imported."""
    if not module:
        return None
    import importlib

    try:
        namespace.update(vars(importlib.import_module(module)))
        namespace["__name__"] = "__main__"
    except BaseException as err:  # noqa: BLE001
        return "cannot import %s for its docstring examples: %s" % (module, failure_message(err))
    return None


def _run_doctest(
    source: str, namespace: dict[str, object], filename: str
) -> tuple[doctest.DocTest, list[SessionFailure]]:
    """Run a session; returns the test and the statements that failed."""
    test = doctest.DocTestParser().get_doctest(source, namespace, "session", filename, 0)
    runner = _CollectingRunner()
    runner.run(test, out=_discard, clear_globs=False)
    return test, runner.failed


def _session_failure(failures: list[SessionFailure]) -> Verdict:
    """The verdict on a session from its first failing statement."""
    example, got, raised = failures[0]
    statement = "`%s`" % " ".join(example.source.strip().split("\n"))
    at = " (block line %d)" % (example.lineno + 1)
    if raised is not None:
        verdict: Verdict = {
            "status": "fail",
            "message": "%s raised %s%s" % (statement, failure_message(raised), at),
        }
    else:
        verdict = _output_differs(example, got or "", statement, at)
    if len(failures) > 1:
        verdict["message"] += "; and %d more failing in this session" % (len(failures) - 1)
    return verdict


def _output_differs(example: doctest.Example, got: str, statement: str, at: str) -> Verdict:
    """The verdict on a statement whose output is not what the page shows."""
    want, have = example.want.strip(), got.strip()
    raised = have.startswith("Traceback (most recent call last)")
    if raised:
        have = have.split("\n")[-1]
    expected, actual = want or "(no output)", have or "(no output)"
    wanted, had = example.want.rstrip("\n").split("\n"), got.rstrip("\n").split("\n")
    if (len(wanted) > 1 or len(had) > 1) and not raised:
        # A long output: say where it first differs; both sides are in the result.
        message = _first_difference(wanted, had, statement, at)
    else:
        message = "%s: expected %s, got %s%s" % (statement, expected, actual, at)
    return {"status": "fail", "expected": expected, "actual": actual, "message": message}


def _first_difference(wanted: list[str], had: list[str], statement: str, at: str) -> str:
    """Where two outputs, as lists of lines, first differ."""
    k = next((i for i, (a, b) in enumerate(zip(wanted, had)) if a != b), min(len(wanted), len(had)))
    a = repr(wanted[k]) if k < len(wanted) else "the end of the output"
    b = repr(had[k]) if k < len(had) else "the end of the output"
    return "%s: output line %d differs: expected %s, got %s%s" % (statement, k + 1, a, b, at)
