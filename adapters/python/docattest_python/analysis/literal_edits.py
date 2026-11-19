"""The edits that change the literals of a declaration, each alone: what ``mutants``
answers for a symbol (PROTOCOL.md).

The literals are those of the values the declaration assigns its name, in source order and
at any depth, but not inside a ``lambda`` (it runs only when called) or a string that fills
in values (an f-string). A number gains one, written as Python writes a number of its type;
a string or bytes gains ``x`` before its closing quote, prefix, quotes and escapes kept, each
part of an implicit concatenation on its own; a bool is flipped; a list, tuple or set offers
its last item removed before the edits of its items. ``None`` and ``...`` are left alone.
"""

from __future__ import annotations

import ast
import math
import tokenize
from typing import NamedTuple

from .modules import Declaration
from .source_map import SourceMap, Token

#: The nodes whose literals are not the value's own: a function, which runs when called, and
#: a string that fills in values, whose parts are not literals of their own. By name, since
#: ``TemplateStr`` is newer than the oldest Python supported.
_NOT_LITERALS = frozenset({"Lambda", "JoinedStr", "TemplateStr"})
_TRIPLE_QUOTES = ('"""', "'''")
_OPENERS = "([{"
_CLOSERS = ")]}"


class LiteralEdit(NamedTuple):
    """One change to a file: the bytes from ``start`` to ``end`` replaced by
    ``replacement``, and what that changes, as Python writes it."""

    start: int
    end: int
    replacement: str
    description: str


def literal_edits(text: str, declaration: Declaration) -> list[LiteralEdit]:
    """Every edit of the literals of an ``assign`` declaration of a module whose text is
    ``text``, in source order."""
    source = SourceMap(text)
    keyed: list[tuple[int, LiteralEdit]] = []
    for value in _assigned_values(declaration):
        _visit(value, source, keyed)
    # Stable: a display's removal, keyed by where the display starts, stays before the
    # edits of its first item.
    keyed.sort(key=lambda pair: pair[0])
    return [edit for _, edit in keyed]


def _assigned_values(declaration: Declaration) -> list[ast.expr]:
    """The values the declaration's statements give its name: of ``A, B = 1, 2``, the ``1``
    that is ``A``'s."""
    values: list[ast.expr] = []
    for node in declaration.nodes:
        if isinstance(node, ast.Assign):
            targets = node.targets
        elif isinstance(node, (ast.AnnAssign, ast.AugAssign)):
            targets = [node.target]
        else:
            continue
        if node.value is None:
            continue
        for target in targets:
            value = _value_of(target, node.value, declaration.name)
            if value is not None:
                values.append(value)
    return values


def _value_of(target: ast.expr, value: ast.expr, name: str) -> ast.expr | None:
    """The part of ``value`` an assignment to ``target`` gives ``name``: all of it, unless
    the target unpacks a display item by item."""
    if isinstance(target, ast.Name):
        return value if target.id == name else None
    if not any(isinstance(n, ast.Name) and n.id == name for n in ast.walk(target)):
        return None
    if not _pairs(target, value):
        return value
    for target_item, value_item in zip(_elements(target), _elements(value)):
        part = _value_of(target_item, value_item, name)
        if part is not None:
            return part
    return None


def _pairs(target: ast.expr, value: ast.expr) -> bool:
    """Whether ``target`` takes the items of the display ``value`` one by one."""
    displays = (ast.Tuple, ast.List)
    if not isinstance(target, displays) or not isinstance(value, displays):
        return False
    items = target.elts + value.elts
    return len(target.elts) == len(value.elts) and not any(isinstance(i, ast.Starred) for i in items)


def _elements(node: ast.expr) -> list[ast.expr]:
    return node.elts if isinstance(node, (ast.Tuple, ast.List)) else []


def _visit(node: ast.AST, source: SourceMap, keyed: list[tuple[int, LiteralEdit]]) -> None:
    if type(node).__name__ in _NOT_LITERALS:
        return
    if isinstance(node, ast.Constant):
        keyed.extend((edit.start, edit) for edit in _constant_edits(node, source))
        return
    if isinstance(node, (ast.List, ast.Tuple, ast.Set)):
        removal = _last_item_removed(node, source)
        if removal is not None:
            keyed.append((source.start(node), removal))
    for child in ast.iter_child_nodes(node):
        _visit(child, source, keyed)


def _constant_edits(node: ast.Constant, source: SourceMap) -> list[LiteralEdit]:
    value: object = node.value
    if isinstance(value, bool):
        return [_replaced(node, source, repr(not value))]
    if isinstance(value, (str, bytes)):
        return [_quoted_x(t) for t in source.tokens_of(node) if t.kind == tokenize.STRING]
    if isinstance(value, (int, float, complex)):
        bumped = _plus_one(value)
        return [_replaced(node, source, bumped)] if bumped is not None else []
    return []


def _replaced(node: ast.expr, source: SourceMap, replacement: str) -> LiteralEdit:
    written = source.written(node)
    return LiteralEdit(source.start(node), source.end(node), replacement, f"{written} → {replacement}")


def _plus_one(value: int | float | complex) -> str | None:
    """A number plus one, as Python writes a literal of its type; None when that is no
    literal (infinity) or no change (a float too large to gain one)."""
    if isinstance(value, int):
        return str(value + 1)
    if isinstance(value, float):
        bumped = value + 1
        return repr(bumped) if math.isfinite(bumped) and bumped != value else None
    # An imaginary literal (``2j``): its imaginary part gains one.
    bumped_imaginary = value + 1j
    finite = math.isfinite(bumped_imaginary.imag) and bumped_imaginary != value
    return repr(bumped_imaginary) if finite else None


def _quoted_x(token: Token) -> LiteralEdit:
    """A string or bytes literal with ``x`` before its closing quote."""
    body = token.text.lstrip("rRbBuU")
    quote = body[:3] if body.startswith(_TRIPLE_QUOTES) else body[0]
    changed = token.text[: -len(quote)] + "x" + quote
    return LiteralEdit(token.start, token.end, changed, f"{token.text} → {changed}")


def _last_item_removed(display: ast.List | ast.Tuple | ast.Set, source: SourceMap) -> LiteralEdit | None:
    """The display without its last item, still valid code; None when it has no items."""
    if not display.elts:
        return None
    description = f"{source.written(display.elts[-1])} removed"
    tokens = source.tokens_of(display)
    enclosed = _enclosed(tokens)
    if len(display.elts) == 1:
        return _emptied(display, tokens, enclosed, source, description)
    commas = _separating_commas(tokens, 1 if enclosed else 0)
    count = len(display.elts)
    # The comma before the last item is kept, so that a pair stays a tuple: ``(1, 2)`` →
    # ``(1,)``. A trailing comma goes with the item.
    start = commas[count - 2].end
    if len(commas) == count:
        end = commas[-1].end
    else:
        end = tokens[-1].start if enclosed else source.end(display)
    return LiteralEdit(start, end, "", description)


def _emptied(
    display: ast.List | ast.Tuple | ast.Set,
    tokens: list[Token],
    enclosed: bool,
    source: SourceMap,
    description: str,
) -> LiteralEdit:
    """A display of one item, emptied: what lies between its brackets removed, but ``{}``
    is a dict and a tuple without brackets would be no expression."""
    if isinstance(display, ast.Set):
        return LiteralEdit(source.start(display), source.end(display), "set()", description)
    if not enclosed:
        return LiteralEdit(source.start(display), source.end(display), "()", description)
    return LiteralEdit(tokens[0].end, tokens[-1].start, "", description)


def _enclosed(tokens: list[Token]) -> bool:
    """Whether the tokens are one bracketed whole: ``(1, 2)``, but not ``(1), 2``."""
    if not tokens or _depth_change(tokens[0]) != 1:
        return False
    depth = 0
    for k, token in enumerate(tokens):
        depth += _depth_change(token)
        if depth == 0:
            return k == len(tokens) - 1
    return False


def _separating_commas(tokens: list[Token], depth_of_items: int) -> list[Token]:
    """The commas between a display's items (a trailing one too), not those inside them."""
    commas: list[Token] = []
    depth = 0
    for token in tokens:
        depth += _depth_change(token)
        if token.kind == tokenize.OP and token.text == "," and depth == depth_of_items:
            commas.append(token)
    return commas


def _depth_change(token: Token) -> int:
    if token.kind != tokenize.OP:
        return 0
    if token.text in _OPENERS:
        return 1
    return -1 if token.text in _CLOSERS else 0
