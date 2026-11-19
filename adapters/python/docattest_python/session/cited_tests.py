"""``runTests``: the tests citations name, run in the project on disk with the interpreter
and import path its examples run with, as many runs at once as ``open.workers``."""

from __future__ import annotations

import os
import tempfile
from concurrent.futures import Future, ThreadPoolExecutor
from dataclasses import dataclass

from ..test_runs.outcome import RequestedTest, TestRun, failed, not_run
from ..test_runs.process import Overran, RunnerProcesses
from ..test_runs.pytest_report import pytest_outcomes, runs_again_alone
from ..test_runs.selection import PytestRun, Run, Runner, Selection, Unselectable, runs, select
from ..test_runs.unittest_report import unittest_outcomes
from ..workers.interpreter import Interpreter
from ..workers.worker_process import ms
from .running import gather, resolved
from .symbols import Symbols

IN_MEMORY = "the project is in memory, where no test can run"


@dataclass(frozen=True)
class OnDisk:
    """A project whose tests can run: its root, and the import path its examples run with."""

    root: str
    search: list[str]


class CitedTests:
    """What answers ``runTests``. Its threads start with the first run."""

    def __init__(
        self, project: OnDisk | None, symbols: Symbols, interpreter: Interpreter, workers: int
    ) -> None:
        #: None for a project in memory.
        self.project = project
        self.symbols = symbols
        self.interpreter = interpreter
        self.workers = workers
        self.processes = RunnerProcesses()
        self._executor: ThreadPoolExecutor | None = None

    def run(self, tests: list[RequestedTest], timeout_ms: float) -> Future[list[TestRun]]:
        project = self.project
        if project is None:
            return resolved([not_run(IN_MEMORY) for _ in tests])
        runner: Runner = "pytest" if self.interpreter.finds("pytest") else "unittest"
        selections = [self._select(test, runner, project.root) for test in tests]
        if self._executor is None:
            self._executor = ThreadPoolExecutor(self.workers, thread_name_prefix="docattest-tests")
        futures = [
            self._executor.submit(self._outcomes, run, project, timeout_ms) for run in runs(selections)
        ]

        def answer(results: list[dict[Selection, TestRun]]) -> list[TestRun]:
            outcomes = {selection: run for result in results for selection, run in result.items()}
            return [not_run(s.message) if isinstance(s, Unselectable) else outcomes[s] for s in selections]

        return gather(futures, answer)

    def close(self) -> None:
        self.processes.close()
        if self._executor is not None:
            self._executor.shutdown(wait=False, cancel_futures=True)

    def _select(self, test: RequestedTest, runner: Runner, root: str) -> Selection:
        declared = self.symbols.declared(test.file)
        if isinstance(declared, str):
            return Unselectable(declared)
        return select(test, declared, runner, lambda d: os.path.isfile(os.path.join(root, d, "__init__.py")))

    def _outcomes(self, run: Run, project: OnDisk, timeout_ms: float) -> dict[Selection, TestRun]:
        """Each test of one run, by its selection."""
        with tempfile.TemporaryDirectory(prefix="docattest-tests-") as temp:
            report_file = os.path.join(temp, "report")
            command = run.command(self.interpreter.python, project.root, report_file)
            ended = self.processes.run(command, project.root, _environment(project), timeout_ms / 1000)
            if isinstance(ended, Overran):
                overran = failed(f"the test run did not finish within {ms(timeout_ms)} ms")
                return dict.fromkeys(run.tests, overran)
            report = _read(report_file)
        if not isinstance(run, PytestRun):
            return dict(zip(run.tests, unittest_outcomes(report or "", run.tests, ended.output)))
        again = runs_again_alone(run, ended.exit_code)
        if again:
            return {s: r for alone in again for s, r in self._outcomes(alone, project, timeout_ms).items()}
        return dict(zip(run.tests, pytest_outcomes(report, run.tests, ended.output)))


def _environment(project: OnDisk) -> dict[str, str]:
    """The adapter's environment, with the project's import path first, as examples have it."""
    inherited = os.environ.get("PYTHONPATH")
    path = [*project.search, inherited] if inherited else project.search
    return dict(os.environ, PYTHONPATH=os.pathsep.join(path))


def _read(file: str) -> str | None:
    """A report the runner wrote, or None when it wrote none."""
    try:
        with open(file, encoding="utf-8") as handle:
            return handle.read()
    except FileNotFoundError:
        return None
