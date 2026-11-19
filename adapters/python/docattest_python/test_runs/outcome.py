"""A test ``runTests`` is asked to run, and how it ended: the protocol's ``TestRun``."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, TypedDict

#: How many lines of a runner's own output a message quotes when it says why nothing ran.
OUTPUT_LINES = 3


@dataclass(frozen=True)
class RequestedTest:
    """A test as its file declares it: the titles that enclose it, outermost first, and its own."""

    file: str
    describes: tuple[str, ...]
    title: str

    def cited(self) -> str:
        """The test as a citation writes it."""
        return " > ".join([self.file, *(f'"{t}"' for t in [*self.describes, self.title])])


class Passed(TypedDict):
    status: Literal["passed"]


class NotPassed(TypedDict):
    status: Literal["failed", "skipped", "notRun"]
    message: str


TestRun = Passed | NotPassed

PASSED: Passed = {"status": "passed"}


def failed(message: str) -> NotPassed:
    return {"status": "failed", "message": message}


def skipped(message: str) -> NotPassed:
    return {"status": "skipped", "message": message}


def not_run(message: str) -> NotPassed:
    return {"status": "notRun", "message": message}


def all_of(cases: list[TestRun], none_ran: str) -> TestRun:
    """A test that holds cases: passed when every case passed, failed when one failed, else
    as the first that did not pass; ``none_ran`` when it has none."""
    if not cases:
        return not_run(none_ran)
    first_failure = next((c for c in cases if c["status"] == "failed"), None)
    if first_failure is not None:
        return first_failure
    return next((c for c in cases if c["status"] != "passed"), PASSED)


def titled(title: str | None, run: TestRun) -> TestRun:
    """The outcome of a part of a test (a case, a subTest, a fixture), its message led by
    the part's name."""
    if run["status"] == "passed" or title is None:
        return run
    return {"status": run["status"], "message": f"{title}: {run['message']}"}


def output_tail(output: str) -> str:
    """The last lines a runner wrote, which say why it stopped; empty when it wrote none."""
    lines = [line.rstrip() for line in output.splitlines() if line.strip()]
    return "\n".join(lines[-OUTPUT_LINES:])


def with_output(message: str, output: str) -> str:
    """A message, followed by what the runner said, when it said anything."""
    tail = output_tail(output)
    return f"{message}:\n{tail}" if tail else message
