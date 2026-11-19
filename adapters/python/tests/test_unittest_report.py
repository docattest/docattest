"""How each test of a unittest run ended, read from the report the adapter's driver wrote:
the lines below were recorded from the driver.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
sys.path.insert(0, PACKAGE_ROOT)

from docattest_python.test_runs.selection import ModuleImport, UnittestTest  # noqa: E402
from docattest_python.test_runs.unittest_report import unittest_outcomes  # noqa: E402

MODULE = ModuleImport("tests", "test_math")

REPORT = """\
{"test": "TestMath.test_gone", "outcome": "missing", "message": "test_math.TestMath is not a unittest.TestCase with a method test_gone"}
{"outcome": "passed", "test": "TestMath.test_adds"}
{"outcome": "failed", "message": "AssertionError: 1 != 0 : two minus one is not zero", "test": "TestMath.test_subtracts"}
{"outcome": "skipped", "message": "not yet", "test": "TestMath.test_multiplies"}
{"outcome": "passed", "test": "TestMath.test_rounding", "subtest": {"message": "rounds down", "params": []}}
{"outcome": "failed", "message": "AssertionError: 5 not less than 5 : 5 cents is too many", "test": "TestMath.test_rounding", "subtest": {"message": "rounds up", "params": []}}
{"outcome": "passed", "test": "TestMath.test_regions", "subtest": {"message": null, "params": ["region='NL'", "rate=21"]}}
{"outcome": "failed", "message": "ZeroDivisionError: division by zero", "test": "TestMath.test_stops"}
"""
FIXTURE = '{"outcome": "failed", "message": "RuntimeError: no database", "fixture": "setUpClass (test_math.TestMath)"}\n'
UNIMPORTABLE = '{"load": "failed", "message": "ModuleNotFoundError: No module named \'nowhere\'"}\n'


def test(method, subtest=None):
    return UnittestTest(MODULE, "TestMath", method, subtest)


def outcome(method, subtest=None, report=REPORT, output=""):
    return unittest_outcomes(report, [test(method, subtest)], output)[0]


class UnittestReport(unittest.TestCase):
    def test_a_passed_failed_and_skipped_test_each_ends_as_unittest_reported_it(self):
        self.assertEqual(outcome("test_adds"), {"status": "passed"})
        self.assertEqual(
            outcome("test_subtracts"),
            {"status": "failed", "message": "AssertionError: 1 != 0 : two minus one is not zero"},
        )
        self.assertEqual(outcome("test_multiplies"), {"status": "skipped", "message": "not yet"})

    def test_a_method_the_class_does_not_have_did_not_run(self):
        self.assertEqual(
            outcome("test_gone"),
            {
                "status": "notRun",
                "message": "test_math.TestMath is not a unittest.TestCase with a method test_gone",
            },
        )

    def test_a_subtest_case_ends_as_its_subtest_and_its_test_as_all_its_subtests(self):
        self.assertEqual(outcome("test_rounding", "rounds down"), {"status": "passed"})
        self.assertEqual(
            outcome("test_rounding", "rounds up"),
            {"status": "failed", "message": "AssertionError: 5 not less than 5 : 5 cents is too many"},
        )
        self.assertEqual(
            outcome("test_rounding"),
            {
                "status": "failed",
                "message": "rounds up: AssertionError: 5 not less than 5 : 5 cents is too many",
            },
        )
        self.assertEqual(outcome("test_regions", "region='NL', rate=21"), {"status": "passed"})

    def test_a_subtest_case_its_test_never_reached_ends_as_its_test_or_did_not_run(self):
        self.assertEqual(
            outcome("test_stops", "rounds up"),
            {"status": "failed", "message": "ZeroDivisionError: division by zero"},
        )
        self.assertEqual(
            outcome("test_rounding", "rounds sideways"),
            {
                "status": "notRun",
                "message": "test_math.TestMath.test_rounding ran no subTest titled rounds sideways",
            },
        )

    def test_a_failed_fixture_fails_its_tests_and_a_module_that_does_not_import_runs_none(self):
        self.assertEqual(
            outcome("test_adds", report=FIXTURE),
            {"status": "failed", "message": "setUpClass (test_math.TestMath): RuntimeError: no database"},
        )
        self.assertEqual(
            outcome("test_adds", report=UNIMPORTABLE),
            {
                "status": "notRun",
                "message": "unittest could not import test_math: ModuleNotFoundError: No module named 'nowhere'",
            },
        )

    def test_a_test_with_no_outcome_did_not_run_and_the_message_quotes_the_run(self):
        self.assertEqual(
            outcome("test_adds", report="", output="Fatal Python error: Segmentation fault\n"),
            {
                "status": "notRun",
                "message": "unittest did not run test_math.TestMath.test_adds:\nFatal Python error: Segmentation fault",
            },
        )


if __name__ == "__main__":
    unittest.main()
