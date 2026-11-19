"""The protocol server, driven the way the core drives it: request lines in, response
lines out, one process per conversation.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from talk import HELLO, SURFACE, Talk, example, fact, opened  # noqa: E402


class Hello(unittest.TestCase):
    def test_hello_is_static_and_the_same_warm(self):
        cold = Talk(HELLO, {"id": 2, "method": "hello", "params": {"protocol": 2}})
        warm = Talk({"id": 1, "method": "hello", "params": {"protocol": 1, "warm": True}})
        hello = cold.result(1)
        self.assertEqual(hello["protocol"], 1)
        self.assertEqual(hello["adapter"]["name"], "python")
        self.assertIn("python", hello["blockLanguages"])
        self.assertEqual(hello["sourceFiles"], {"include": ["**/*.py"], "exclude": []})
        self.assertIn("__pycache__", hello["ignoredNames"])
        self.assertIn("*.egg-info", hello["ignoredNames"])
        self.assertEqual(hello["ignoredMarkers"], ["pyvenv.cfg"])
        self.assertEqual(hello["infoWords"], ["continue"])
        self.assertEqual(
            hello["capabilities"], ["tests", "runTests", "observables", "affected", "display", "mutants"]
        )
        self.assertEqual(warm.result(1), hello)
        self.assertEqual(
            cold.error(2),
            {"kind": "user", "message": "docattest: the python adapter speaks protocol 1, not 2"},
        )
        self.assertEqual(cold.status, 0)

    def test_mistakes_by_the_core_are_internal_and_do_not_end_the_process(self):
        open_ = opened({"lib.py": "raise RuntimeError('boom')"})
        talk = Talk(
            {"id": 9, "method": "exports", "params": {"surface": "default"}},
            HELLO,
            "this is not JSON",
            "",
            {"id": 3, "method": "teleport", "params": {}},
            {"id": 4, "method": "exports", "params": {"surface": "default"}},
            open_,
            dict(open_, id=5),
            {"id": 6, "method": "observables", "params": {"surface": "default"}},
            {"id": 7, "method": "mentions", "params": {"spans": "x"}},
        )
        self.assertEqual(
            talk.error(9), {"kind": "internal", "message": '"exports" before "hello": hello is always first'}
        )
        self.assertEqual(
            talk.error(None), {"kind": "internal", "message": "a request line is not JSON: this is not JSON"}
        )
        self.assertEqual(talk.error(3), {"kind": "internal", "message": 'unknown method "teleport"'})
        self.assertEqual(talk.error(4), {"kind": "internal", "message": '"exports" before "open"'})
        self.assertEqual(talk.result(2), {})
        self.assertEqual(talk.error(5)["kind"], "internal")
        self.assertEqual(
            talk.error(6),
            {
                "kind": "user",
                "message": 'docattest: surface "default" failed to load: RuntimeError: boom',
            },
        )
        self.assertEqual(talk.error(7)["kind"], "internal")
        self.assertEqual(talk.status, 0)

    def test_close_waits_and_ends_the_session(self):
        slow = example("import time\ntime.sleep(0.3)\nassert True")
        talk = Talk(
            HELLO,
            opened({"lib.py": SURFACE}),
            {"id": 3, "method": "run", "params": {"examples": [slow], "cases": []}},
            {"id": 4, "method": "close", "params": {}},
            {"id": 5, "method": "exports", "params": {"surface": "default"}},
        )
        self.assertEqual(talk.result(3)[0]["status"], "pass")
        self.assertEqual(talk.result(4), {})
        self.assertEqual(talk.error(5), {"kind": "internal", "message": '"exports" after "close"'})
        self.assertLess(talk.responses.index(talk.of(3)), talk.responses.index(talk.of(4)))

    def test_a_missing_or_mistyped_field_is_refused_where_it_enters(self):
        no_code = {k: v for k, v in example("assert True").items() if k != "code"}
        unknown = dict(example("assert True"), kind="snapshot")
        session = opened({"lib.py": SURFACE})
        talk = Talk(
            HELLO,
            {
                "id": 2,
                "method": "open",
                "params": {k: v for k, v in session["params"].items() if k != "timeoutMs"},
            },
            dict(session, id=6),
            {"id": 3, "method": "compile", "params": {"examples": [no_code]}},
            {"id": 4, "method": "symbols", "params": {"attestations": [unknown]}},
            {"id": 5, "method": "evaluate", "params": {"facts": [dict(fact("N"), line="1")]}},
        )
        self.assertEqual(talk.error(2), {"kind": "internal", "message": 'open: "timeoutMs" must be a number'})
        self.assertEqual(
            talk.error(3), {"kind": "internal", "message": 'compile: "examples"[0]: "code" must be a string'}
        )
        self.assertEqual(
            talk.error(4),
            {"kind": "internal", "message": 'symbols: "attestations"[0]: unknown kind "snapshot"'},
        )
        self.assertEqual(
            talk.error(5), {"kind": "internal", "message": 'evaluate: "facts"[0]: "line" must be an integer'}
        )


class Text(unittest.TestCase):
    def test_assertions(self):
        codes = [
            ("python", "assert add(1, 1) == 2"),
            ("python", "print(add(1, 1))"),
            ("py", "import pytest\nwith pytest.raises(ValueError):\n    f()"),
            ("python", "self.assertEqual(1, 1)"),
            ("pycon", ">>> add(1, 1)\n2"),
            ("pycon", ">>> x = add(1, 1)"),
            ("python", ">>> add(1, 1)\n2"),
            ("python", "this does not parse ("),
        ]
        talk = Talk(
            HELLO,
            {
                "id": 2,
                "method": "assertions",
                "params": {"examples": [{"lang": lang, "code": code} for lang, code in codes]},
            },
        )
        self.assertEqual(talk.result(2), [True, False, True, True, True, False, True, True])

    def test_docstrings_are_prose_and_their_sessions_are_examples(self):
        source = '"""The module."""\n\n\ndef double(n):\n    """Doubles a number.\n\n    Twice, <!-- no --> exactly.\n\n    Args:\n        n: the number.\n\n    >>> double(2)\n    4\n    """\n    return n * 2\n\n\nclass Box:\n    def open(self):\n        """Opens it.\n\n        >>> Box().open()\n        """\n'
        files = [{"page": "src/a.py", "text": source}, {"page": "docs/a.md", "text": source}]
        talk = Talk(
            HELLO,
            {"id": 2, "method": "docProse", "params": {"files": files}},
            {"id": 3, "method": "docComments", "params": {"files": files}},
        )
        prose = talk.result(2)
        self.assertEqual(prose[1], [])
        self.assertEqual(
            prose[0],
            [
                {"text": "The module.", "start": 1, "end": 1},
                {"text": "Doubles a number. Twice, exactly.", "start": 5, "end": 14},
                {"text": "Opens it.", "start": 20, "end": 23},
            ],
        )
        first, second = talk.result(3)
        self.assertEqual(first["id"], "src/a.py:5:example:0")
        self.assertEqual(
            (first["page"], first["line"], first["kind"], first["mode"], first["lang"]),
            ("src/a.py", 5, "example", "check", "pycon"),
        )
        self.assertEqual(first["claim"], prose[0][1]["text"])
        self.assertEqual(first["owner"], "double")
        self.assertFalse(first["weak"])
        self.assertIn(">>> double(2)", first["code"])
        self.assertEqual((second["owner"], second["line"], second["weak"]), ("Box", 20, True))

    def test_mentions(self):
        spans = [
            "retry(fn)",
            "Client()",
            "await retry(fn)",
            "@retry",
            "mypkg.retry",
            'LIMITS["small"]',
            "retry.py",
            "retry-after",
            "retries",
            "docs/retry",
        ]
        talk = Talk(
            HELLO,
            {
                "id": 2,
                "method": "mentions",
                "params": {"spans": spans, "symbols": ["retry", "Client", "LIMITS"]},
            },
        )
        self.assertEqual(
            talk.result(2), ["retry", "Client", "retry", "retry", "retry", "LIMITS", None, None, None, None]
        )

    def test_expressions(self):
        expressions = [
            "min(LIMIT, 3)",
            "min(MAX_TRIES, 3)",
            "LIMIT + 3",
            "[r['a'] for r in ROWS if r is not None]",
        ]
        before, renamed, other, loop = Talk(
            HELLO, {"id": 2, "method": "expressions", "params": {"expressions": expressions}}
        ).result(2)
        self.assertEqual(before["names"], ["min", "LIMIT"])
        self.assertEqual(renamed["shape"], before["shape"])
        self.assertNotEqual(other["shape"], before["shape"])
        self.assertEqual(loop["names"], ["r", "r", "ROWS", "r"])

    def test_display_shows_a_data_blocks_values_as_python_does(self):
        values = [True, False, None, 3, 1.0, 1.5, 1e21, "a b"]
        shown = Talk(HELLO, {"id": 2, "method": "display", "params": {"values": values}}).result(2)
        self.assertEqual(shown, ["True", "False", "None", "3", "1.0", "1.5", "1e+21", "a b"])


class Values(unittest.TestCase):
    """How Python values reach the core's comparison (the JSON form of ``Observation``)."""

    # Each fact evaluated by test_evaluations, by its id.
    EXPRESSIONS = {
        "int": "N + 1",
        "float": "RATE",
        "bool": "ON",
        "none": "NOTHING",
        "str": "SHAPE",
        "list": "KEYS",
        "tuple": "PAIR",
        "set": "TAGS",
        "dict": "LIMITS",
        "generator": "(k.lower() for k in KEYS)",
        "await": "await version()",
        "call": "add(2, 2)",
        "private": "_PRIVATE",
        "unbound": "_NOWHERE",
        "raises": "LIMITS['nope']",
        "syntax": "N +",
        "big": "2 ** 70",
        "stdlib": "math.floor(RATE) + len(inspect.signature(add).parameters)",
    }

    def test_evaluations(self):
        facts = [fact(e, id=k) for k, e in self.EXPRESSIONS.items()] + [
            fact("N", id="unknown", surface="nope")
        ]
        talk = Talk(
            HELLO, opened({"lib.py": SURFACE}), {"id": 3, "method": "evaluate", "params": {"facts": facts}}
        )
        got = talk.result(3)
        self.assert_scalars(got)
        self.assert_collections(got)
        self.assert_computed(got)
        self.assert_refusals(got)

    def assert_scalars(self, got):
        self.assertEqual(got["int"], {"ok": True, "value": {"text": "4", "description": "int 4"}})
        self.assertEqual(got["float"]["value"], {"text": "1.5", "description": "float 1.5"})
        self.assertEqual(got["bool"]["value"], {"text": "True", "description": "bool True"})
        self.assertEqual(got["none"]["value"], {"text": "None", "description": "None"})
        self.assertEqual(got["str"]["value"]["description"][:13], "str 'class Po")
        # Text says that it is text; nothing else says anything about its type.
        self.assertEqual(got["str"]["value"]["kind"], "text")
        self.assertEqual([k for k, v in got.items() if v["ok"] and "kind" in v["value"]], ["str", "await"])
        self.assertNotIn("items", got["str"]["value"])

    def assert_collections(self, got):
        self.assertEqual(
            got["list"]["value"],
            {"text": "['A', 'B', 'C']", "description": "a list", "items": ["A", "B", "C"]},
        )
        self.assertEqual(got["tuple"]["value"]["items"], ["x", "1"])
        self.assertEqual(got["tuple"]["value"]["description"], "a tuple")
        self.assertEqual(got["set"]["value"]["items"], ["only"])
        self.assertEqual(got["dict"]["value"]["items"], ["small", "large"])
        self.assertEqual(got["dict"]["value"]["description"], "a dict")
        self.assertEqual(got["generator"]["value"]["items"], ["a", "b", "c"])

    def assert_computed(self, got):
        self.assertEqual(got["await"]["value"]["text"], "2.0")
        self.assertEqual(got["call"]["value"]["text"], "4")
        self.assertEqual(got["big"]["value"]["text"], str(2**70))
        # A constant is read from source, exported or not.
        self.assertEqual(got["private"]["value"]["text"], "7")
        self.assertEqual(got["stdlib"]["value"]["text"], "3")

    def assert_refusals(self, got):
        self.assertEqual(
            got["unbound"],
            {
                "ok": False,
                "message": "fact `_NOWHERE`: _NOWHERE is not exported by surface \"default\" (NameError: name '_NOWHERE' is not defined)",
            },
        )
        self.assertEqual(
            got["raises"], {"ok": False, "message": "fact `LIMITS['nope']` raised KeyError: 'nope'"}
        )
        self.assertIn("cannot parse fact expression `N +`", got["syntax"]["message"])
        self.assertEqual(
            got["unknown"], {"ok": False, "message": 'unknown surface "nope" (configured: default)'}
        )

    def test_setup_facts_and_snapshots(self):
        files = {
            "lib.py": "N = 3\nERRORS = {'missing': {'field': 'name'}, 'invalid': {'field': 'email'}}\n",
            "docs/setup.py": "from lib import ERRORS\nfacts = {'error_fields': lambda: [e['field'] for e in ERRORS.values()], 'N': lambda: 4}\n",
        }
        facts = [
            fact("error_fields()", id="helper"),
            fact("N", id="clash"),
            fact("[r['code'] for r in snapshot('regions')]", id="snap"),
            fact("snapshot('nope')", id="missing"),
        ]
        snapshots = {"dir": "docs/.snapshots", "data": {"regions": [{"code": "eu"}, {"code": "us"}]}}
        talk = Talk(
            HELLO,
            opened(files, setup="docs/setup.py", snapshots=snapshots),
            {"id": 3, "method": "evaluate", "params": {"facts": facts}},
            {"id": 4, "method": "symbols", "params": {"attestations": facts}},
        )
        got = talk.result(3)
        self.assertEqual(got["helper"]["value"]["items"], ["name", "email"])
        self.assertIn('"N" is both an export of surface "default" and a setup fact', got["clash"]["message"])
        self.assertEqual(got["snap"]["value"]["items"], ["eu", "us"])
        self.assertIn('snapshot "nope" not found in docs/.snapshots', got["missing"]["message"])
        self.assertEqual(talk.result(4)["helper"], [{"name": "ERRORS", "surface": "default"}])

    def test_manifests_are_read_by_the_path_a_fact_writes(self):
        facts = [
            fact("manifest('pyproject.toml')['project']['requires-python'].removeprefix('>=')", id="read"),
            fact("manifest('other.toml')", id="unread"),
        ]
        manifests = {"pyproject.toml": {"project": {"requires-python": ">=3.10"}}}
        talk = Talk(
            HELLO,
            opened({"lib.py": "N = 3\n"}, manifests=manifests),
            {"id": 3, "method": "evaluate", "params": {"facts": facts}},
        )
        got = talk.result(3)
        self.assertEqual(got["read"]["value"]["text"], "3.10")
        self.assertIn('manifest "other.toml" was not read', got["unread"]["message"])

    def test_a_file_is_read_as_text_by_the_path_a_fact_writes(self):
        facts = [
            fact("re.search(r'USER (\\d+)', file('Dockerfile'))[1]", id="read"),
            fact("file('Dockerfile.dev')", id="unread"),
        ]
        talk = Talk(
            HELLO,
            opened({"lib.py": "N = 3\n"}, texts={"Dockerfile": "FROM python:3.12\nUSER 10001\n"}),
            {"id": 3, "method": "evaluate", "params": {"facts": facts}},
        )
        got = talk.result(3)
        self.assertEqual((got["read"]["value"]["text"], got["read"]["value"]["kind"]), ("10001", "text"))
        self.assertIn('file "Dockerfile.dev" was not read', got["unread"]["message"])

    def test_matched_and_field_are_what_the_core_worked_out_for_the_arguments_a_fact_writes(self):
        facts = [
            fact("matched('Dockerfile', r'USER (\\d+)')", id="matched"),
            fact("manifestField('ci.yml', 'deploy.replicas')", id="field"),
            fact("matched('Dockerfile', 'USER')", id="unread-match"),
            fact("manifestField('ci.yml', 'deploy.image')", id="unread-field"),
        ]
        read = {
            "matches": {"Dockerfile": {"USER (\\d+)": "10001"}},
            "manifestFields": {"ci.yml": {"deploy.replicas": 3}},
        }
        talk = Talk(
            HELLO,
            opened({"lib.py": "N = 3\n"}, **read),
            {"id": 3, "method": "evaluate", "params": {"facts": facts}},
        )
        got = talk.result(3)
        self.assertEqual(
            (got["matched"]["value"]["text"], got["matched"]["value"]["kind"]), ("10001", "text")
        )
        self.assertEqual(got["field"]["value"], {"text": "3", "description": "int 3"})
        self.assertIn("matched('Dockerfile', 'USER') was not read", got["unread-match"]["message"])
        self.assertIn("manifestField('ci.yml', 'deploy.image') was not read", got["unread-field"]["message"])


if __name__ == "__main__":
    unittest.main()
