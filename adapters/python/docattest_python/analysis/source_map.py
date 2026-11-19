"""Where a module's nodes and tokens are, as offsets in bytes of its UTF-8 text: the unit an
edit of the file is written in (PROTOCOL.md, ``mutants``)."""

from __future__ import annotations

import ast
import bisect
import io
import tokenize
from typing import NamedTuple


class Token(NamedTuple):
    """One token of the text: its type (``tokenize.STRING``, ``tokenize.OP``), as written,
    and where it starts and ends."""

    kind: int
    text: str
    start: int
    end: int


class SourceMap:
    """The byte offsets of one module's text. The text must parse: its tokens are read when
    the map is made."""

    def __init__(self, text: str) -> None:
        self._encoded = text.encode("utf-8")
        # Split as ``ast`` and ``tokenize`` count lines: at each ``\n``, a ``\r`` before it
        # kept on its line.
        self._lines = io.StringIO(text, newline="\n").readlines()
        self._line_starts = [0]
        for line in self._lines:
            self._line_starts.append(self._line_starts[-1] + len(line.encode("utf-8")))
        self.tokens = [self._token(t) for t in tokenize.generate_tokens(io.StringIO(text).readline)]
        self._token_starts = [t.start for t in self.tokens]

    def start(self, node: ast.expr) -> int:
        # ``ast`` already counts a column in UTF-8 bytes.
        return self._line_starts[node.lineno - 1] + node.col_offset

    def end(self, node: ast.expr) -> int:
        assert node.end_lineno is not None and node.end_col_offset is not None
        return self._line_starts[node.end_lineno - 1] + node.end_col_offset

    def written(self, node: ast.expr) -> str:
        """A node's text as the file writes it."""
        return self._encoded[self.start(node) : self.end(node)].decode("utf-8")

    def tokens_of(self, node: ast.expr) -> list[Token]:
        """The tokens that lie within a node, in order."""
        first = bisect.bisect_left(self._token_starts, self.start(node))
        last = bisect.bisect_left(self._token_starts, self.end(node))
        return self.tokens[first:last]

    def _token(self, token: tokenize.TokenInfo) -> Token:
        return Token(token.type, token.string, self._offset(*token.start), self._offset(*token.end))

    def _offset(self, row: int, column: int) -> int:
        """A ``tokenize`` position, a column counted in characters, as a byte offset."""
        if row > len(self._lines):
            return len(self._encoded)
        return self._line_starts[row - 1] + len(self._lines[row - 1][:column].encode("utf-8"))
