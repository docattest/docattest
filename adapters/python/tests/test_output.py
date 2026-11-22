"""Tests for ``# Output:`` / ``# Unordered output:`` and surface-optional facts in Python."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from docattest_python.language import asserts
from docattest_python.workers.docattest_worker.output import ExpectedOutput, compare_output, expected_output
from tests.talk import HELLO, Talk


class OutputComments(unittest.TestCase):
    def test_parses_single_multiline_and_unordered(self) -> None:
        self.assertEqual(expected_output('print("7.50 EUR")\n# Output: 7.50 EUR\n'), ("7.50 EUR", False))
        self.assertEqual(
            expected_output('print("b")\nprint("a")\n# Unordered output:\n# a\n# b\n'),
            ("a\nb", True),
        )
        self.assertIsNone(expected_output("# Output: 1\nprint(1)\n# trailing\n"))

    def test_counts_as_assertion(self) -> None:
        self.assertTrue(asserts("python", 'print("ok")\n# Output: ok\n'))
        self.assertFalse(asserts("python", 'print("ok")\n'))

    def test_compare_output_ordered_and_unordered(self) -> None:
        self.assertEqual(compare_output("b\na\n", ExpectedOutput("a\nb", True))["status"], "pass")
        bad = compare_output("actual\n", ExpectedOutput("expected", False))
        self.assertEqual(bad["status"], "fail")
        self.assertEqual(bad.get("expected"), "expected")
        self.assertEqual(bad.get("actual"), "actual")


class RunOutputAndNoSurfaces(unittest.TestCase):
    def test_runs_output_block_and_evaluates_fact_without_surfaces(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp, "pyproject.toml").write_text(
                '[project]\nname = "demo"\nversion = "3.1.0"\n', encoding="utf-8"
            )
            talk = Talk(
                HELLO,
                {
                    "id": 2,
                    "method": "open",
                    "params": {
                        "root": tmp,
                        "surfaces": [],
                        "timeoutMs": 5000,
                        "workers": 1,
                        "network": False,
                        "snapshots": {"data": {}},
                        "manifests": {"pyproject.toml": {"project": {"name": "demo", "version": "3.1.0"}}},
                        "examples": [],
                    },
                },
                {
                    "id": 3,
                    "method": "evaluate",
                    "params": {
                        "facts": [
                            {
                                "id": "f1",
                                "kind": "fact",
                                "page": "README.md",
                                "line": 1,
                                "surface": "default",
                                "expression": 'manifest("pyproject.toml")["project"]["version"]',
                                "ordered": False,
                                "claim": "",
                                "target": {"kind": "scalar", "value": "3.1.0"},
                            }
                        ]
                    },
                },
                {
                    "id": 4,
                    "method": "run",
                    "params": {
                        "examples": [
                            {
                                "id": "e1",
                                "kind": "example",
                                "page": "README.md",
                                "line": 3,
                                "lang": "python",
                                "mode": "check",
                                "network": False,
                                "weak": False,
                                "code": 'print("hello")\n# Output: hello\n',
                                "claim": "",
                            },
                            {
                                "id": "e2",
                                "kind": "example",
                                "page": "README.md",
                                "line": 8,
                                "lang": "python",
                                "mode": "check",
                                "network": False,
                                "weak": False,
                                "code": 'print("wrong")\n# Output: right\n',
                                "claim": "",
                            },
                        ],
                        "cases": [],
                    },
                },
                {"id": 5, "method": "close", "params": {}},
            )
            evaluated = talk.result(3)
            self.assertTrue(evaluated["f1"]["ok"])
            self.assertEqual(evaluated["f1"]["value"]["text"], "3.1.0")
            ran = talk.result(4)
            self.assertEqual(ran[0]["status"], "pass")
            self.assertEqual(ran[0]["message"], "printed the output it states")
            self.assertEqual(ran[1]["status"], "fail")
            self.assertIn('expected output "right", got "wrong"', ran[1]["message"])


if __name__ == "__main__":
    unittest.main()
