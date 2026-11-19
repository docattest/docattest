"""How each test of a unittest run ended, read from the driver's report
(``unittest_driver``): one JSON line per outcome unittest reported."""

from __future__ import annotations

import json
from collections.abc import Sequence
from dataclasses import dataclass, field

from ..analysis.declared_tests.subtests import subtest_title
from ..workers.docattest_worker.fields import Fields
from .outcome import PASSED, TestRun, all_of, failed, not_run, skipped, titled, with_output
from .selection import UnittestTest


@dataclass
class _Report:
    """The driver's report, by test: why the module did not load, the outcomes of fixtures,
    each test's own outcome, and its subTests' outcomes by title, in the order they came."""

    load: str | None = None
    fixtures: list[TestRun] = field(default_factory=list[TestRun])
    own: dict[str, TestRun] = field(default_factory=dict[str, TestRun])
    subtests: dict[str, list[tuple[str | None, TestRun]]] = field(
        default_factory=dict[str, list[tuple[str | None, TestRun]]]
    )


def unittest_outcomes(report: str, tests: Sequence[UnittestTest], output: str) -> list[TestRun]:
    """Each test's outcome; ``output`` is what the run wrote, which says why a test that has
    none did not run."""
    read = _read(report)
    return [_outcome(test, read, output) for test in tests]


def _read(report: str) -> _Report:
    read = _Report()
    for line in report.splitlines():
        if not line.strip():
            continue
        event = Fields(json.loads(line), "the unittest driver's report")
        if event.has("load"):
            read.load = event.string("message")
            continue
        run = _run(event.string("outcome"), event.optional_string("message"))
        if event.has("fixture"):
            read.fixtures.append(titled(event.string("fixture"), run))
        elif event.has("subtest"):
            title = _title(event.object("subtest"))
            read.subtests.setdefault(event.string("test"), []).append((title, run))
        else:
            read.own[event.string("test")] = run
    return read


def _run(outcome: str, message: str | None) -> TestRun:
    if outcome == "passed":
        return PASSED
    text = message or f"unittest reported it as {outcome}"
    if outcome == "skipped":
        return skipped(text)
    if outcome == "missing":
        return not_run(text)
    return failed(text)


def _title(subtest: Fields) -> str | None:
    return subtest_title(subtest.optional_string("message"), subtest.strings("params"))


def _outcome(test: UnittestTest, report: _Report, output: str) -> TestRun:
    if report.load is not None:
        return not_run(f"unittest could not import {test.module.name}: {report.load}")
    own = report.own.get(test.name())
    subtests = report.subtests.get(test.name(), [])
    if own is None and not subtests:
        if report.fixtures:
            return report.fixtures[0]
        return not_run(with_output(f"unittest did not run {test.module.name}.{test.name()}", output))
    if test.subtest is None:
        # unittest reports no outcome of its own for a test one of whose subTests did not pass.
        return all_of([titled(title, run) for title, run in subtests] + ([own] if own else []), "")
    return _case(test, own, subtests)


def _case(test: UnittestTest, own: TestRun | None, subtests: list[tuple[str | None, TestRun]]) -> TestRun:
    """A subTest case: as its subTest ended; as its test ended, when the test stopped
    before it."""
    name = f"{test.module.name}.{test.name()}"
    missing = f"{name} ran no subTest titled {test.subtest}"
    cases = [run for title, run in subtests if title == test.subtest]
    if not cases and own is not None and own["status"] != "passed":
        return own
    return all_of(cases, missing)
