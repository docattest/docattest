"""How each test of a pytest run ended, read from the JUnit XML report pytest writes with
``--junitxml``: one ``testcase`` per test or parametrized case that ran."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from xml.etree import ElementTree

from .outcome import PASSED, TestRun, all_of, failed, skipped, titled, with_output
from .selection import PytestRun, PytestTest

#: pytest's exit code when it refuses the whole run before any test runs: one node id that
#: selects nothing is enough.
USAGE_ERROR = 4


@dataclass(frozen=True)
class _Case:
    """A ``testcase`` of the report: where pytest says it is, and how it ended."""

    classname: str
    name: str
    run: TestRun


def pytest_outcomes(report: str | None, tests: Sequence[PytestTest], output: str) -> list[TestRun]:
    """Each test's outcome; ``report`` is None when pytest wrote none, and ``output``, what
    it printed, says why a test that has no ``testcase`` did not run."""
    cases = _cases(report) if report is not None else []
    return [_outcome(test, cases, output) for test in tests]


def runs_again_alone(run: PytestRun, exit_code: int) -> list[PytestRun]:
    """The runs to make instead of one pytest refused: each of its tests alone, so that one
    node id that selects nothing does not keep the others from running."""
    if exit_code != USAGE_ERROR or len(run.tests) < 2:
        return []
    return [PytestRun((test,)) for test in run.tests]


def _cases(report: str) -> list[_Case]:
    root = ElementTree.fromstring(report)
    return [
        _Case(case.get("classname", ""), case.get("name", ""), _run(case)) for case in root.iter("testcase")
    ]


def _run(case: ElementTree.Element) -> TestRun:
    """A failure or error (in the test, its setup or its teardown) fails it; a skip, or an
    expected failure, which pytest reports as a skip, skips it."""
    for child in case:
        if child.tag in ("failure", "error"):
            return failed(_message(child, "pytest reported it as failed"))
    for child in case:
        if child.tag == "skipped":
            return skipped(_message(child, "pytest skipped it"))
    return PASSED


def _message(child: ElementTree.Element, otherwise: str) -> str:
    """The outcome's own message (an assertion's), else its text."""
    return (child.get("message") or child.text or "").strip() or otherwise


def _outcome(test: PytestTest, cases: list[_Case], output: str) -> TestRun:
    """A test's outcome: its ``testcase``, or for a test cited whole, those of all its cases."""
    classname, name = address(test.node_id)
    whole = "[" not in name
    found = [
        titled(case.name, case.run) if case.name != name else case.run
        for case in cases
        if case.classname == classname and (case.name == name or (whole and case.name.startswith(name + "[")))
    ]
    return all_of(found, with_output(f"pytest selected no test by {test.node_id}", output))


def address(node_id: str) -> tuple[str, str]:
    """Where pytest's JUnit report puts a node id: its ``classname`` (the file's path dotted,
    without ``.py``, then its classes) and its ``name`` (the test's, with any parameters)."""
    path, bracket, params = node_id.partition("[")
    names = path.split("::")
    file = names[0].replace("/", ".")
    names[0] = file[: -len(".py")] if file.endswith(".py") else file
    names[-1] += bracket + params
    return ".".join(names[:-1]), names[-1]
