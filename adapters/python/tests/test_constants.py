"""Facts that read constants from source: which assignments are constants, what a fact
reads without importing its surface, what it may not read, and how ``where`` and
``impact`` count such constants.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, PACKAGE_ROOT)

from talk import HELLO, Talk, fact, opened  # noqa: E402
from test_core import ENV, USABLE, core  # noqa: E402

from docattest_python.analysis.constants import constants  # noqa: E402
from docattest_python.analysis.modules import Module  # noqa: E402

# A module whose import would fail loudly: nothing a fact reads from source may run it.
SCRIPT = """\
import os

__all__ = ["serve"]

_PORT = 8080
TIMEOUT: int = 30 * 60
HOST = "localhost"
ADDRESS = HOST + ":" + "8080"
REGIONS = {"NL": 21, "DE": 19}
_started = os.getpid()


def _round(value):
    return int(value + 0.5)


def serve():
    pass


raise SystemExit("the script ran")
"""

PROGRAM = """\
PORT = 9090
_LEVEL = 2


def main():
    pass


main()
raise SystemExit("the program ran")
"""

FILES = {"script.py": SCRIPT, "app/__init__.py": "", "app/__main__.py": PROGRAM}
SURFACES = [{"name": "default", "entry": "./script.py"}, {"name": "app", "entry": "./app/__main__.py"}]


def evaluate(*facts):
    request = {"id": 3, "method": "evaluate", "params": {"facts": list(facts)}}
    return Talk(HELLO, opened(FILES, surfaces=SURFACES), request).result(3)


def texts(values):
    return {k: v["value"]["text"] if v["ok"] else v["message"] for k, v in values.items()}


class Constants(unittest.TestCase):
    def test_what_is_a_constant(self):
        source = (
            "import os\nA = 1\nB: float = A / 4\nC = -A + 2 * 3 // 2 % 5\nD = 'a' + 'b'\nE = (A, [B], {'k': {D}})\n"
            "F = G = b'x'\nH = 1 + 2j\nTWICE = 1\nTWICE = 2\nCALL = os.getcwd()\nFORMATTED = f'{A}'\n"
            "REPEATED = '-' * 3\nMIXED = 'a' + 1\nLOOP = LOOP\nINFINITE = 1e999\nZERO = 1 // 0\nSTAR = [*E]\n"
            "UNHASHABLE = {[1]}\nif A:\n    INSIDE = 1\nfor LOOPED in []:\n    pass\nLATER = AFTER\nAFTER = 1\n"
        )
        found = constants(Module("m", "m.py", source, False))
        self.assertEqual(
            found,
            {
                "A": 1,
                "B": 0.25,
                "C": 2,
                "D": "ab",
                "E": (1, [0.25], {"k": {"ab"}}),
                "F": b"x",
                "G": b"x",
                "H": 1 + 2j,
                "AFTER": 1,
            },
        )

    def test_a_fact_reads_constants_from_source(self):
        got = evaluate(
            fact("_PORT", id="private"),
            fact("TIMEOUT // 60", id="outside __all__"),
            fact("ADDRESS", id="concatenated"),
            fact("list(REGIONS)", id="dict"),
            fact("PORT", id="program", surface="app"),
            fact("_LEVEL", id="program private", surface="app"),
        )
        self.assertEqual(
            texts(got),
            {
                "private": "8080",
                "outside __all__": "30",
                "concatenated": "localhost:8080",
                "dict": "['NL', 'DE']",
                "program": "9090",
                "program private": "2",
            },
        )

    def test_nothing_of_the_module_runs_unless_a_fact_reads_something_else(self):
        got = evaluate(fact("_PORT", id="constant"), fact("serve()", id="export"))
        self.assertEqual(
            texts(got),
            {"constant": "8080", "export": 'surface "default" failed to load: SystemExit: the script ran'},
        )

    def test_only_constants_are_read_from_source(self):
        got = evaluate(
            fact("_round(0.5)", id="function"),
            fact("_started", id="variable"),
            fact("main()", id="program function", surface="app"),
        )
        self.assertEqual(
            texts(got),
            {
                "function": 'fact `_round(0.5)`: _round is a function that surface "default" does not export; of its unexported names, a fact reads only constants',
                "variable": 'fact `_started`: _started is a variable that surface "default" does not export; of its unexported names, a fact reads only constants',
                "program function": 'fact `main()`: main is a function of surface "app", a program (`__main__.py`); of a program, a fact reads only constants',
            },
        )

    def test_a_program_offers_its_exported_constants_without_running(self):
        request = {"id": 3, "method": "observables", "params": {"surface": "app"}}
        found = Talk(HELLO, opened(FILES, surfaces=SURFACES), request).result(3)
        self.assertEqual([(o["expression"], o["value"]["text"]) for o in found], [("PORT", "9090")])

    def test_a_fact_attests_the_constants_it_reads(self):
        facts = [dict(fact("_PORT + TIMEOUT", id="read"), page="docs/a.md")]
        request = {"id": 3, "method": "symbols", "params": {"attestations": facts}}
        got = Talk(HELLO, opened(FILES, surfaces=SURFACES), request).result(3)
        self.assertEqual(
            got["read"], [{"name": "_PORT", "surface": "default"}, {"name": "TIMEOUT", "surface": "default"}]
        )


@unittest.skipUnless(USABLE and shutil.which("git"), "needs the docattest core and git")
class ImpactOfConstants(unittest.TestCase):
    def test_impact_lists_a_constant_with_the_facts_that_read_it(self):
        with tempfile.TemporaryDirectory() as temp:
            root = os.path.realpath(temp)
            files = dict(
                FILES,
                **{
                    "docs/ports.md": "Port `8080`. <!-- fact: _PORT -->\n\nPort `9090`. <!-- fact @app: PORT -->\n",
                    "docattest.config.json": json.dumps(
                        {
                            "pages": ["docs/*.md"],
                            "surfaces": {"default": "./script.py", "app": "./app/__main__.py"},
                            "adapter": "python",
                        }
                    ),
                },
            )
            for rel, text in files.items():
                os.makedirs(os.path.dirname(os.path.join(root, rel)), exist_ok=True)
                with open(os.path.join(root, rel), "w", encoding="utf-8") as handle:
                    handle.write(text)
            for args in (["init", "-q", "-b", "main"], ["add", "-A"], ["commit", "-q", "-m", "base"]):
                subprocess.run(["git", *args], cwd=root, env=ENV, check=True, capture_output=True)
            for rel, old, new in (
                ("script.py", "_PORT = 8080", "_PORT = 8081"),
                ("app/__main__.py", "9090", "9091"),
            ):
                path = os.path.join(root, rel)
                with open(path, encoding="utf-8") as handle:
                    text = handle.read()
                with open(path, "w", encoding="utf-8") as handle:
                    handle.write(text.replace(old, new))
            code, out, err = core("impact", "--json", root=root)
        self.assertEqual(code, 0, err)
        references = [(r["symbol"], r["page"], r["kind"]) for r in json.loads(out)["references"]]
        self.assertEqual(references, [("PORT", "docs/ports.md", "fact"), ("_PORT", "docs/ports.md", "fact")])


if __name__ == "__main__":
    unittest.main()
