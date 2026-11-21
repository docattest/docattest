"""``mutants`` over the protocol: which declaration each symbol names, the edits of its
literals as a file's byte offsets, the limit, and why a symbol has none.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from talk import HELLO, Talk, opened  # noqa: E402

PACKAGE = {
    "shop/__init__.py": "from .settings import ALIAS, PORT, STARTED, Shop, open_shop\nfrom os import sep\n",
    "shop/settings.py": (
        "import os\n"
        "\n"
        "# Prices in €, which takes three bytes.\n"
        "PORT = 8080\n"
        "STARTED = os.getpid()\n"
        "ALIAS = PORT\n"
        "\n"
        "\n"
        "class Shop:\n"
        "    pass\n"
        "\n"
        "\n"
        "def open_shop():\n"
        "    return 1\n"
    ),
}
SURFACES = [{"name": "default", "entry": "./shop/__init__.py"}]


def mutants(symbols, limit=8, files=PACKAGE):
    request = {"id": 3, "method": "mutants", "params": {"symbols": symbols, "limit": limit}}
    return Talk(HELLO, opened(files, surfaces=SURFACES), request).result(3)


def applied(edit, files=PACKAGE):
    """The file an edit changes, with the edit made."""
    encoded = files[edit["path"]].encode("utf-8")
    return (encoded[: edit["start"]] + edit["replacement"].encode("utf-8") + encoded[edit["end"] :]).decode()


class Mutants(unittest.TestCase):
    def test_a_surface_symbol_is_changed_where_its_re_export_is_declared(self):
        [answer] = mutants([{"name": "PORT", "surface": "default"}])
        [edit] = answer["edits"]
        self.assertEqual(edit["path"], "shop/settings.py")
        self.assertEqual(edit["description"], "8080 → 8081")
        self.assertIn("\nPORT = 8081\n", applied(edit))
        self.assertNotIn("why", answer)

    def test_a_surface_reads_its_unexported_constants_from_source(self):
        files = {"lib.py": 'from shop.settings import PORT\n_LEVEL = "debug"\n', **PACKAGE}
        request = {"symbols": [{"name": "_LEVEL", "surface": "default"}], "limit": 8}
        talk = Talk(HELLO, opened(files), {"id": 3, "method": "mutants", "params": request})
        [edit] = talk.result(3)[0]["edits"]
        self.assertEqual((edit["path"], edit["description"]), ("lib.py", '"debug" → "debugx"'))

    def test_a_symbol_without_a_surface_is_a_constant_a_surface_declares(self):
        files = {**PACKAGE, "shop/__init__.py": 'from .settings import PORT\n_REGION = "eu"\n'}
        answers = mutants([{"name": "_REGION"}, {"name": "STARTED"}], files=files)
        [edit] = answers[0]["edits"]
        self.assertEqual(edit["description"], '"eu" → "eux"')
        self.assertIn('\n_REGION = "eux"\n', applied(edit, files))
        # Declared in a module no surface is, it is no constant a fact reads.
        self.assertEqual(answers[1], {"edits": [], "why": "notFound"})

    def test_at_most_limit_edits_the_first_in_source_order(self):
        files = {"shop/__init__.py": 'KEYS = ["A", "B", "C"]\n'}
        [answer] = mutants([{"name": "KEYS", "surface": "default"}], limit=2, files=files)
        self.assertEqual([e["description"] for e in answer["edits"]], ['"C" removed', '"A" → "Ax"'])

    def test_a_non_literal_alias_mutates_as_an_expression(self):
        [answer] = mutants([{"name": "ALIAS", "surface": "default"}])
        [edit] = answer["edits"]
        self.assertEqual(edit["description"], "PORT → 2 * PORT")
        self.assertIn("\nALIAS = 2 * PORT\n", applied(edit))

    def test_a_symbol_without_edits_says_why(self):
        names = ["Shop", "open_shop", "STARTED", "sep", "nothing_declares_this"]
        answers = mutants([{"name": n, "surface": "default"} for n in names] + [{"name": "Shop"}])
        self.assertEqual(
            answers,
            [
                {"edits": [], "why": "function"},
                {"edits": [], "why": "function"},
                {"edits": [], "why": "noLiteral"},
                {"edits": [], "why": "notFound"},
                {"edits": [], "why": "notFound"},
                {"edits": [], "why": "function"},
            ],
        )

    def test_a_declaration_in_a_directory_never_the_projects_own_is_not_found(self):
        files = {
            "lib.py": "from node_modules.vendored import RETRIES\n",
            "node_modules/vendored.py": "RETRIES = 3\n",
        }
        request = {"symbols": [{"name": "RETRIES", "surface": "default"}], "limit": 8}
        talk = Talk(HELLO, opened(files), {"id": 3, "method": "mutants", "params": request})
        self.assertEqual(talk.result(3), [{"edits": [], "why": "notFound"}])


if __name__ == "__main__":
    unittest.main()
