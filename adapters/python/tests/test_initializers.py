"""What a proof that reads a value attests of the names the value's initializer uses: a fact,
a named fact and a cases runner attest them, at any depth, private constants too, but not
what a function's body reads, as ``symbols``, ``where``, ``uncovered`` and ``impact`` show.

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

# ENV is built from COMMAND_PATH, itself built from BIN, and from a constant the module keeps
# private. `total` reads LIMIT in its body, which calling it does not attest.
LIBRARY = """\
_STDERR_OFFSET = 2
BIN = "/bin"
COMMAND_PATH = BIN + "/run"
ENV = [f"PATH={COMMAND_PATH}", f"STDERR={_STDERR_OFFSET}"]
LIMIT = 3
total = lambda: LIMIT  # noqa: E731
"""

SETUP = """\
from lib import ENV


def check_env(row):
    assert row["line"] in ENV


facts = {"env": lambda: ENV}
cases = {"env": check_env}
"""

FILES = {"lib.py": LIBRARY, "docs/docattest_setup.py": SETUP}

PAGE = """\
The command runs with: <!-- fact: ENV -->

- `PATH=/bin/run`
- `STDERR=2`

At most `3`. <!-- fact: total() -->
"""

#: What reading ENV attests, by name.
BUILT_FROM = ["BIN", "COMMAND_PATH", "ENV", "_STDERR_OFFSET"]


class Initializers(unittest.TestCase):
    def test_a_proof_that_reads_a_value_attests_the_names_its_initializer_uses(self):
        attestations = [
            fact("ENV", id="fact"),
            fact("env()", id="named"),
            table("env", ["line"], [["PATH=/bin/run"]]),
            fact("total()", id="total"),
        ]
        request = {"id": 3, "method": "symbols", "params": {"attestations": attestations}}
        got = Talk(HELLO, opened(FILES, setup="docs/docattest_setup.py"), request).result(3)
        self.assertEqual(
            {key: _names(refs) for key, refs in got.items()},
            {"fact": BUILT_FROM, "named": BUILT_FROM, "env": BUILT_FROM, "total": ["total"]},
        )


@unittest.skipUnless(USABLE and shutil.which("git"), "needs the docattest core and git")
class InitializersInTheCore(unittest.TestCase):
    def test_where_uncovered_and_impact_count_the_names_an_initializer_uses(self):
        with tempfile.TemporaryDirectory() as temp:
            root = os.path.realpath(temp)
            _commit_project(root)
            checked = core("check", root=root)
            where = {
                name: _lines(core("where", "--json", name, root=root)) for name in [*BUILT_FROM, "LIMIT"]
            }
            uncovered = json.loads(core("uncovered", "--json", "--surface=default", root=root)[1])
            _write(root, "lib.py", LIBRARY.replace('"/bin"', '"/opt"'))
            code, out, err = core("impact", "--json", root=root)
        self.assertEqual(checked[0], 0, checked[1] + checked[2])
        self.assertEqual(where, {**{name: [1] for name in BUILT_FROM}, "LIMIT": []})
        self.assertEqual(uncovered, ["LIMIT"])
        self.assertEqual(code, 0, err)
        references = [(r["symbol"], r["line"]) for r in json.loads(out)["references"]]
        self.assertEqual(references, [("BIN", 1), ("COMMAND_PATH", 1), ("ENV", 1)])


def _names(refs):
    assert all(r["surface"] == "default" for r in refs), refs
    return sorted(r["name"] for r in refs)


def _lines(answer):
    code, out, err = answer
    assert code == 0, err
    return [r["line"] for r in json.loads(out)]


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
