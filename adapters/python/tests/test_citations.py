"""Citations of the cases of parametrized tests and of subtests, by the ids pytest and
unittest print, read from syntax.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from talk import HELLO, Talk, opened  # noqa: E402
from test_core import USABLE, core  # noqa: E402

PARAMETRIZED = """\
import pytest
from pytest import mark, param

from lib import add

CASES = [(1, 2, 3), (2, 2, 4)]
GROWN = [(1, 1)]
GROWN.append((2, 2))
pytestmark = pytest.mark.slow


@pytest.mark.parametrize("a,b,total", CASES, ids=["small", "even"])
def test_named(a, b, total):
    assert add(a, b) == total


@pytest.mark.parametrize("percent", [10, -2.5, "half off", None, (1, 2)])
def test_generated(percent):
    pass


@pytest.mark.parametrize(("x", "y"), [param(1, 2, id="given"), param(3, 4), (5, 6)], ids=[None, "listed", None])
def test_params(x, y):
    pass


@pytest.mark.parametrize("x", [0, 1])
@mark.parametrize("y", ["a"])
def test_stacked(x, y):
    pass


@pytest.mark.parametrize("region", ["NL"])
class TestPrices:
    @pytest.mark.parametrize("price", [100])
    def test_total(self, region, price):
        pass


@pytest.mark.parametrize("x", ["same", "same"])
def test_repeated_ids(x):
    pass


@pytest.mark.parametrize("x", [add(1, 2)])
def test_computed_value(x):
    pass


@pytest.mark.parametrize("x", GROWN)
def test_grown_table(x):
    pass
"""

SUBTESTS = """\
import unittest

REGIONS = {"NL": 21, "DE": 19}


class TestTax(unittest.TestCase):
    def test_messages(self):
        for name, rate in [("big discount", 50), ("no discount", 0)]:
            with self.subTest(name):
                self.assertGreaterEqual(rate, 0)

    def test_params(self):
        cases = [{"region": "NL", "rate": 21}, {"region": "DE", "rate": 19}]
        for case in cases:
            with self.subTest(region=case["region"]):
                pass

    def test_items(self):
        for region, rate in REGIONS.items():
            with self.subTest(f"{region}", rate=rate):
                pass
        for region, rate in REGIONS.items():
            with self.subTest(region, rate=rate):
                pass

    def test_literal(self):
        with self.subTest("alone"):
            with self.subTest("nested"):
                pass

    def test_computed(self):
        for k in range(3):
            with self.subTest(k):
                pass
"""


def listing(file, text):
    request = {"id": 3, "method": "tests", "params": {"file": file}}
    talk = Talk(HELLO, opened({"lib.py": "def add(a, b):\n    return a + b\n", file: text}), request)
    return [(" > ".join([*t["describes"], t["title"]]), t["line"]) for t in talk.result(3)["tests"]]


class Citations(unittest.TestCase):
    def test_the_cases_of_parametrized_tests(self):
        self.assertEqual(
            listing("tests/test_p.py", PARAMETRIZED),
            [
                ("test_named", 13),
                ("test_named > small", 6),
                ("test_named > even", 6),
                ("test_generated", 18),
                ("test_generated > 10", 17),
                ("test_generated > -2.5", 17),
                ("test_generated > half off", 17),
                ("test_generated > None", 17),
                ("test_generated > percent4", 17),
                ("test_params", 23),
                ("test_params > given", 22),
                ("test_params > listed", 22),
                ("test_params > 5-6", 22),
                ("test_stacked", 29),
                ("test_stacked > a-0", 28),
                ("test_stacked > a-1", 28),
                ("TestPrices > test_total", 36),
                ("TestPrices > test_total > 100-NL", 35),
                ("test_repeated_ids", 41),
                ("test_computed_value", 46),
                ("test_grown_table", 51),
            ],
        )

    def test_a_pytestmark_that_parametrizes_hides_the_cases(self):
        text = PARAMETRIZED.replace("pytest.mark.slow", 'pytest.mark.parametrize("z", [1])')
        self.assertNotIn(("test_named > small", 6), listing("tests/test_p.py", text))

    def test_the_cases_of_subtests(self):
        self.assertEqual(
            listing("tests/test_s.py", SUBTESTS),
            [
                ("TestTax > test_messages", 7),
                ("TestTax > test_messages > big discount", 8),
                ("TestTax > test_messages > no discount", 8),
                ("TestTax > test_params", 12),
                ("TestTax > test_params > region='NL'", 13),
                ("TestTax > test_params > region='DE'", 13),
                ("TestTax > test_items", 18),
                ("TestTax > test_items > NL (rate=21)", 3),
                ("TestTax > test_items > DE (rate=19)", 3),
                ("TestTax > test_literal", 26),
                ("TestTax > test_literal > alone", 27),
                ("TestTax > test_computed", 31),
            ],
        )

    @unittest.skipUnless(USABLE, "no docattest core binary (set DOCATTEST_CORE)")
    def test_a_case_is_cited_by_its_id(self):
        page = (
            'Small sums. <!-- test: tests/test_p.py > "test_named" > "small" -->\n\n'
            'Prices. <!-- test: tests/test_p.py > "TestPrices" > "test_total" > "100-NL" -->\n\n'
            'Big discounts. <!-- test: tests/test_s.py > "TestTax" > "test_messages" > "big discount" -->\n\n'
            'Renamed. <!-- test: tests/test_p.py > "test_named" > "large" -->\n'
        )
        files = {
            "lib.py": "def add(a, b):\n    return a + b\n",
            "tests/test_p.py": PARAMETRIZED,
            "tests/test_s.py": SUBTESTS,
            "docs/cited.md": page,
            "docattest.config.json": json.dumps(
                {"pages": ["docs/*.md"], "surfaces": {"default": "./lib.py"}, "adapter": "python"}
            ),
        }
        with tempfile.TemporaryDirectory() as root:
            for rel, text in files.items():
                os.makedirs(os.path.dirname(os.path.join(root, rel)), exist_ok=True)
                with open(os.path.join(root, rel), "w", encoding="utf-8") as handle:
                    handle.write(text)
            _, out, err = core("check", "--json", root=root)
        results = [(r["status"], r["message"]) for r in json.loads(out)["results"]] if out else err
        self.assertEqual(
            results[:3],
            [
                ("pass", 'tests/test_p.py:6 declares "test_named" > "small" (not run by docattest)'),
                (
                    "pass",
                    'tests/test_p.py:35 declares "TestPrices" > "test_total" > "100-NL" (not run by docattest)',
                ),
                (
                    "pass",
                    'tests/test_s.py:8 declares "TestTax" > "test_messages" > "big discount" (not run by docattest)',
                ),
            ],
        )
        self.assertEqual(results[3][0], "error")
        self.assertIn(
            'tests/test_p.py declares no test "test_named" > "large"; tests there: "test_named", "test_named" > "small", "test_named" > "even"',
            results[3][1],
        )

    def test_a_case_attests_what_its_test_uses(self):
        cite = {
            "id": "cite",
            "kind": "test",
            "page": "docs/a.md",
            "line": 1,
            "claim": "",
            "file": "tests/test_p.py",
            "title": "small",
            "describes": ["test_named"],
        }
        request = {"id": 3, "method": "symbols", "params": {"attestations": [cite]}}
        files = {"lib.py": "def add(a, b):\n    return a + b\n", "tests/test_p.py": PARAMETRIZED}
        self.assertEqual(
            Talk(HELLO, opened(files), request).result(3), {"cite": [{"name": "add", "surface": "default"}]}
        )


if __name__ == "__main__":
    unittest.main()
