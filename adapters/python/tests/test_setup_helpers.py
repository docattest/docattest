"""What the setup module's helpers attest: a cases runner, and a named fact that a fact
calls, attest what they read of a surface through the functions of the setup module they
call, its exports and its constants, private ones too, as ``symbols``, ``where``,
``uncovered`` and ``impact`` show.

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
sys.path.insert(0, HERE)

from talk import HELLO, Talk, fact, opened, table  # noqa: E402
from test_core import ENV, USABLE, core  # noqa: E402

LIBRARY = """\
FORBIDDEN_CAPS = ("SYS_ADMIN", "NET_RAW")
COMMAND_PATH = "/usr/bin/shopd"
UNUSED = 1
_PORT = 8080
_ROUTES = {name: f"/{name}" for name in ("health", "orders")}
"""

# The setup module reads the surface's private names as any module may import them.
SETUP = """\
from lib import _PORT, _ROUTES, COMMAND_PATH, FORBIDDEN_CAPS


def caps():
    return list(FORBIDDEN_CAPS)


def address():
    return f"{COMMAND_PATH}:{_PORT}"


def check_cap(row):
    assert row["cap"] in caps()


def check_port(row):
    assert row["port"] == str(_PORT)


facts = {"forbidden_caps": lambda: caps(), "address": address, "routes": lambda: list(_ROUTES)}
cases = {"cap": check_cap, "port": check_port}
"""

FILES = {"lib.py": LIBRARY, "docs/docattest_setup.py": SETUP}

PAGE = """\
Never granted: <!-- fact: forbidden_caps() -->

- `SYS_ADMIN`
- `NET_RAW`

It listens at `/usr/bin/shopd:8080`. <!-- fact: address() -->
"""


class SetupHelpers(unittest.TestCase):
    def test_a_named_fact_reads_what_the_surface_does_not_export(self):
        facts = [fact("address()", id="address"), fact("routes()", id="routes")]
        request = {"id": 3, "method": "evaluate", "params": {"facts": facts}}
        got = Talk(HELLO, opened(FILES, setup="docs/docattest_setup.py"), request).result(3)
        self.assertEqual(got["address"]["value"]["text"], "/usr/bin/shopd:8080")
        self.assertEqual(got["routes"]["value"]["items"], ["health", "orders"])

    def test_a_named_fact_and_a_runner_attest_the_exports_and_constants_they_reach(self):
        attestations = [
            fact("forbidden_caps()", id="caps"),
            fact("address()", id="address"),
            fact("routes()", id="routes"),
            table("cap", ["cap"], [["SYS_ADMIN"]]),
            table("port", ["port"], [["8080"]]),
        ]
        request = {"id": 3, "method": "symbols", "params": {"attestations": attestations}}
        got = Talk(HELLO, opened(FILES, setup="docs/docattest_setup.py"), request).result(3)
        surface = "default"
        self.assertEqual(got["caps"], [{"name": "FORBIDDEN_CAPS", "surface": surface}])
        self.assertEqual(
            got["address"],
            [{"name": "COMMAND_PATH", "surface": surface}, {"name": "_PORT", "surface": surface}],
        )
        # `_ROUTES` is computed, so it is neither an export nor a constant: nothing to attest.
        self.assertEqual(got["routes"], [])
        self.assertEqual(got["cap"], [{"name": "FORBIDDEN_CAPS", "surface": surface}])
        self.assertEqual(got["port"], [{"name": "_PORT", "surface": surface}])


@unittest.skipUnless(USABLE and shutil.which("git"), "needs the docattest core and git")
class SetupHelpersInTheCore(unittest.TestCase):
    def test_where_uncovered_and_impact_count_what_a_named_fact_reaches(self):
        with tempfile.TemporaryDirectory() as temp:
            root = os.path.realpath(temp)
            _commit_project(root)
            where = {
                name: _pages(core("where", "--json", name, root=root)) for name in ("FORBIDDEN_CAPS", "_PORT")
            }
            uncovered = json.loads(core("uncovered", "--json", "--surface=default", root=root)[1])
            _write(root, "lib.py", LIBRARY.replace("8080", "8081").replace('"NET_RAW"', '"NET_ADMIN"'))
            code, out, err = core("impact", "--json", root=root)
        self.assertEqual(where, {"FORBIDDEN_CAPS": [("docs/a.md", 1)], "_PORT": [("docs/a.md", 6)]})
        self.assertEqual(uncovered, ["UNUSED"])
        self.assertEqual(code, 0, err)
        references = [(r["symbol"], r["line"]) for r in json.loads(out)["references"]]
        self.assertEqual(references, [("FORBIDDEN_CAPS", 1), ("_PORT", 6)])


def _pages(answer):
    code, out, err = answer
    assert code == 0, err
    return [(r["page"], r["line"]) for r in json.loads(out)]


def _commit_project(root):
    config = {"pages": ["docs/*.md"], "surfaces": {"default": "./lib.py"}, "setup": "docs/docattest_setup.py"}
    files = dict(
        FILES, **{"docs/a.md": PAGE, "docattest.config.json": json.dumps(dict(config, adapter="python"))}
    )
    for rel, text in files.items():
        _write(root, rel, text)
    for args in (["init", "-q", "-b", "main"], ["add", "-A"], ["commit", "-q", "-m", "base"]):
        subprocess.run(["git", *args], cwd=root, env=ENV, check=True, capture_output=True)


def _write(root, rel, text):
    path = os.path.join(root, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)


if __name__ == "__main__":
    unittest.main()
