"""How each test of a pytest run ended, read from the JUnit XML report pytest wrote: the
report below was recorded from pytest 9.2 (``--junitxml``, ``--rootdir`` at the project).

    python3 -m unittest discover -s adapters/python/tests -v
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
sys.path.insert(0, PACKAGE_ROOT)

from docattest_python.test_runs.pytest_report import (  # noqa: E402
    USAGE_ERROR,
    address,
    pytest_outcomes,
    runs_again_alone,
)
from docattest_python.test_runs.selection import PytestRun, PytestTest  # noqa: E402

FILE = "tests/test_py.py"

REPORT = """\
<?xml version="1.0" encoding="utf-8"?><testsuites name="pytest tests"><testsuite name="pytest" \
errors="1" failures="2" skipped="2" tests="7" time="0.019"><testcase classname="tests.test_py" \
name="test_ok" time="0.000" /><testcase classname="tests.test_py" name="test_bad" time="0.000">\
<failure message="AssertionError: two minus one is not zero&#10;assert (2 - 1) == 0">def test_bad():
&gt;       assert 2 - 1 == 0, "two minus one is not zero"
E       AssertionError: two minus one is not zero
E       assert (2 - 1) == 0

tests/test_py.py:9: AssertionError</failure></testcase><testcase classname="tests.test_py" \
name="test_later" time="0.000"><skipped type="pytest.skip" message="later">/project/tests/test_py.py:12: \
later</skipped></testcase><testcase classname="tests.test_py" name="test_price[rounds down]" \
time="0.000" /><testcase classname="tests.test_py" name="test_price[rounds up]" time="0.000">\
<failure message="AssertionError: 5 cents is too many&#10;assert 5 &lt; 5">cents = 5

    @pytest.mark.parametrize("cents", [1, 5], ids=["rounds down", "rounds up"])
    def test_price(cents):
&gt;       assert cents &lt; 5, f"{cents} cents is too many"
E       AssertionError: 5 cents is too many
E       assert 5 &lt; 5

tests/test_py.py:19: AssertionError</failure></testcase><testcase classname="tests.test_py" \
name="test_expected" time="0.000"><skipped type="pytest.xfail" message="not done" /></testcase>\
<testcase classname="tests.test_py" name="test_needs" time="0.000"><error message="failed on setup \
with &quot;RuntimeError: no database&quot;">@pytest.fixture
    def broken():
&gt;       raise RuntimeError("no database")
E       RuntimeError: no database

tests/test_py.py:34: RuntimeError</error></testcase></testsuite></testsuites>
"""


def node(path):
    return PytestTest(FILE, f"{FILE}::{path}")


def outcome(path, report=REPORT, output=""):
    return pytest_outcomes(report, [node(path)], output)[0]


class PytestReport(unittest.TestCase):
    def test_a_passed_failed_and_skipped_test_each_ends_as_its_testcase_says(self):
        self.assertEqual(outcome("test_ok"), {"status": "passed"})
        self.assertEqual(
            outcome("test_bad"),
            {"status": "failed", "message": "AssertionError: two minus one is not zero\nassert (2 - 1) == 0"},
        )
        self.assertEqual(outcome("test_later"), {"status": "skipped", "message": "later"})

    def test_an_expected_failure_is_skipped_and_an_error_in_setup_fails(self):
        self.assertEqual(outcome("test_expected"), {"status": "skipped", "message": "not done"})
        self.assertEqual(
            outcome("test_needs"),
            {"status": "failed", "message": 'failed on setup with "RuntimeError: no database"'},
        )

    def test_a_case_ends_as_its_own_testcase_and_a_test_cited_whole_as_all_its_cases(self):
        self.assertEqual(outcome("test_price[rounds down]"), {"status": "passed"})
        self.assertEqual(
            outcome("test_price[rounds up]"),
            {"status": "failed", "message": "AssertionError: 5 cents is too many\nassert 5 < 5"},
        )
        self.assertEqual(
            outcome("test_price"),
            {
                "status": "failed",
                "message": "test_price[rounds up]: AssertionError: 5 cents is too many\nassert 5 < 5",
            },
        )

    def test_a_test_with_no_testcase_did_not_run_and_the_message_quotes_pytest(self):
        printed = (
            "ERROR: not found: /project/tests/test_py.py::Helper::test_hidden\n(no match)\n\nno tests ran\n"
        )
        self.assertEqual(
            outcome("Helper::test_hidden", output=printed),
            {
                "status": "notRun",
                "message": f"pytest selected no test by {FILE}::Helper::test_hidden:\n"
                "ERROR: not found: /project/tests/test_py.py::Helper::test_hidden\n(no match)\nno tests ran",
            },
        )
        self.assertEqual(
            pytest_outcomes(None, [node("test_ok")], "")[0],
            {"status": "notRun", "message": f"pytest selected no test by {FILE}::test_ok"},
        )

    def test_a_node_id_is_found_in_the_report_by_its_classname_and_name(self):
        self.assertEqual(
            address("tests/unit/test_a.py::TestA::test_b"), ("tests.unit.test_a.TestA", "test_b")
        )
        self.assertEqual(address("test_a.py::test_b[x.py::y]"), ("test_a", "test_b[x.py::y]"))

    def test_a_run_pytest_refused_runs_again_one_test_at_a_time(self):
        run = PytestRun((node("test_ok"), node("test_gone")))
        self.assertEqual(
            runs_again_alone(run, USAGE_ERROR),
            [PytestRun((node("test_ok"),)), PytestRun((node("test_gone"),))],
        )
        self.assertEqual(runs_again_alone(run, 1), [])
        self.assertEqual(runs_again_alone(PytestRun((node("test_gone"),)), USAGE_ERROR), [])


if __name__ == "__main__":
    unittest.main()
