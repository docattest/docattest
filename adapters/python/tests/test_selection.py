"""Which runner runs a cited test, how it selects it, and the command lines of the runs, all
decided without running anything.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import ast
import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
sys.path.insert(0, PACKAGE_ROOT)

from docattest_python.analysis.declared_tests.listing import declared  # noqa: E402
from docattest_python.test_runs.outcome import RequestedTest  # noqa: E402
from docattest_python.test_runs.selection import (  # noqa: E402
    DRIVER,
    ModuleImport,
    PytestRun,
    PytestTest,
    UnittestRun,
    UnittestTest,
    Unselectable,
    runs,
    select,
    unittest_module,
)

TESTS = """\
import unittest

import pytest


def test_plain():
    pass


@pytest.mark.parametrize("name", ["a::b [c]", "-x"])
def test_odd_ids(name):
    pass


class TestA(unittest.TestCase):
    def test_b(self):
        for region in ["NL"]:
            with self.subTest(region=region):
                pass

    class TestInner(unittest.TestCase):
        def test_deep(self):
            pass
"""
DECLARED = declared(ast.parse(TESTS))
FILE = "tests/test_odd.py"
MODULE = ModuleImport("tests", "test_odd")


def no_packages(_directory):
    return False


def chosen(describes, title, runner, is_package=no_packages):
    return select(RequestedTest(FILE, tuple(describes), title), DECLARED, runner, is_package)


class Selection(unittest.TestCase):
    def test_pytest_selects_a_test_by_its_node_id_and_a_case_with_its_id_in_brackets(self):
        self.assertEqual(chosen([], "test_plain", "pytest"), PytestTest(FILE, f"{FILE}::test_plain"))
        self.assertEqual(chosen(["TestA"], "test_b", "pytest"), PytestTest(FILE, f"{FILE}::TestA::test_b"))
        self.assertEqual(
            chosen(["test_odd_ids"], "a::b [c]", "pytest"),
            PytestTest(FILE, f"{FILE}::test_odd_ids[a::b [c]]"),
        )
        self.assertEqual(
            chosen(["TestA", "TestInner"], "test_deep", "pytest"),
            PytestTest(FILE, f"{FILE}::TestA::TestInner::test_deep"),
        )

    def test_a_subtest_case_runs_with_its_test_under_unittest_whichever_runner_runs_the_rest(self):
        case = UnittestTest(MODULE, "TestA", "test_b", "region='NL'")
        self.assertEqual(chosen(["TestA", "test_b"], "region='NL'", "pytest"), case)
        self.assertEqual(chosen(["TestA", "test_b"], "region='NL'", "unittest"), case)

    def test_unittest_runs_the_methods_of_classes_at_the_top_of_a_module(self):
        self.assertEqual(
            chosen(["TestA"], "test_b", "unittest"), UnittestTest(MODULE, "TestA", "test_b", None)
        )
        for describes, title in [
            ([], "test_plain"),
            (["test_odd_ids"], "-x"),
            (["TestA", "TestInner"], "test_deep"),
        ]:
            with self.subTest(title=title):
                selection = chosen(describes, title, "unittest")
                assert isinstance(selection, Unselectable), selection
                self.assertIn("pytest, which runs the others, is not installed", selection.message)

    def test_a_subtest_case_of_a_nested_class_runs_under_neither_runner(self):
        nested = "class TestA:\n    class TestB(unittest.TestCase):\n        def test_c(self):\n            with self.subTest('x'):\n                pass\n"
        request = RequestedTest(FILE, ("TestA", "TestB", "test_c"), "x")
        selection = select(request, declared(ast.parse(nested)), "pytest", no_packages)
        self.assertEqual(
            selection,
            Unselectable(
                f"unittest cannot run {request.cited()}: it runs the test methods of a class at the top of a module"
            ),
        )

    def test_a_path_the_file_does_not_declare_as_a_test_is_not_selected(self):
        self.assertEqual(
            chosen([], "TestA", "pytest"), Unselectable(f'{FILE} declares no test {FILE} > "TestA"')
        )
        self.assertIsInstance(chosen(["TestA"], "test_gone", "unittest"), Unselectable)


class ModuleImports(unittest.TestCase):
    def test_a_file_is_imported_from_the_first_directory_above_it_that_is_not_a_package(self):
        packages = {"tests", "tests/unit"}
        self.assertEqual(
            unittest_module("tests/unit/test_a.py", packages.__contains__),
            ModuleImport(".", "tests.unit.test_a"),
        )
        self.assertEqual(
            unittest_module("src/tests/test_a.py", {"src/tests"}.__contains__),
            ModuleImport("src", "tests.test_a"),
        )
        self.assertEqual(unittest_module("tests/test_a.py", no_packages), ModuleImport("tests", "test_a"))
        self.assertEqual(unittest_module("test_a.py", no_packages), ModuleImport(".", "test_a"))

    def test_a_path_that_is_not_a_module_name_cannot_be_imported(self):
        self.assertIsNone(unittest_module("tests/test-a.py", no_packages))
        self.assertIsNone(unittest_module("my-tests/test_a.py", {"my-tests"}.__contains__))
        self.assertIsInstance(
            select(RequestedTest("tests/test-a.py", ("TestA",), "test_b"), DECLARED, "unittest", no_packages),
            Unselectable,
        )


class Commands(unittest.TestCase):
    def test_pytest_gets_each_node_id_once_after_the_options_end(self):
        odd = PytestTest(FILE, f"{FILE}::test_odd_ids[-x]")
        command = PytestRun((odd, odd)).command("/py", "/root", "/tmp/r.xml")
        self.assertEqual(
            command,
            [
                "/py",
                "-m",
                "pytest",
                "-p",
                "no:cacheprovider",
                "-q",
                "--rootdir=/root",
                "--junitxml=/tmp/r.xml",
                "--",
                f"{FILE}::test_odd_ids[-x]",
            ],
        )

    def test_the_driver_gets_what_to_run_as_one_json_argument_that_no_title_breaks(self):
        quoted = ModuleImport("tests", "test_q")
        run = UnittestRun(
            quoted,
            (
                UnittestTest(quoted, "TestA", "test_b", 'say "hi" \\ there'),
                UnittestTest(quoted, "TestA", "test_b", None),
                UnittestTest(quoted, "TestC", "test_d", None),
            ),
        )
        command = run.command("/py", "/root", "/tmp/r.jsonl")
        self.assertEqual(command[:2], ["/py", DRIVER])
        self.assertEqual(
            json.loads(command[2]),
            {
                "report": "/tmp/r.jsonl",
                "top": os.path.normpath("/root/tests"),
                "module": "test_q",
                "tests": [["TestA", "test_b"], ["TestC", "test_d"]],
            },
        )

    def test_the_tests_of_one_file_share_a_run_for_each_runner(self):
        a = PytestTest("tests/test_a.py", "tests/test_a.py::test_x")
        b = PytestTest("tests/test_b.py", "tests/test_b.py::test_y")
        a2 = PytestTest("tests/test_a.py", "tests/test_a.py::test_z")
        sub = UnittestTest(ModuleImport("tests", "test_a"), "TestA", "test_b", "region='NL'")
        self.assertEqual(
            runs([a, Unselectable("no"), b, sub, a2]),
            [PytestRun((a, a2)), PytestRun((b,)), UnittestRun(sub.module, (sub,))],
        )


if __name__ == "__main__":
    unittest.main()
