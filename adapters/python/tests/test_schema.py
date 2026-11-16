"""The package's copy of the protocol's schemas, agent descriptors and pinned core version is the
neutral source's, byte for byte; the validator the other tests lean on can say no; and the
conformance suite the core carries holds for this adapter."""

import os
import subprocess
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import schema  # noqa: E402
from test_core import CORE, USABLE  # noqa: E402

SCHEMAS = schema.Validator.load()

SOURCE = os.path.normpath(os.path.join(HERE, "..", "..", "..", "protocol"))


def tree(root: str) -> dict:
    out = {}
    copied = [
        os.path.join(d, f)
        for d in ("schema", "agents")
        for f in sorted(os.listdir(os.path.join(root, d)))
        if f.endswith(".json")
    ]
    for name in ["core.json", "platforms.json"] + copied:
        with open(os.path.join(root, name), "rb") as handle:
            out[name] = handle.read()
    return out


class Copy(unittest.TestCase):
    @unittest.skipUnless(os.path.isdir(SOURCE), "not in a checkout of the repository")
    def test_the_copy_is_the_source(self):
        self.assertEqual(tree(schema.PACKAGE), tree(SOURCE), "run: python3 adapters/python/sync_protocol.py")


class Validator(unittest.TestCase):
    def test_it_refuses_what_the_schemas_refuse(self):
        observation = {"text": "a", "description": "str 'a'", "kind": "text"}
        self.assertEqual(SCHEMAS.check(observation, "Observation", "results.json"), [])
        for wrong in (
            {"text": "a"},
            {**observation, "kind": "string"},
            {**observation, "items": [1]},
            {**observation, "more": 1},
        ):
            self.assertTrue(SCHEMAS.check(wrong, "Observation", "results.json"), wrong)
        listing = {"ok": True, "tests": [], "suites": [{"title": "T", "describes": [None], "line": 1}]}
        self.assertTrue(SCHEMAS.check(listing, "tests.result"))
        listing["suites"][0]["kind"] = "a class"
        self.assertEqual(SCHEMAS.check(listing, "tests.result"), [])
        self.assertTrue(SCHEMAS.check({"id": 0, "method": "x"}, "Request", "messages.json"))
        self.assertTrue(SCHEMAS.check({"id": 1, "result": {}, "error": {}}, "Response", "messages.json"))


KIT = os.path.join(os.path.dirname(HERE), "conformance.json")


@unittest.skipUnless(USABLE, "no docattest core binary (set DOCATTEST_CORE)")
class Conformance(unittest.TestCase):
    """The protocol's conformance suite (PROTOCOL.md, section 5), which the core carries."""

    def test_the_adapter_conforms(self):
        adapter = [sys.executable, os.path.join(os.path.dirname(HERE), "docattest_python", "__main__.py")]
        done = subprocess.run(
            [CORE, "conformance", "run", KIT, "--", *adapter],
            capture_output=True,
            text=True,
            timeout=300,
        )
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertRegex(done.stdout, r"conformance \(python\): (\d+) of \1 checks hold")


if __name__ == "__main__":
    unittest.main()
