"""``docattest.cells.code_spans``: the code spans of a table cell, as the core reads them,
held to the table every language's helper shares and to the core itself.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, PACKAGE_ROOT)

from test_core import USABLE, core  # noqa: E402

from docattest.cells import code_spans  # noqa: E402

#: The table every language's cell helper and the core are tested against.
SHARED_CASES = os.path.join(PACKAGE_ROOT, "..", "..", "protocol", "test-data", "code-spans.json")

# Cells as a page writes them, each with at least one code span: a cell with none is
# compared by its text, which code_spans does not give.
CELLS_AS_WRITTEN = [
    "`FOO=1` `BAR=2`",
    "`print(1)`",
    "``a`b``",
    "`` `x` ``",
    "```a``b```",
    "`  padded  `",
    "`  `x",
    "text `a` more `b` end",
    "`a\\|b`",
    "\\`x` `y`",
    "\\\\`x`",
    "`x\\`y`",
    "``a` `b`",
    "`a`` `b`",
    "\t` tab and no-break ` ",
    "`﻿byte order mark\u0085`",
    "`\u001ffile separator\u001f`",
    "`a` <!-- `b` --> `c`",
    '<span title="`">`x`</span>',
    "<https://example.com/`a`> `b`",
    "<a@b.c> `d`",
    "a < b `c` > d",
    "**`bold`** _`em`_",
    "[`a`](b) `c`",
    "[a](`b`) `c`",
    '[a](u "`t`") `c`',
    "[a](u '`t`') `c`",
    "[a](u (`t`)) `c`",
    "[a](<`b` c>) `d`",
    "[a](b(`c`)) `d`",
    "[a] (`b`)",
    "a](`b`)",
    "\\[a](`b`)",
    "[[a](b)](`c`) `z`",
    "![[a](b)](`c`) `z`",
    "[![a](b)](`c`) `z`",
    "[a [b] c](`d`) `z`",
    "[a][`b`]",
    "[a`](b)` `c`",
    "[a](b `c`) `d`",
    "<!-->`a`",
    "<?x `a` ?> `b`",
    "<!X `a`> `b`",
    "<![CDATA[`a`]]> `b`",
    "</a `b`> `c`",
]


class CodeSpans(unittest.TestCase):
    def test_what_a_runner_reads_of_a_cell(self):
        for name, cell, spans in [
            ("spans in order", "`FOO=1` `BAR=2`", ["FOO=1", "BAR=2"]),
            ("a longer run holds a backtick", "``a`b`` or `` `c` ``", ["a`b", "`c`"]),
            ("no code", "no code", []),
        ]:
            with self.subTest(name):
                self.assertEqual(code_spans(cell), spans)

    def test_code_spans(self):
        with open(SHARED_CASES, encoding="utf-8") as handle:
            cases = json.load(handle)["cases"]
        self.assertTrue(cases)
        for case in cases:
            with self.subTest(case["name"]):
                self.assertEqual(code_spans(case["cell"]), case["spans"])

    @unittest.skipUnless(USABLE, "no docattest core binary (set DOCATTEST_CORE)")
    def test_code_spans_are_what_a_column_fact_compares(self):
        """Each cell is the one row of a table whose column fact lists what code_spans gives
        for the cell as a runner gets it, so a fact that fails is a cell the two read
        differently."""
        spans = [code_spans(cell.replace("\\|", "|")) for cell in CELLS_AS_WRITTEN]
        page = "".join(
            f"<!-- fact column=cell: {found!r} -->\n| cell |\n|---|\n| {cell} |\n\n"
            for cell, found in zip(CELLS_AS_WRITTEN, spans)
        )
        files = {
            "cells.md": page,
            "lib.py": "",
            "docattest.config.json": json.dumps(
                {"pages": ["cells.md"], "surfaces": {"default": "./lib.py"}, "adapter": "python"}
            ),
        }
        with tempfile.TemporaryDirectory() as root:
            for name, text in files.items():
                with open(os.path.join(root, name), "w", encoding="utf-8") as handle:
                    handle.write(text)
            _, out, err = core("check", "--json", root=root)
        results = json.loads(out)["results"] if out else []
        self.assertEqual(len(results), len(CELLS_AS_WRITTEN), err)
        for cell, found, result in zip(CELLS_AS_WRITTEN, spans, results):
            with self.subTest(cell):
                self.assertEqual(result["status"], "pass", f"code_spans gives {found!r}: {result['message']}")


if __name__ == "__main__":
    unittest.main()
