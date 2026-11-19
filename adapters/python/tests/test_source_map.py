"""Byte offsets of a module's nodes and tokens, which an edit of the file is written in.

python3 -m unittest discover -s adapters/python/tests -v
"""

import ast
import os
import sys
import tokenize
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

from docattest_python.analysis.source_map import SourceMap  # noqa: E402

TEXT = '# "€"\r\nPAIR = ("é", [1,\n  2])\n'


class Offsets(unittest.TestCase):
    def test_nodes_and_tokens_are_where_their_bytes_are(self):
        source = SourceMap(TEXT)
        encoded = TEXT.encode("utf-8")
        statement = ast.parse(TEXT).body[0]
        assert isinstance(statement, ast.Assign) and isinstance(statement.value, ast.Tuple)
        pair = statement.value
        self.assertEqual(encoded[source.start(pair) : source.end(pair)].decode(), '("é", [1,\n  2])')
        self.assertEqual(source.written(pair.elts[1]), "[1,\n  2]")
        tokens = source.tokens_of(pair)
        self.assertEqual([t.text for t in tokens if t.kind == tokenize.NUMBER], ["1", "2"])
        for token in tokens:
            self.assertEqual(encoded[token.start : token.end].decode(), token.text)


if __name__ == "__main__":
    unittest.main()
