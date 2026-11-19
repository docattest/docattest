"""``runTests`` end to end: a small project on disk whose cited tests the adapter runs with
unittest, or with pytest where the project's interpreter has it, and a project in memory,
where nothing runs.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from talk import HELLO, Talk, opened  # noqa: E402

#: The interpreter the adapter runs the tests with, as it chooses it here.
PYTHON = os.environ.get("DOCATTEST_PYTHON") or sys.executable
HAS_PYTEST = subprocess.run([PYTHON, "-c", "import pytest"], capture_output=True).returncode == 0

MATH = """\
import time
import unittest

CASES = [{"name": "rounds down", "cents": 1}, {"name": "rounds up", "cents": 5}]


class TestMath(unittest.TestCase):
    def test_adds(self):
        print("what a test prints goes nowhere near the core")
        self.assertEqual(1 + 1, 2)

    def test_subtracts(self):
        self.assertEqual(2 - 1, 0, "two minus one is not zero")

    @unittest.skip("not yet")
    def test_multiplies(self):
        pass

    def test_rounding(self):
        for case in CASES:
            with self.subTest(case["name"]):
                self.assertLess(case["cents"], 5, "%d cents is too many" % case["cents"])

    def test_waits(self):
        time.sleep(60)


class Helpers:
    def test_not_a_test_case(self):
        pass


def test_function():
    pass
"""

PACKAGED = """\
import unittest

from shoplib import THREE

from .helpers import TWO


class TestRelative(unittest.TestCase):
    def test_imports_beside_it_and_from_src(self):
        self.assertEqual(TWO + 1, THREE)
"""

PYTEST_TESTS = """\
import pytest


def test_ok():
    assert True


def test_bad():
    assert 2 - 1 == 0, "two minus one is not zero"


@pytest.mark.skip(reason="later")
def test_later():
    pass


@pytest.mark.parametrize("cents", [1, 5], ids=["rounds down", "rounds up"])
def test_price(cents):
    assert cents < 5, f"{cents} cents is too many"


class Helper:
    def test_hidden(self):
        pass
"""


def test(file, *path):
    return {"file": file, "describes": list(path[:-1]), "title": path[-1]}


def run_tests(tests, files=None, root=None, timeout_ms=60000, env=None):
    """``runTests`` in a project in memory (``files``) or on disk (``root``)."""
    open_request = opened(files or {})
    if root is not None:
        del open_request["params"]["files"]
        open_request["params"].update(root=root, surfaces=[])
    request = {"id": 3, "method": "runTests", "params": {"tests": tests, "timeoutMs": timeout_ms}}
    talk = Talk(HELLO, open_request, request, env=env)
    return talk.result(3)


class CitedTestsOnDisk(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix="docattest-cited-")
        self.addCleanup(shutil.rmtree, self.root)
        self.write("tests/test_math.py", MATH)
        self.write("pkg_tests/__init__.py", "")
        self.write("pkg_tests/helpers.py", "TWO = 2\n")
        self.write("pkg_tests/test_relative.py", PACKAGED)
        self.write("src/shoplib/__init__.py", "THREE = 3\n")

    def write(self, rel, text):
        path = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)

    def test_each_cited_test_runs_and_ends_as_its_runner_reports(self):
        math = "tests/test_math.py"
        results = run_tests(
            [
                test(math, "TestMath", "test_adds"),
                test(math, "TestMath", "test_subtracts"),
                test(math, "TestMath", "test_multiplies"),
                test(math, "Helpers", "test_not_a_test_case"),
                test("pkg_tests/test_relative.py", "TestRelative", "test_imports_beside_it_and_from_src"),
            ],
            root=self.root,
        )
        self.assertEqual([r["status"] for r in results], ["passed", "failed", "skipped", "notRun", "passed"])
        self.assertIn("two minus one is not zero", results[1]["message"])
        self.assertEqual(results[2]["message"], "not yet")

    def test_a_subtest_case_runs_with_its_test_and_ends_as_its_subtest(self):
        math = "tests/test_math.py"
        results = run_tests(
            [
                test(math, "TestMath", "test_rounding", "rounds down"),
                test(math, "TestMath", "test_rounding", "rounds up"),
                test(math, "TestMath", "test_rounding"),
            ],
            root=self.root,
        )
        self.assertEqual([r["status"] for r in results], ["passed", "failed", "failed"])
        self.assertIn("5 cents is too many", results[1]["message"])
        self.assertIn("5 cents is too many", results[2]["message"])

    @unittest.skipIf(HAS_PYTEST, "pytest runs a test function where it is installed")
    def test_without_pytest_a_test_function_does_not_run(self):
        [result] = run_tests([test("tests/test_math.py", "test_function")], root=self.root)
        self.assertEqual(result["status"], "notRun")
        self.assertIn("pytest, which runs the others, is not installed", result["message"])

    def test_a_run_that_overruns_its_time_fails_its_tests(self):
        results = run_tests(
            [
                test("tests/test_math.py", "TestMath", "test_waits"),
                test("tests/test_math.py", "TestMath", "test_adds"),
            ],
            root=self.root,
            timeout_ms=2000,
        )
        overran = {"status": "failed", "message": "the test run did not finish within 2000 ms"}
        self.assertEqual(results, [overran, overran])

    @unittest.skipUnless(HAS_PYTEST, f"pytest is not installed for {PYTHON}")
    def test_pytest_runs_each_cited_test_by_its_node_id(self):
        self.write("tests/test_py.py", PYTEST_TESTS)
        file = "tests/test_py.py"
        results = run_tests(
            [
                test(file, "test_ok"),
                test(file, "test_bad"),
                test(file, "test_later"),
                test(file, "Helper", "test_hidden"),
                test(file, "test_price", "rounds down"),
                test(file, "test_price", "rounds up"),
                test(file, "test_price"),
            ],
            root=self.root,
            env={"DOCATTEST_PYTHON": PYTHON},
        )
        statuses = [r["status"] for r in results]
        self.assertEqual(statuses, ["passed", "failed", "skipped", "notRun", "passed", "failed", "failed"])
        self.assertIn("two minus one is not zero", results[1]["message"])
        self.assertIn("5 cents is too many", results[5]["message"])
        self.assertFalse(os.path.exists(os.path.join(self.root, ".pytest_cache")))


class CitedTestsInMemory(unittest.TestCase):
    def test_nothing_runs_in_a_project_in_memory(self):
        results = run_tests(
            [test("tests/test_a.py", "TestA", "test_b"), test("tests/test_a.py", "test_c")],
            files={"lib.py": "", "tests/test_a.py": "def test_c():\n    pass\n"},
        )
        in_memory = {"status": "notRun", "message": "the project is in memory, where no test can run"}
        self.assertEqual(results, [in_memory, in_memory])


if __name__ == "__main__":
    unittest.main()
