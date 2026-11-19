"""Which runner runs each cited test, how it is selected, and the runs that select them: one
runner invocation per test file and runner.

pytest selects a test by its node id (``tests/test_a.py::TestA::test_b``, a parametrized
case with its id in brackets). unittest runs in this adapter's driver, which imports the
file as ``unittest discover`` would and runs the named methods. A subTest case cannot be
selected alone, by either runner: it runs with its test, under unittest, which reports
each subTest's outcome.
"""

from __future__ import annotations

import json
import os
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import Literal

from ..analysis.declared_tests.listing import Declared, DeclaredKind
from .outcome import RequestedTest

Runner = Literal["pytest", "unittest"]

#: The script unittest runs in, started by its path on the project's interpreter.
DRIVER = os.path.join(os.path.dirname(os.path.abspath(__file__)), "unittest_driver.py")


@dataclass(frozen=True)
class PytestTest:
    file: str
    node_id: str


@dataclass(frozen=True)
class ModuleImport:
    """How unittest imports a test file: the directory it puts on ``sys.path`` (relative to
    the root, ``.`` for the root) and the module's dotted name from there."""

    top: str
    name: str


@dataclass(frozen=True)
class UnittestTest:
    module: ModuleImport
    class_name: str
    method: str
    #: The title of the subTest case cited, or None for the whole test.
    subtest: str | None

    def name(self) -> str:
        """The test as the driver's report names it: ``Class.method``."""
        return f"{self.class_name}.{self.method}"


@dataclass(frozen=True)
class Unselectable:
    """A test no runner can select, and why."""

    message: str


Selection = PytestTest | UnittestTest | Unselectable


def select(
    test: RequestedTest, declared: Sequence[Declared], runner: Runner, is_package: Callable[[str], bool]
) -> Selection:
    """How ``runner`` selects a test its file ``declared``; ``is_package`` says whether a
    directory, relative to the root, holds an ``__init__.py``."""
    found = next(
        (d for d in declared if d.title == test.title and tuple(d.describes) == test.describes), None
    )
    if found is None or found.kind == "suite":
        return Unselectable(f"{test.file} declares no test {test.cited()}")
    if found.kind == "subtest" or runner == "unittest":
        return _unittest_test(test, found.kind, runner, is_package)
    path = "".join(f"::{title}" for title in test.describes)
    if found.kind == "parametrized":
        return PytestTest(test.file, f"{test.file}{path}[{test.title}]")
    return PytestTest(test.file, f"{test.file}{path}::{test.title}")


def _unittest_test(
    test: RequestedTest, kind: DeclaredKind, runner: Runner, is_package: Callable[[str], bool]
) -> Selection:
    """A method of a class at the top of its module, or a subTest case of one: what unittest
    runs. A parametrized case is pytest's."""
    subtest = kind == "subtest"
    if kind == "parametrized" or len(test.describes) != (2 if subtest else 1):
        why = "it runs the test methods of a class at the top of a module"
        if runner == "unittest":
            why += ", and pytest, which runs the others, is not installed"
        return Unselectable(f"unittest cannot run {test.cited()}: {why}")
    module = unittest_module(test.file, is_package)
    if module is None:
        return Unselectable(f"unittest cannot import {test.file}: its path is not a module's name")
    if subtest:
        return UnittestTest(module, test.describes[0], test.describes[1], test.title)
    return UnittestTest(module, test.describes[0], test.title, None)


def unittest_module(file: str, is_package: Callable[[str], bool]) -> ModuleImport | None:
    """How ``unittest discover`` imports a test file: from the first directory above it that
    is not a package, by its dotted name from there; None when that name is not one."""
    *directories, base = file.split("/")
    stem, extension = os.path.splitext(base)
    names = [stem]
    while directories and is_package("/".join(directories)):
        names.insert(0, directories.pop())
    if extension != ".py" or not all(n.isidentifier() for n in names):
        return None
    return ModuleImport("/".join(directories) or ".", ".".join(names))


@dataclass(frozen=True)
class PytestRun:
    """The tests of one file that pytest runs, in one invocation."""

    tests: tuple[PytestTest, ...]

    def command(self, python: str, root: str, report: str) -> list[str]:
        """pytest, with the report written as JUnit XML to ``report``; node ids follow ``--``
        so that none is read as an option."""
        ids = list(dict.fromkeys(t.node_id for t in self.tests))
        return [
            python,
            "-m",
            "pytest",
            "-p",
            "no:cacheprovider",
            "-q",
            f"--rootdir={root}",
            f"--junitxml={report}",
            "--",
            *ids,
        ]


@dataclass(frozen=True)
class UnittestRun:
    """The tests of one file that unittest runs, in one invocation of the driver."""

    module: ModuleImport
    tests: tuple[UnittestTest, ...]

    def command(self, python: str, root: str, report: str) -> list[str]:
        """The driver, told what to import and run as one JSON argument, which no title can
        break."""
        methods = list(dict.fromkeys((t.class_name, t.method) for t in self.tests))
        spec = {
            "report": report,
            "top": os.path.normpath(os.path.join(root, self.module.top)),
            "module": self.module.name,
            "tests": [list(m) for m in methods],
        }
        return [python, DRIVER, json.dumps(spec)]


Run = PytestRun | UnittestRun


def runs(selections: Sequence[Selection]) -> list[Run]:
    """One run for each test file and runner."""
    by_file: dict[str, list[PytestTest]] = {}
    by_module: dict[ModuleImport, list[UnittestTest]] = {}
    for selection in selections:
        if isinstance(selection, PytestTest):
            by_file.setdefault(selection.file, []).append(selection)
        elif isinstance(selection, UnittestTest):
            by_module.setdefault(selection.module, []).append(selection)
    pytest_runs: list[Run] = [PytestRun(tuple(tests)) for tests in by_file.values()]
    return pytest_runs + [UnittestRun(module, tuple(tests)) for module, tests in by_module.items()]
