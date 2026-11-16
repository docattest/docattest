"""The protocol server's static answers, read from syntax: exports, compile, symbols,
tests, inCode, affected, and the observables of a surface.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from talk import HELLO, Talk, example, fact, opened  # noqa: E402


class Static(unittest.TestCase):
    PACKAGE = {
        "pyproject.toml": '[project]\nname = "demo-kit"\nversion = "1.2.3"\n',
        "src/kit/__init__.py": 'from .core import LIMIT, retry\nfrom .core import helper as _helper\nfrom . import core\nimport os\nVERSION = "1"\n',
        "src/kit/core.py": "import json\nLIMIT = 3\n\n\ndef helper():\n    return LIMIT\n\n\ndef retry(fn):\n    return helper() and fn()\n\n\ndef unused():\n    return json\n",
        "src/kit/extra.py": '__all__ = ["shown"]\nshown = 1\nhidden = 2\n',
        "tests/test_core.py": "import kit\nfrom kit import retry\nfrom kit.core import LIMIT\n\n\ndef test_retries():\n    retry(print)\n\n\nclass TestLimit:\n    def test_limit(self):\n        assert LIMIT == 3 and kit.VERSION\n\n    async def test_async(self):\n        pass\n\n\ndef helper_not_a_test():\n    pass\n",
    }
    SURFACES = [
        {"name": "default", "entry": "kit"},
        {"name": "extra", "entry": "./src/kit/extra.py"},
        {"name": "aliased", "entry": {"import": "shiny", "path": "src/kit/core.py"}},
        {"name": "gone", "entry": "./src/missing.py"},
    ]

    # Facts (id 4) and an example (id 5) that read the surfaces by the names they resolved to.
    READ_THROUGH_SURFACES = (
        {
            "id": 4,
            "method": "evaluate",
            "params": {
                "facts": [
                    fact("LIMIT", id="a", surface="aliased"),
                    fact("shown", id="b", surface="extra"),
                    fact("x", id="c", surface="gone"),
                ]
            },
        },
        {
            "id": 5,
            "method": "run",
            "params": {
                "examples": [
                    example(
                        "from shiny import LIMIT\nimport kit.extra\nassert LIMIT == 3 and kit.extra.shown == 1"
                    )
                ],
                "cases": [],
            },
        },
    )

    def session(self, *requests):
        return Talk(HELLO, opened(self.PACKAGE, surfaces=self.SURFACES), *requests)

    def test_exports_and_where_surfaces_resolved(self):
        talk = self.session(
            *[
                {"id": 10 + k, "method": "exports", "params": {"surface": s}}
                for k, s in enumerate(["default", "extra", "aliased", "gone", "unknown"])
            ],
            {"id": 3, "method": "describeSurfaces", "params": {}},
            *self.READ_THROUGH_SURFACES,
        )
        self.assertEqual(talk.result(10), ["LIMIT", "VERSION", "retry"])
        self.assertEqual(talk.result(11), ["shown"])
        self.assertEqual(talk.result(12), ["LIMIT", "helper", "retry", "unused"])
        self.assertIsNone(talk.result(13))
        self.assertIsNone(talk.result(14))
        self.assert_described(talk.result(3))
        values = talk.result(4)
        self.assertTrue(values["a"]["ok"], values["a"])
        self.assertEqual(values["a"]["value"]["text"], "3")
        self.assertEqual(values["b"]["value"]["text"], "1")
        self.assertEqual(values["c"], {"ok": False, "message": 'surface "gone" (missing) not found'})
        self.assertEqual(talk.result(5)[0]["status"], "pass", talk.result(5))

    def assert_described(self, described):
        self.assertEqual(
            described["default"],
            {
                "resolution": {
                    "import": "kit",
                    "entry": "src/kit/__init__.py",
                    "package": {"name": "demo-kit", "version": "1.2.3", "display": "demo-kit 1.2.3"},
                },
                "warnings": [],
            },
        )
        self.assertEqual(described["extra"]["resolution"]["import"], "kit.extra")
        self.assertEqual(
            described["aliased"]["resolution"],
            {
                "import": "shiny",
                "entry": "src/kit/core.py",
                "package": {"name": "demo-kit", "version": "1.2.3", "display": "demo-kit 1.2.3"},
            },
        )
        self.assertNotIn("resolution", described["gone"])
        self.assertIn("./src/missing.py does not exist", described["gone"]["warnings"][0])

    def test_compile_is_what_can_be_said_without_running(self):
        codes = {
            "ok": "from kit import retry\nimport json\n\nasync def main():\n    return retry(json.dumps)\nraise SystemExit('never runs')",
            "syntax": "from kit import retry\nretry(",
            "module": "import kit\nimport no_such_module_anywhere",
            "name": "from kit import retyr",
            "attribute": "import kit\nkit.retyr(print)",
            "undefined": "from kit import retry\nretry(fn)",
            "await": "import asyncio\nawait asyncio.sleep(0)",
            "outside": "return 1",
            "session": ">>> from kit import retry\n>>> retry(\n",
            "session-name": ">>> from kit import retyr\n",
        }
        examples = [example(c, id=k, mode="compile") for k, c in codes.items()]
        talk = self.session({"id": 3, "method": "compile", "params": {"examples": examples}})
        by_id = {}
        for d in talk.result(3):
            by_id.setdefault(d["id"], []).append((d["line"], d["message"]))
        self.assertNotIn("ok", by_id)
        self.assertNotIn("await", by_id)
        self.assertEqual(by_id["syntax"][0][0], 2)
        self.assertIn("SyntaxError", by_id["syntax"][0][1])
        self.assertEqual(by_id["module"], [(2, "No module named 'no_such_module_anywhere'")])
        self.assertEqual(by_id["name"], [(1, "cannot import name 'retyr' from 'kit' (src/kit/__init__.py)")])
        self.assertEqual(by_id["attribute"], [(2, "module 'kit' has no attribute 'retyr'")])
        self.assertEqual(by_id["undefined"], [(2, "name 'fn' is not defined")])
        self.assertIn("'return' outside function", by_id["outside"][0][1])
        self.assertEqual(by_id["session"][0][0], 2)
        self.assertEqual(
            by_id["session-name"], [(1, "cannot import name 'retyr' from 'kit' (src/kit/__init__.py)")]
        )

    def test_symbols(self):
        attestations = [
            example("from kit import retry, LIMIT as L\nimport os", id="named"),
            example("import kit\nkit.retry(print)\nkit.nothing", id="module"),
            example("from kit.core import retry, unused", id="submodule"),
            example(">>> from kit import retry\n>>> retry(print)", id="session", lang="pycon"),
            fact("min(LIMIT, 3) + len(VERSION)", id="fact"),
            fact("shown", id="other", surface="extra"),
            {
                "id": "cite",
                "kind": "test",
                "page": "docs/a.md",
                "line": 1,
                "claim": "",
                "file": "tests/test_core.py",
                "title": "test_limit",
                "describes": [],
            },
            {
                "id": "cite2",
                "kind": "test",
                "page": "docs/a.md",
                "line": 1,
                "claim": "",
                "file": "tests/test_core.py",
                "title": "test_retries",
                "describes": [],
            },
        ]
        got = self.session({"id": 3, "method": "symbols", "params": {"attestations": attestations}}).result(3)
        names = {k: sorted((r["name"], r["surface"]) for r in v) for k, v in got.items()}
        self.assertEqual(names["named"], [("LIMIT", "default"), ("retry", "default")])
        self.assertEqual(names["module"], [("retry", "default")])
        # Reached through a module that is no surface: it attests the same declaration
        # wherever a surface exports it.
        self.assertEqual(
            names["submodule"], [("retry", "aliased"), ("retry", "default"), ("unused", "aliased")]
        )
        self.assertEqual(names["session"], [("retry", "default")])
        self.assertEqual(names["fact"], [("LIMIT", "default"), ("VERSION", "default")])
        self.assertEqual(names["other"], [("shown", "extra")])
        self.assertEqual(names["cite"], [("LIMIT", "aliased"), ("LIMIT", "default"), ("VERSION", "default")])
        self.assertEqual(names["cite2"], [("retry", "default")])

    def test_tests_are_listed_from_syntax(self):
        talk = self.session(
            *[
                {"id": 3 + k, "method": "tests", "params": {"file": f}}
                for k, f in enumerate(["tests/test_core.py", "tests/gone.py", "README.md"])
            ]
        )
        listing = talk.result(3)
        self.assertEqual(
            listing["tests"],
            [
                {"title": "test_retries", "describes": [], "line": 6},
                {"title": "test_limit", "describes": ["TestLimit"], "line": 11},
                {"title": "test_async", "describes": ["TestLimit"], "line": 14},
            ],
        )
        self.assertEqual(
            listing["suites"], [{"title": "TestLimit", "describes": [], "line": 10, "kind": "a class"}]
        )
        self.assertEqual(talk.result(4), {"ok": False, "message": "tests/gone.py does not exist"})
        self.assertEqual(
            talk.result(5),
            {"ok": False, "message": "README.md is not a Python file, so its tests cannot be read"},
        )

    def test_in_code(self):
        talk = self.session(
            {
                "id": 3,
                "method": "inCode",
                "params": {
                    "names": ["LIMIT", "GONE", "test_retries", "shown"],
                    "files": ["tests/test_core.py"],
                },
            }
        )
        self.assertEqual(talk.result(3), ["LIMIT", "test_retries", "shown"])

    def affected(self, base):
        talk = self.session({"id": 3, "method": "affected", "params": {"base": base, "changed": list(base)}})
        return [(r["surface"], r["name"]) for r in talk.result(3)]

    def test_affected_follows_what_an_export_uses(self):
        core = self.PACKAGE["src/kit/core.py"]
        # helper changed: retry uses it; LIMIT does not.
        self.assertEqual(
            self.affected({"src/kit/core.py": core.replace("return LIMIT", "return LIMIT + 1")}),
            [("aliased", "helper"), ("aliased", "retry"), ("default", "retry")],
        )
        # A constant changed: everything that reads it.
        self.assertEqual(
            self.affected({"src/kit/core.py": core.replace("LIMIT = 3", "LIMIT = 4")}),
            [
                ("aliased", "LIMIT"),
                ("aliased", "helper"),
                ("aliased", "retry"),
                ("default", "LIMIT"),
                ("default", "retry"),
            ],
        )
        # Comments, blank lines and docstrings are not changes.
        self.assertEqual(
            self.affected(
                {
                    "src/kit/core.py": core.replace(
                        "def helper():", 'def helper():  # was\n    """Used to say something else."""'
                    )
                }
            ),
            [],
        )
        # A file that is not code affects nothing; an export that did not exist appeared.
        self.assertEqual(self.affected({"README.md": None}), [])
        self.assertEqual(self.affected({"src/kit/extra.py": None}), [("extra", "shown")])
        self.assertEqual(
            self.affected(
                {"src/kit/__init__.py": self.PACKAGE["src/kit/__init__.py"].replace('VERSION = "1"\n', "")}
            ),
            [("default", "VERSION")],
        )

    def test_observables_offer_values_with_the_expression_that_reads_each(self):
        surface = (
            "from enum import Enum\nfrom dataclasses import dataclass\n"
            "N = 3\nKEYS = ['A', 'B']\nLIMITS = {'small': 1, 'large': 9}\n"
            "ROUTES = [{'method': 'GET', 'path': '/a'}, {'method': 'POST', 'path': '/b'}]\n"
            "class Color(Enum):\n    RED = 'r'\n    BLUE = 'b'\n"
            "@dataclass\nclass Point:\n    x: int = 0\nORIGIN = Point()\n"
            "def f():\n    pass\n"
        )
        talk = Talk(
            HELLO,
            opened({"lib.py": surface}),
            {"id": 3, "method": "observables", "params": {"surface": "default"}},
        )
        found = {o["expression"]: o for o in talk.result(3)}
        self.assertEqual(
            found["N"],
            {
                "surface": "default",
                "expression": "N",
                "path": ["N"],
                "value": {"text": "3", "description": "int 3"},
            },
        )
        self.assertEqual(found["KEYS"]["value"]["items"], ["A", "B"])
        self.assertEqual(found["list(LIMITS)"]["value"]["items"], ["small", "large"])
        self.assertEqual(found["list(LIMITS.values())"]["value"]["items"], ["1", "9"])
        self.assertEqual(found["LIMITS['large']"]["path"], ["LIMITS", "large"])
        self.assertEqual(found["[r['path'] for r in ROUTES]"]["value"]["items"], ["/a", "/b"])
        self.assertEqual(
            found["[f\"{r['method']} {r['path']}\" for r in ROUTES]"]["value"]["items"], ["GET /a", "POST /b"]
        )
        self.assertEqual(found["[m.name for m in Color]"]["value"]["items"], ["RED", "BLUE"])
        self.assertEqual(found["[m.value for m in Color]"]["value"]["items"], ["r", "b"])
        self.assertEqual(found["ORIGIN.x"]["value"]["text"], "0")
        # Whatever is offered evaluates, through its own expression, to the value offered.
        facts = [fact(e, id=e) for e in found]
        again = Talk(
            HELLO, opened({"lib.py": surface}), {"id": 3, "method": "evaluate", "params": {"facts": facts}}
        ).result(3)
        for expression, offered in found.items():
            self.assertTrue(again[expression]["ok"], again[expression])
            self.assertEqual(
                again[expression]["value"].get("items"), offered["value"].get("items"), expression
            )
            self.assertEqual(again[expression]["value"]["text"], offered["value"]["text"], expression)

    def test_observables_offer_named_facts_without_arguments_called_on_the_default_surface(self):
        files = {
            "lib.py": "N = 3\ndef platforms():\n    return {'linux-x64': 'manylinux', 'darwin-arm64': 'macosx'}\n",
            "docs/setup.py": (
                "from lib import platforms\n"
                "async def versions():\n    return {'node': '22.18'}\n"
                "def wheel(name):\n    return platforms()[name]\n"
                "def broken():\n    raise RuntimeError('no')\n"
                "facts = {'names': lambda: list(platforms()), 'versions': versions, 'wheel': wheel,\n"
                "         'broken': broken, 'N': lambda: 4, 'MARK': 'x'}\n"
            ),
        }
        surfaces = [{"name": "default", "entry": "./lib.py"}, {"name": "other", "entry": "./lib.py"}]
        talk = Talk(
            HELLO,
            opened(files, setup="docs/setup.py", surfaces=surfaces),
            {"id": 3, "method": "observables", "params": {"surface": "default"}},
            {"id": 4, "method": "observables", "params": {"surface": "other"}},
        )
        found = {o["expression"]: o["value"].get("items", o["value"]["text"]) for o in talk.result(3)}
        self.assertEqual(
            found,
            {
                "N": "3",
                "names()": ["linux-x64", "darwin-arm64"],
                "list((await versions()))": ["node"],
                "list((await versions()).values())": ["22.18"],
                "(await versions())['node']": "22.18",
            },
        )
        self.assertEqual([o["expression"] for o in talk.result(4)], ["N"])
        # Each helper's value evaluates, as the fact it suggests, to the value offered (`N`
        # clashes with the setup fact of that name, which is why that fact is left out).
        called = {e: v for e, v in found.items() if e != "N"}
        facts = [fact(e, id=e) for e in called]
        again = Talk(
            HELLO,
            opened(files, setup="docs/setup.py"),
            {"id": 3, "method": "evaluate", "params": {"facts": facts}},
        ).result(3)
        self.assertEqual({e: v["value"].get("items", v["value"]["text"]) for e, v in again.items()}, called)

    def test_a_set_is_offered_in_an_order_that_does_not_change(self):
        talk = Talk(
            HELLO,
            opened({"lib.py": "TAGS = {'h', 'c', 'f', 'a', 'e', 'g', 'b', 'd'}\nFROZEN = frozenset()\n"}),
            {"id": 3, "method": "observables", "params": {"surface": "default"}},
        )
        found = {o["expression"]: o["value"] for o in talk.result(3)}
        self.assertEqual(found["TAGS"]["items"], list("abcdefgh"))
        self.assertEqual(found["TAGS"]["text"], "{'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'}")
        self.assertEqual(found["FROZEN"]["text"], "frozenset()")


if __name__ == "__main__":
    unittest.main()
