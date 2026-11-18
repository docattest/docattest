"""What Python says of itself without a project: the data of ``hello`` and the questions
about text (``assertions``, ``docProse``, ``docComments``, ``mentions``, ``expressions``,
``display``).

Nothing here imports project code, starts a process or reads a file.
"""

from __future__ import annotations

import re
from typing import TYPE_CHECKING, NamedTuple, cast

from . import __version__
from .workers.docattest_worker.values import safe_str

if TYPE_CHECKING:
    # Imported where they are used: ``hello`` is answered before either is loaded.
    import ast
    import doctest

PROTOCOL_VERSION = 1

#: Info-string languages whose fenced blocks the adapter runs. ``pycon`` (and any block
#: whose first statement is a ``>>>`` prompt) is an interpreter session, run by doctest.
BLOCK_LANGUAGES = ["python", "py", "python3", "py3", "pycon"]
SESSION_LANGUAGES = {"pycon"}
SOURCE_FILES = {"include": ["**/*.py"], "exclude": []}
#: Never the project's own, wherever they are: each is a glob for one name.
IGNORED_NAMES = ["__pycache__", "site-packages", "node_modules", "*.egg-info", "*.dist-info"]
#: A virtualenv, whatever it is called: the directory that holds this file.
IGNORED_MARKERS = ["pyvenv.cfg"]
#: Info-string words of this adapter's own; the core passes them on in an example's tags.
CONTINUE = "continue"
INFO_WORDS = [CONTINUE]
CAPABILITIES = ["tests", "observables", "affected", "display"]


class SourceText(NamedTuple):
    """A source file the core asks about, and its text."""

    page: str
    text: str


class Snippet(NamedTuple):
    """An example's code, and the language its block is tagged with."""

    lang: str
    code: str


def hello() -> dict[str, object]:
    return {
        "protocol": PROTOCOL_VERSION,
        "adapter": {"name": "python", "version": __version__},
        "blockLanguages": list(BLOCK_LANGUAGES),
        "sourceFiles": {"include": list(SOURCE_FILES["include"]), "exclude": []},
        "ignoredNames": list(IGNORED_NAMES),
        "ignoredMarkers": list(IGNORED_MARKERS),
        "capabilities": list(CAPABILITIES),
        "infoWords": list(INFO_WORDS),
    }


def is_source(page: str) -> bool:
    return page.endswith(".py")


# --- prose ------------------------------------------------------------------------------

_COMMENT = re.compile(r"<!--.*?-->", re.S)
_SPACE = re.compile(r"\s+")


def prose(text: str) -> str:
    """Prose as the core writes a claim: comments out, whitespace collapsed, markup kept."""
    return _SPACE.sub(" ", _COMMENT.sub(" ", text)).strip()


# --- interpreter sessions ----------------------------------------------------------------

_PROMPT = re.compile(r"^\s*>>> ?", re.M)


def is_session(lang: str, code: str) -> bool:
    """Whether a block is an interpreter session (``>>>`` prompts) rather than a script."""
    if lang.lower() in SESSION_LANGUAGES:
        return True
    for line in code.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        return stripped.startswith(">>>")
    return False


def session_examples(text: str) -> list[doctest.Example] | None:
    """The doctest examples of a session, or None when doctest cannot parse it."""
    import doctest

    try:
        return doctest.DocTestParser().get_examples(text)
    except ValueError:
        return None


def session_source(text: str) -> str:
    """The statements of a session as one script (for symbols and static checks)."""
    examples = session_examples(text) or []
    return "".join(e.source for e in examples)


# --- assertions -------------------------------------------------------------------------

_ASSERT_CALL = re.compile(r"^(assert|fail$|raises$|warns$|deprecated_call$|approx$)", re.I)


def asserts(lang: str, code: str) -> bool:
    """Whether an example asserts anything. One that cannot be read counts as asserting."""
    if is_session(lang, code):
        examples = session_examples(code)
        if examples is None:
            return True
        # An expected output (or an expected traceback) is the assertion of a session.
        if any(e.want.strip() or e.exc_msg is not None for e in examples):
            return True
        code = "".join(e.source for e in examples)
    return _code_asserts(code)


def _code_asserts(code: str) -> bool:
    """Whether code holds an ``assert`` or calls something that asserts (``assertEqual``,
    ``pytest.raises``)."""
    import ast

    try:
        tree = ast.parse(code)
    except (SyntaxError, ValueError):
        return True
    for node in ast.walk(tree):
        if isinstance(node, ast.Assert):
            return True
        if isinstance(node, ast.Call):
            func = node.func
            name: str = func.attr if isinstance(func, ast.Attribute) else getattr(func, "id", "")
            if name and _ASSERT_CALL.match(name):
                return True
    return False


# --- docstrings -------------------------------------------------------------------------

# Where a docstring's description ends: the first prompt, or the first section in the
# Google, NumPy or reStructuredText conventions. These are Python's "tags".
_SECTION = re.compile(
    r"^\s*(?:>>>|:\w[\w ]*:|\.\. \w+::|"
    r"(?:Args|Arguments|Parameters|Params|Returns?|Yields?|Raises|Examples?|Notes?|"
    r"Attributes|See Also|Warnings?|References|Usage|Keyword Args|Keyword Arguments|"
    r"Other Parameters|Methods)\s*:{0,2}\s*$)",
    re.I,
)


class Docstring:
    """One docstring: where it is, what it documents, its cleaned text."""

    __slots__ = ("start", "end", "text", "owner")

    def __init__(self, start: int, end: int, text: str, owner: str | None) -> None:
        self.start = start
        self.end = end
        #: As ``inspect.cleandoc`` gives it; an example's block lines count from its top.
        self.text = text
        self.owner = owner


def description_of(text: str) -> str:
    lines = text.split("\n")
    for k, line in enumerate(lines):
        if _SECTION.match(line):
            # A NumPy section title sits above its underline, which is what matched nothing;
            # a `Title\n-----` pair ends the description at the title.
            lines = lines[:k]
            break
        if k + 1 < len(lines) and re.match(r"^\s*-{3,}\s*$", lines[k + 1]) and line.strip():
            lines = lines[:k]
            break
    return prose(" ".join(lines))


def docstrings(text: str) -> list[Docstring]:
    """Every docstring of a module, in source order: the module's, then each class's,
    function's and method's. A file that does not parse has none."""
    import ast

    try:
        tree = ast.parse(text)
    except (SyntaxError, ValueError):
        return []
    found: list[Docstring] = []

    def visit(node: ast.AST, owner: str | None) -> None:
        docstring = _docstring_of(node, owner)
        if docstring is not None:
            found.append(docstring)
        body: object = getattr(node, "body", None)
        for child in cast("list[ast.AST]", body) if isinstance(body, list) else []:
            if isinstance(child, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
                # A method documents its class as far as a surface is concerned.
                visit(child, owner if owner is not None else child.name)

    visit(tree, None)
    found.sort(key=lambda d: d.start)
    return found


def _docstring_of(node: ast.AST, owner: str | None) -> Docstring | None:
    """The docstring of a module, class or function, or None when it has none."""
    import ast
    import inspect

    if (
        not isinstance(node, (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef))
        or not node.body
    ):
        return None
    first = node.body[0]
    if (
        isinstance(first, ast.Expr)
        and isinstance(first.value, ast.Constant)
        and isinstance(first.value.value, str)
    ):
        end = first.value.end_lineno or first.value.lineno
        return Docstring(first.value.lineno, end, inspect.cleandoc(first.value.value), owner)
    return None


def doc_prose(files: list[SourceText]) -> list[list[dict[str, object]]]:
    out: list[list[dict[str, object]]] = []
    for f in files:
        if not is_source(f.page):
            out.append([])
            continue
        out.append(
            [{"text": description_of(d.text), "start": d.start, "end": d.end} for d in docstrings(f.text)]
        )
    return out


def doc_comments(files: list[SourceText]) -> list[dict[str, object]]:
    """Docstring examples: every docstring with ``>>>`` prompts is one ``check`` example,
    run as doctest runs it. ``# doctest: +SKIP`` and the other doctest directives apply."""
    out: list[dict[str, object]] = []
    for page, text in files:
        if not is_source(page) or ">>>" not in text:
            continue
        count = 0
        for d in docstrings(text):
            if not _PROMPT.search(d.text):
                continue
            examples = session_examples(d.text)
            if not examples:
                continue
            example: dict[str, object] = {
                "id": f"{page}:{d.start}:example:{count}",
                "page": page,
                "line": d.start,
                "kind": "example",
                "claim": description_of(d.text),
                "lang": "pycon",
                "mode": "check",
                "network": False,
                "code": d.text,
                "weak": not asserts("pycon", d.text),
            }
            if d.owner is not None:
                example["owner"] = d.owner
            out.append(example)
            count += 1
    return out


# --- mentions ---------------------------------------------------------------------------

_FILE_SUFFIX = re.compile(r"\.(?:pyi?|pyc|md|rst|txt|toml|cfg|ini|json|ya?ml|html|csv|sh)$", re.I)
_LEADING = re.compile(r"^(?:await\s+|@|not\s+|\*{1,2})?([A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)")


def mentioned(span: str, symbols: set[str]) -> str | None:
    """The one of ``symbols`` an inline code span names, or None.

    A span names a symbol when it starts with it, bare or module-qualified: ``retry``,
    ``retry(fn)``, ``@retry``, ``mypkg.retry()``, ``RETRY_DEFAULTS["attempts"]``.
    ``retry.py`` is a file and ``retry-after`` is something else.
    """
    span = span.strip()
    m = _LEADING.match(span)
    if not m:
        return None
    dotted = m.group(1)
    rest = span[m.end() :]
    if rest[:1] in ("-", "/", "@", "'", '"', "\\") or _FILE_SUFFIX.search(dotted):
        return None
    for part in dotted.split("."):
        if part in symbols:
            return part
    return None


def mentions(spans: list[str], symbols: list[str]) -> list[str | None]:
    wanted = set(symbols)
    return [mentioned(s, wanted) for s in spans]


# --- expressions ------------------------------------------------------------------------

_TOKEN = re.compile(
    r"""[A-Za-z_]\w*|\d[\w.]*|'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|\*\*|//|[<>=!]=|->|:=|<<|>>|\S"""
)
_NAME = re.compile(r"^[A-Za-z_]\w*$")
_KEYWORDS = frozenset(
    "False None True and as assert async await break class continue def del elif else except "
    "finally for from global if import in is lambda nonlocal not or pass raise return try "
    "while with yield".split()
)
_STRING_PREFIX = re.compile(r"^(?:[rRbBuUfF]{1,2})$")


def expression_shape(expression: str) -> dict[str, object]:
    """A fact expression with every name replaced by a placeholder, and its names."""
    tokens: list[str] = _TOKEN.findall(expression)
    shape: list[str] = []
    names: list[str] = []
    for k, tok in enumerate(tokens):
        nxt = tokens[k + 1] if k + 1 < len(tokens) else ""
        is_prefix = _STRING_PREFIX.match(tok) and nxt[:1] in ("'", '"')
        if _NAME.match(tok) and tok not in _KEYWORDS and not is_prefix:
            shape.append("\u00b7")
            names.append(tok)
        else:
            shape.append(tok)
    return {"shape": " ".join(shape), "names": names}


def expressions(items: list[str]) -> list[dict[str, object]]:
    return [expression_shape(e) for e in items]


def assertions(examples: list[Snippet]) -> list[bool]:
    return [asserts(e.lang, e.code) for e in examples]


def display(values: list[object]) -> list[str]:
    """Values of a data block, as ``json`` reads them (``1`` an int, ``1.0`` a float, ``null``
    None), each shown as a fact's value is: ``True``, ``None``, ``1.0``."""
    return [safe_str(value) for value in values]
