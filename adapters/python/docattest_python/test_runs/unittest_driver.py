"""Where unittest runs the tests citations name, on the project's interpreter.

Started by its path as ``python unittest_driver.py <spec>`` (standard library only), from the
project's root. The spec, one JSON argument, names the report file, the directory the test
file is imported from, the module, and the ``[class, method]`` pairs to run. The module is
imported as ``unittest discover`` imports it, and the tests run in one suite, so that module
and class fixtures run as they would there.

The report is one JSON line per event, written as it happens, so that what ran before a
test that ends the process is kept:

- ``{"test": "Class.method", "outcome": ..., "message": ...}``: passed, failed, skipped, or
  missing (the module has no such ``TestCase`` method);
- the same with ``"subtest": {"message", "params"}`` for each subTest the test entered;
- ``{"fixture": ..., "outcome": ..., "message": ...}``: a module or class fixture failed or
  skipped, and its tests did not run;
- ``{"load": "failed", "message": ...}``: the module did not import.
"""

from __future__ import annotations

import importlib
import json
import os
import sys
import traceback
import unittest
from types import ModuleType, TracebackType
from typing import IO, TypedDict, cast

#: What unittest's result methods are given for an error.
ErrorInfo = tuple[type[BaseException], BaseException, TracebackType] | tuple[None, None, None]

EXPECTED_FAILURE = "it is marked expectedFailure, and failed as expected"
UNEXPECTED_SUCCESS = "it is marked expectedFailure, but passed"


class Spec(TypedDict):
    report: str
    top: str
    module: str
    tests: list[list[str]]


def main() -> None:
    spec = cast(Spec, json.loads(sys.argv[1]))
    # Started by its path, the driver's directory is first on sys.path; the test file's
    # import directory takes its place, as discover's start directory would.
    own = os.path.dirname(os.path.abspath(__file__))
    sys.path[:] = [p for p in sys.path if os.path.abspath(p or ".") != own]
    sys.path.insert(0, spec["top"])
    with open(spec["report"], "a", encoding="utf-8") as report:
        run(spec, report)


def run(spec: Spec, report: IO[str]) -> None:
    try:
        module = importlib.import_module(spec["module"])
    except BaseException as err:  # noqa: BLE001 - whatever stops the import is the reason
        write(report, {"load": "failed", "message": _exception_text(type(err), err)})
        return
    names: dict[unittest.TestCase, str] = {}
    suite = unittest.TestSuite()
    for class_name, method in spec["tests"]:
        name = f"{class_name}.{method}"
        test = _test(module, class_name, method)
        if test is None:
            message = f"{spec['module']}.{class_name} is not a unittest.TestCase with a method {method}"
            write(report, {"test": name, "outcome": "missing", "message": message})
        else:
            names[test] = name
            suite.addTest(test)
    suite.run(RecordingResult(report, names))


def _test(module: ModuleType, class_name: str, method: str) -> unittest.TestCase | None:
    """The test ``discover`` would make of a method of a class of the module, if it is one."""
    owner: object = getattr(module, class_name, None)
    if not isinstance(owner, type) or not issubclass(owner, unittest.TestCase):
        return None
    if not callable(getattr(owner, method, None)):
        return None
    return owner(method)


class RecordingResult(unittest.TestResult):
    """A result that writes each outcome to the report as unittest reports it."""

    def __init__(self, report: IO[str], names: dict[unittest.TestCase, str]) -> None:
        super().__init__()
        self.report = report
        self.names = names

    def addSuccess(self, test: unittest.TestCase) -> None:
        self._record(test, "passed", None)

    def addFailure(self, test: unittest.TestCase, err: ErrorInfo) -> None:
        self._record(test, "failed", _error_text(err))

    def addError(self, test: unittest.TestCase, err: ErrorInfo) -> None:
        self._record(test, "failed", _error_text(err))

    def addSkip(self, test: unittest.TestCase, reason: str) -> None:
        self._record(test, "skipped", reason or "unittest skipped it")

    def addExpectedFailure(self, test: unittest.TestCase, err: ErrorInfo) -> None:
        self._record(test, "skipped", EXPECTED_FAILURE)

    def addUnexpectedSuccess(self, test: unittest.TestCase) -> None:
        self._record(test, "failed", UNEXPECTED_SUCCESS)

    def addSubTest(self, test: unittest.TestCase, subtest: unittest.TestCase, err: ErrorInfo | None) -> None:
        if err is None:
            self._record(subtest, "passed", None)
        else:
            self._record(subtest, "failed", _error_text(err))

    def _record(self, test: unittest.TestCase, outcome: str, message: str | None) -> None:
        event: dict[str, object] = {"outcome": outcome}
        if message is not None:
            event["message"] = message
        parent: object = getattr(test, "test_case", None)
        if isinstance(parent, unittest.TestCase) and parent in self.names:
            event.update(test=self.names[parent], subtest=_subtest(test))
        elif test in self.names:
            event["test"] = self.names[test]
        else:
            # A module or class fixture: unittest reports it as a test of its own.
            event["fixture"] = str(test)
        write(self.report, event)


def _subtest(subtest: unittest.TestCase) -> dict[str, object]:
    """A subTest's message (None without one) and its parameters as unittest prints them.
    unittest keeps the message in a private attribute, absent being a private sentinel."""
    sentinel = vars(unittest.case).get("_subtest_msg_sentinel")
    message = vars(subtest).get("_message", sentinel)
    params = cast("dict[str, object]", vars(subtest).get("params", {}))
    return {
        "message": None if message is sentinel else str(message),
        "params": [f"{key}={value!r}" for key, value in params.items()],
    }


def _error_text(err: ErrorInfo) -> str:
    return _exception_text(err[0], err[1])


def _exception_text(kind: type[BaseException] | None, value: BaseException | None) -> str:
    """The exception's own lines (``AssertionError: 2 != 0 : the reason``), not its traceback."""
    return "".join(traceback.format_exception_only(kind, value)).strip()


def write(report: IO[str], event: dict[str, object]) -> None:
    report.write(json.dumps(event) + "\n")
    report.flush()


if __name__ == "__main__":
    main()
