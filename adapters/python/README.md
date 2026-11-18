# docattest for Python

[docattest](../../README.md) for Python projects: documentation that CI can fail. Your
Markdown pages carry examples that run, facts that are compared with values in your code,
tables whose rows are tests, and citations of tests you already have.

Try it in your browser first: the [playground](https://docattest.github.io/docattest/playground.html)
lets you break a small project's code and watch docattest catch its docs.

This package is what a Python user installs. It provides two commands: <!-- fact: list(manifest("pyproject.toml")["project"]["scripts"]) -->
- **`docattest`**, the command you run. It is a launcher: it finds the docattest core (one
  binary for every language, which reads the pages and writes the report), tells it how to
  start the Python adapter and what command runs docattest in your project, and becomes it.
- **`docattest-adapter-python`**, the adapter: the program that answers everything that
  needs Python. It speaks version 1 <!-- fact: PROTOCOL_VERSION --> of [the adapter protocol](../../protocol/PROTOCOL.md). You do
  not run it yourself.

The distribution is called `docattest` <!-- fact: manifest("pyproject.toml")["project"]["name"] -->.
Its import packages are `docattest` (the launcher) and `docattest_python` (the adapter). <!-- fact: [p for p in manifest("pyproject.toml")["tool"]["setuptools"]["packages"] if "." not in p] -->

Python 3.10 or newer. <!-- fact: manifest("pyproject.toml")["project"]["requires-python"].removeprefix(">=") --> Standard library only: no dependencies.

## Install

```sh
pip install docattest                 # the commands docattest and docattest-adapter-python
```

The core comes inside the wheel: there is one wheel for each of 5 <!-- fact: len(platforms()) -->
platforms, `py3-none-<platform>` (the Python code is pure; only the core is native), and pip
picks the one built for your machine. Nothing is downloaded when docattest runs, and Node is
never needed.

<!-- cases: wheel_tags -->
| platform | name | wheel tags |
|---|---|---|
| macOS 11+ on Apple silicon | `darwin-arm64` | `macosx_11_0_arm64` |
| macOS 10.12+ on Intel | `darwin-x64` | `macosx_10_12_x86_64` |
| Linux x86-64, glibc or musl | `linux-x64` | `manylinux_2_17_x86_64`, `manylinux2014_x86_64`, `musllinux_1_1_x86_64` |
| Linux arm64, glibc or musl | `linux-arm64` | `manylinux_2_17_aarch64`, `manylinux2014_aarch64`, `musllinux_1_1_aarch64` |
| Windows x64 | `windows-x64` | `win_amd64` |

The name is the platform's in every docattest package: the npm package and the Go archive
that carry its core are called by it too.

The Linux core is a static binary, so one wheel serves glibc and musl (Alpine) alike. There
is no source distribution, because it could not build the core: on any other platform
`pip install docattest` finds nothing to install, rather than installing a `docattest`
that cannot run. An install from a checkout (`pip install -e adapters/python`, for
development) has no core either; its first run says so, names your platform, and says to
install the wheel or set `DOCATTEST_CORE`.

`python3 adapters/python/build_wheel.py --out <dir> --core <platform>=<binary>` builds the
wheels from cores built for each platform (the release workflow runs it; see
[CONTRIBUTING.md](../../CONTRIBUTING.md), "Releasing"). It needs nothing but the standard
library and pip, which fetches setuptools for the build.

The launcher looks for the core in three places, in this order, the same as the npm
package does:
<!-- test: tests/test_launcher.py > "Lookup" > "test_the_core_is_the_one_given_then_the_bundled_one_then_the_cached_one" -->

1. `DOCATTEST_CORE` <!-- fact @launcher: CORE_VARIABLE -->, the path of a core binary. It
   is asked for its version (`--version --json`) and refused when it speaks another
   protocol than this package, or does not answer. <!-- test: tests/test_launcher.py > "Lookup" > "test_a_core_that_speaks_another_protocol_is_refused" -->

   ```console check
   $ DOCATTEST_CORE=/bin/echo python3 -m docattest check
   docattest: DOCATTEST_CORE (/bin/echo) is not a docattest core: it does not answer --version --json
   [exit 1]
   ```
2. `docattest/bin/docattest` inside this package (`docattest.exe` on Windows): where the
   wheel for your platform carries it.
3. `<cache>/core/<version>/<name>/docattest`, a cache shared with the npm package, where
   `<name>` is the platform's name from the table above. The version is the one in
   `docattest_python/protocol/core.json`, which this package pins. `<cache>` is:

<!-- cases: cache -->
| on | set | cache |
|---|---|---|
| any system | `DOCATTEST_CACHE=/mine` | `/mine` |
| macOS | | `~/Library/Caches/docattest` |
| Linux | `XDG_CACHE_HOME=/x` | `/x/docattest` |
| Linux | `XDG_CACHE_HOME=relative` | `~/.cache/docattest` |
| Linux | | `~/.cache/docattest` |
| Windows | `LOCALAPPDATA=C:\Local` | `C:\Local\docattest` |
| Windows | | `~\AppData\Local\docattest` |

`XDG_CACHE_HOME` counts only when it is an absolute path, as the XDG specification says.

`DOCATTEST_TRACE=1` makes the launcher say which core it runs.
<!-- test: tests/test_launcher.py > "Launch" > "test_a_trace_names_the_core_it_runs" -->

From a checkout of the repository, without installing anything:

```sh
cargo build --release --manifest-path core/Cargo.toml
export DOCATTEST_CORE="$PWD/core/target/release/docattest"
export PYTHONPATH="$PWD/adapters/python"
python3 -m docattest check --root adapters/python/tests/fixture
```

`docattest hooks install`, `hooks install --git` and `agents install` write down a command
that runs docattest without Node: the virtualenv's own script when the virtualenv is inside
the project, else `docattest` when that is the script the `PATH` finds, else the script by
its full path, else this interpreter by its full path (`python -m docattest`). <!-- test: tests/test_launcher.py > "Launch" > "test_every_command_that_writes_the_command_down_is_told_it" -->
The launcher knows no coding agent. For a virtualenv inside the project it says
`{root}/.venv/bin/docattest`, and the core writes the root as the reader of the command
names it: `"$CLAUDE_PROJECT_DIR"/.venv/bin/docattest impact --hook` in a hook of Claude
Code (its descriptor's `projectRoot`), `./.venv/bin/docattest` in `AGENTS.md` and in a git
hook. <!-- test: tests/test_launcher.py > "Launch" > "test_hooks_are_told_a_command_that_runs_docattest_without_node" -->
Set `DOCATTEST_COMMAND` <!-- fact @launcher: COMMAND_VARIABLE --> to have it write something
else (`uv run docattest`).
<!-- test: tests/test_launcher.py > "Launch" > "test_what_is_already_set_stands" -->

Any agent, or none, can use the two plain commands, from anywhere inside the project:
`docattest impact --file <path>` after an edit and `docattest check` before finishing.
`docattest hooks install --agent gemini` (or `cursor`, or a descriptor file of your own)
installs hooks for another agent; [docs/agents.md](../../docs/agents.md) has the details.

A bare core, run without the launcher, finds the adapter as `docattest-adapter-python` on
the `PATH`, or through `DOCATTEST_ADAPTER_PYTHON` <!-- fact @launcher: ADAPTER_VARIABLE -->: a JSON array
(`["python3", "/path/to/adapters/python/bin/docattest-adapter-python"]`) or a command line
split on spaces.

## Configure

`docattest.config.json` <!-- fact @launcher: CONFIG_FILE --> at the project root:

```json
{
  "pages": ["README.md", "docs/**/*.md"],
  "sources": ["src/**/*.py"],
  "surfaces": { "default": "shoplib" },
  "setup": "docs/docattest_setup.py",
  "adapter": "python"
}
```

- **`surfaces`** name what pages are allowed to observe. An entry is
  - a module name, `"shoplib"` or `"shoplib.shipping"`: imported as written. It is looked
    for in the project root, then in `src/`, then wherever `import` finds it;
  - a path, `"./src/shoplib"` or `"./shoplib/shipping.py"`: that file of the project,
    imported under the name Python gives it (`src/shoplib/shipping.py` is
    `shoplib.shipping`: the path from the first directory without an `__init__.py`);
  - both, `{ "import": "shoplib", "path": "build/lib/shoplib/__init__.py" }`: examples
    import `import`, and it is read from `path`.

  A surface may be a program, a `__main__.py` (`"shopd": "./src/shopd/__main__.py"`):
  importing it would run it, so its constants are all that facts read of it (see
  [Facts](#facts), below).
- **What a surface exports** is its `__all__` when that is written out as a list. Otherwise
  it is the module's public names: every function, class and assigned name that does not
  start with `_`, and, in a package's `__init__.py`, what it imports from its own package
  (`from .pricing import total` is a re-export; `import os` is not). `uncovered` lists the
  exports no page attests.
- **`sources`** are the Python files whose docstrings hold examples (see below).
- **`setup`** is a Python module with the runners of cases tables and named facts.

## A page

This page documents the small shop library that this package's tests use
(`tests/fixture/`), and this guide checks it inside a copy of that project:

````markdown check fixture=tests/fixture
# Pricing

A discount is a whole percentage taken off a price.

```python check
from shoplib import Money, apply_discount

assert apply_discount(Money(1000), 25) == Money(750)
```

The same as an interpreter session:

```pycon check
>>> from shoplib import Money, total
>>> total([Money(1000)], "NL")
Money(cents=1210, currency='EUR')
```

Prices are in `EUR` unless told otherwise. <!-- fact: DEFAULT_CURRENCY -->

Tax is known for these regions: <!-- fact: TAX_RATES -->
- `NL`
- `DE`
- `FR`

<!-- cases: discount -->
| case           | price | percent | result |
|----------------|-------|---------|--------|
| a quarter off  | 1000  | 25      | 750    |
| rounds half up | 999   | 50      | 500    |

Discounts above the maximum are refused. <!-- test: tests/test_pricing.py > "TestTotals" > "test_rejects_big_discounts" -->
````

```sh
docattest check          # exit 1 when a page and the code disagree
docattest uncovered      # exports no page attests
docattest where total    # the pages, lines and sentences that attest a symbol
docattest audit          # lists, tables and values that look copied from the code, with no directive
docattest impact         # after an edit: the exports it affects and the prose that names them
```

A failing proof is reported with its line, what the page expected and what the code gave.
The same project has a page that is wrong on purpose, once for each way a proof can fail:

```console check cwd=tests/fixture
$ docattest check --quiet docs/broken.md
docs/broken.md
  ✗ L5 example: AssertionError: expected Money(cents=700, currency='EUR'), got Money(cents=750, currency='EUR') (block line 3)
  ✗ L11 example: `quote("NL")`: expected 2, got 1 (block line 2)
  ✗ L17 example: cannot import name 'aply_discount' from 'shoplib' (src/shoplib/__init__.py) (block line 1)
  ✗ L21 example: ShopError: No carrier serves the region (block line 2)
  ✗ L26 example: timed out after 3000 ms
  ✗ L31 example: URLError: <urlopen error network access is disabled for attestations; tag the block `check network` to allow it> (block line 2)
  ! L36 example: example at L36: unknown tag "chekc"; did you mean "check"?
  ✗ L40 fact: expected 60, got 50 from MAX_DISCOUNT
  ✗ L42 fact: missing: DE, FR; unexpected: BE (2 items on the page, 3 in TAX_RATES)
  ! L46 fact: fact `_round(0.5)`: _round is not exported by surface "default" (NameError: name '_round' is not defined)
  ✗ L51 cases: docs/broken.md:51 wrong on purpose: AssertionError: expected '800', got '750'
  ✗ L52 cases: docs/broken.md:52 too much: ShopError: The discount is outside the allowed range
  ! L54 cases: cases runner "nope" at L54 is not in `cases` of docs/docattest_setup.py (known: discount, delivery)
  ! L59 test: tests/test_pricing.py declares no test "test_that_was_renamed"; tests there: "test_discount_rounds_half_up", "TestTotals" > "test_adds_tax", "TestTotals" > "test_rejects_big_discounts"
FAILED
[exit 1]
```

## What each attestation means in Python

### Examples: `python check`, `python compile`

Blocks tagged `python`, `py`, `python3`, `py3` or `pycon` are the adapter's. <!-- fact: BLOCK_LANGUAGES -->

- **`check`** runs the block as a script. The proof is the plain `assert` statement. A
  failing `assert a == b` reports both sides and the line within the block (`L5` in the
  report above); the adapter rewrites comparisons in `assert` to keep the two values, as
  pytest does.
  `await` may be used at the top level; the block then runs in a new event loop.
  `__name__` is `"__main__"`.

  ```python check
  import asyncio

  await asyncio.sleep(0)
  assert __name__ == "__main__"
  ```

  A block with no `assert` (and no `pytest.raises`, `self.assert…`) runs but is counted as *weak*. <!-- test: tests/test_protocol.py > "Text" > "test_assertions" -->
- **A session** is a block tagged `pycon` <!-- fact: ", ".join(SESSION_LANGUAGES) -->, or any block whose first statement is a `>>>`
  prompt. It runs the way `doctest` runs it. <!-- test: tests/test_running.py > "Running" > "test_sessions_run_as_doctest_runs_them" -->
  Each output must match what the page shows, an expected `Traceback` must be raised,
  `# doctest: +SKIP` and the other doctest directives work, and `...` in an expected output
  matches anything (`ELLIPSIS` is on; nothing else is). A statement with an output is the
  session's assertion.
- **`compile`** never runs the block. Python has no compiler to ask, so `compile` means
  what can be said honestly from syntax: the block parses and compiles; every import finds
  its module; a name imported from one of your own modules is bound there
  (`cannot import name 'aply_discount' from 'shoplib'`); an attribute read straight off an
  imported project module exists; and a name the block reads is bound somewhere in it or
  is a builtin. <!-- test: tests/test_static.py > "Static" > "test_compile_is_what_can_be_said_without_running" -->
  It is not a type check: a wrong argument or a missing method is not found.
  `check` blocks pass through the same test before they run.
- **Isolation.** Every block runs in its own process: a fresh `__main__`, fresh imports of
  your package, nothing left over from the block before. Blocks of one page share
  nothing, so each must import what it uses, unless it says `continue`. <!-- test: tests/test_running.py > "Running" > "test_blocks_are_isolated_from_each_other" -->
- **`continue`.** <!-- fact: CONTINUE --> Python documentation is often one interpreter session from the top of
  the page to the bottom: the first block imports, the fifth still uses the import. A
  block tagged `python check continue` (or `pycon check continue`) goes on from the
  examples before it on its page, back to the nearest one without the word: their
  statements run first, in the same namespace, and then the block. <!-- test: tests/test_running.py > "Running" > "test_a_block_tagged_continue_goes_on_from_the_blocks_before_it" -->
  Of these three blocks, the second goes on from the first and the third starts afresh:

  ```python check
  from shoplib import Money

  price = Money(1000)
  assert str(price) == "10.00 EUR"
  ```

  ```python check continue
  assert price == Money(1000)
  ```

  ```python check
  assert "price" not in globals() and "Money" not in globals()
  ```

  It is still judged alone and still runs in its own process; the earlier blocks are run again for it, and
  what they prove is their own result, so a session's outputs are compared only in the
  block that shows them. If an earlier block cannot run, the block fails and says which
  (`the block it continues (L12) did not run: …`). `compile` knows the names the earlier
  blocks bind, and `where` credits a block with what it uses of their imports. A first
  block cannot continue. To bind a page written as one session, tag its first block
  `check` and the rest `check continue`: nothing inside the blocks changes.
- **Renderer attributes** in an info string are ignored:
  ```` ```python title="pricing.py" hl_lines="1 2" check ```` is a `check` block.
- **Time.** Workers are forked from a clean process (`os.fork`); a block that outlives
  `runtime.timeoutMs` has its process group killed. <!-- test: tests/test_running.py > "Running" > "test_a_stuck_block_is_stopped_and_the_next_one_runs" -->
  The time counts from when the block is handed to a worker that has started: starting the
  interpreter is not counted, whether a block is forked from a worker or, where there is no
  `os.fork`, runs in a worker of its own. <!-- test: tests/test_workers.py > "Interpreter" > "test_starting_the_interpreter_is_not_counted_against_a_block" -->
  `runtime.workers` is the number of blocks that run at once.
- **Output.** `print` in attested code goes nowhere; anything written to file descriptor 1
  by other means goes to stderr. <!-- test: tests/test_running.py > "Running" > "test_stdout_carries_nothing_but_responses" -->
- **Network.** Internet sockets are refused unless the block is tagged `check network`
  (which runs only with `DOCATTEST_NETWORK=1`). <!-- test: tests/test_running.py > "Running" > "test_network_is_off_unless_a_block_asks" -->
  The guard patches `socket`; an extension module with its own sockets is not stopped.

### Docstring examples

Every docstring in a `sources` file that contains `>>>` prompts is one example, run as
`python -m doctest` runs it: the session starts with the names of its module, so it needs
no import of the thing it documents. The claim it defends is the docstring's description:
the text before the first prompt or the first `Args:`/`Parameters`/`:param` section. A
function's or class's docstring attests that export; a method's attests its class.
<!-- test: tests/test_running.py > "Running" > "test_a_docstring_session_starts_with_its_module" -->

```python
def apply_discount(price: Money, percent: int) -> Money:
    """Take a percentage off a price, rounding half up to a whole cent.

    >>> apply_discount(Money(1000), 25)
    Money(cents=750, currency='EUR')
    """
```

### Facts

A fact expression is a Python expression over the surface's exports and constants:

```markdown
At most `3` attempts. <!-- fact: RETRY_DEFAULTS["max_attempts"] -->
The rates: <!-- fact column=rate: [f"{r}%" for r in TAX_RATES.values()] -->
```

It may use the exports, the constants (below), the builtins, `snapshot("name")`,
`manifest("path")`, `file("path")`, the named facts of the setup module, and any
standard-library module by its name (`math.pi`, `inspect.signature(f).parameters`): a fact
cannot `import`. It may `await`.

A fact reads a constant from the source of its surface's module, exported or not
(`_PORT`, a name left out of `__all__`), since a module's most documented values, its port
or its default timeout, are often private, or in a script that runs when it is imported. <!-- test: tests/test_constants.py > "Constants" > "test_a_fact_reads_constants_from_source" -->
A constant is a name the module assigns once, in a statement of its own top level (not
inside an `if`, `try` or `with`), to a constant expression: a literal as `ast.literal_eval`
reads one, the name of a constant assigned before it, `+` or `-` before a number,
`+ - * / // %` between numbers, or `+` between two strings, two bytes, two tuples or two
lists. Its value is the one that assignment writes; nothing that changes it later runs. <!-- test: tests/test_constants.py > "Constants" > "test_what_is_a_constant" -->

```python
# src/shopd/server.py
import os

PORT = 8080                         # a constant
_TIMEOUT = 30 * 60                  # a constant: 1800
_BASE = "https://" + "shop.example" # a constant
_STARTED = os.getpid()              # a variable: it calls something
```

A fact that reads only constants does not import its surface, so nothing of the module
runs. A fact that reads any other export imports it, and when that import fails, only such
facts fail. <!-- test: tests/test_constants.py > "Constants" > "test_nothing_of_the_module_runs_unless_a_fact_reads_something_else" -->
A function, class or variable that the module declares and does not export is an error that
says only constants are read there (`_STARTED is a variable that surface "default" does
not export; of its unexported names, a fact reads only constants`), and so is a name the
surface does not have (`_round is not exported by surface "default"` in the report above):
that is what keeps a page honest about what a user can see. Of a program, a `__main__.py`,
a fact reads only constants, exported or not, and `audit` offers its exported constants. <!-- test: tests/test_constants.py > "Constants" > "test_only_constants_are_read_from_source" --> <!-- test: tests/test_constants.py > "Constants" > "test_a_program_offers_its_exported_constants_without_running" -->
A fact attests the constants it reads, exported or not, as `where` shows; `uncovered`
lists only exports. <!-- test: tests/test_constants.py > "Constants" > "test_a_fact_attests_the_constants_it_reads" -->
So `impact` lists a change to a constant as it lists a change to an export, with the facts
that read it. <!-- test: tests/test_constants.py > "ImpactOfConstants" > "test_impact_lists_a_constant_with_the_facts_that_read_it" -->

`manifest("pyproject.toml")` is the data of a project file, read and parsed by the core
(JSON, TOML, YAML and `go.mod`; the [manifests](../../docs/manifests.md) page has the details),
as dicts and lists: `manifest("pyproject.toml")["project"]["requires-python"]`. The commands
and the Python version at the top of this page are read so, on any Python the package
supports, `tomllib` or not. The path is relative to the project root and written out in
the fact: a path built at run time is never read.

`file("path")` is the text of any other project file, a `str`, read by the same rules, for
the methods of `str` and for `re`: the script that runs the adapter from a checkout names
its interpreter `python3`. <!-- fact: re.match(r"#!/usr/bin/env (\S+)", file("bin/docattest-adapter-python"))[1] -->

The page's literal is compared with `str(value)`: a value is displayed as its language
displays it, and the core knows no spelling of its own. So write values the way Python
prints them: `True`, `None`, `1.0`, `('a', 1)`. A list on the page is compared with the items of
any iterable, each through `str`. A dict's items are its keys, as iterating it gives them:
`<!-- fact: TAX_RATES -->` above a list checks the keys. A set has no order, so do not
combine one with `fact ordered`.

A fact with `field=` checks one value of a JSON, TOML or YAML block
([facts](../../docs/facts.md#a-fact-can-check-one-field-of-a-data-block)), which is
written as its format writes values. The adapter shows the block's value as Python shows
what `json` reads it as, so it compares with the fact's as the same value: `true` is
`True`, `null` is `None`, `1` is an `int` and `1.0` a `float`.

~~~~markdown check fixture=tests/fixture
The shop's settings, which match the library's defaults:

<!-- fact field=currency: DEFAULT_CURRENCY -->
<!-- fact field=maxDiscount: MAX_DISCOUNT -->
<!-- fact field=taxed: "NL" in TAX_RATES -->

```json
{ "currency": "EUR", "maxDiscount": 50, "taxed": true }
```
~~~~

### Cases tables and named facts: the setup module

```python compile
# docs/docattest_setup.py
from shoplib import ERROR_CODES, Money, apply_discount


def discount(row):                      # one call per row; the row is a dict of strings
    got = apply_discount(Money(int(row["price"])), int(row["percent"]))
    assert str(got.cents) == row["result"]


discount.columns = ["price", "percent", "result"]   # optional: columns the table must have

cases = {"discount": discount}
facts = {"error_messages": lambda: list(ERROR_CODES.values())}
```

`cases` maps a runner's name to a function; `facts` maps a name to something a fact
expression may use (`<!-- fact: error_messages() -->`). A runner may be `async`. Asserts
in the setup module report both sides, like asserts in a block.

A runner gets each cell's text as the page writes it, code spans included: a cell
`` `print(1)` `` reaches it with its backticks
([docs/cases.md](../../docs/cases.md#a-runner-receives-each-cell-as-written)).
`docattest.cells.code_spans` takes the code out: the content of every code span in a cell,
in order, as a fact over the cell's column compares it. <!-- test: tests/test_cells.py > "CodeSpans" > "test_code_spans_are_what_a_column_fact_compares" -->

```python check
from docattest.cells import code_spans

assert code_spans("`FOO=1` `BAR=2`") == ["FOO=1", "BAR=2"]
assert code_spans("``a`b`` or `` `c` ``") == ["a`b", "`c`"]
assert code_spans("no code") == []
```

A span opens with a run of backticks and closes at the next run of as many, and the white
space around its content is dropped. As CommonMark has it, a backtick escaped with a
backslash opens no span, and neither does one inside an HTML tag or comment, an autolink,
or a link's destination or title. <!-- test: tests/test_cells.py > "CodeSpans" > "test_code_spans" -->
The module is part of the `docattest` distribution, so a setup module imports it from the
environment its code runs in: install `docattest` there, as you do to run it. This guide's
runners read their cells so ([docattest_setup.py](docattest_setup.py)).

A row is named by the page, its line and its first cell. A table's rows run one after
another, in the page's order, in one process, which runs the setup module once. Module
globals, and anything a row starts and does not stop (threads, and the servers they run;
signal handlers), are still there for the rows after it, so a runner stops what it starts
before it returns. A row that raises fails, `SystemExit` too, and the next row runs in the
same process. A row that runs out of time or ends the process (`os._exit`) is the last of
its process: the next row starts a fresh one, which runs the setup module again. <!-- test: tests/test_cases_rows.py > "CasesRows" > "test_a_tables_rows_share_its_process" -->
Tables never share a process, and run side by side, with each other and with blocks:
`runtime.workers` processes at once in all. <!-- test: tests/test_cases_rows.py > "CasesRows" > "test_tables_run_side_by_side_and_never_share_a_process" -->

### Citations

`<!-- test: tests/test_pricing.py > "TestTotals" > "test_rejects_big_discounts" -->` ties
a sentence to a test. A test's title is its function's name: any function or method whose
name starts with `test`, as pytest and unittest find them. <!-- test: tests/test_static.py > "Static" > "test_tests_are_listed_from_syntax" --> Enclosing classes may be named
first, outermost first. Citing a class itself is an error that says so
(`"TestTotals" in tests/test_pricing.py is a class, not a test`). The file is read, never
run.

The cases of a parametrized test, and the subtests of a test, can be cited too: each is a
test inside its test, titled by the id its runner prints for it, so the case that pytest
reports as `test_total[big discount]` is cited as `"test_total" > "big discount"`, the way
a test inside a class is.

- **`@pytest.mark.parametrize`.** A case's id is the one written out, in `ids=[...]` or in
  `pytest.param(..., id="...")`, or else the one pytest makes of a literal value: a string
  or bytes with what is not printable ASCII escaped; a number, `True`, `False` or `None` as
  `str` writes it; `<argname><index>` for a tuple, list, set or dict. A row's ids are joined
  with `-`, and so are those of stacked decorators, the one nearest the function first,
  then those on its classes. The rows are a list or tuple written in the decorator or
  assigned once to a name of the module. <!-- test: tests/test_citations.py > "Citations" > "test_the_cases_of_parametrized_tests" -->
- **`with self.subTest(...)`.** A case is titled by what unittest prints for it: its
  message (`subTest("big discount")`), its parameters as unittest writes them
  (`subTest(region="NL")` is `region='NL'`), or both (`NL (rate=21)`). Each argument is a
  literal, or comes from the row of a `for` loop over a literal table, written in the loop
  or assigned once to a name of the method or of the module: one of the loop's names, an
  item of one (`case["name"]`, `case[0]`), or a key of a literal dict's `.items()`. <!-- test: tests/test_citations.py > "Citations" > "test_the_cases_of_subtests" -->
  This guide cites one of this package's own:
  `code_spans` reads two spans from a cell that holds a backtick. <!-- test: tests/test_cells.py > "CodeSpans" > "test_what_a_runner_reads_of_a_cell" > "a longer run holds a backtick" -->

What cannot be read so cannot be cited: a value that is not a literal, ids a function
makes, a table changed after it is assigned (`CASES.append(…)`), a subtest inside another,
and ids that repeat, which pytest renames by rules that change between its versions. No case
of a test under a `pytestmark` that parametrizes can be cited. <!-- test: tests/test_citations.py > "Citations" > "test_a_pytestmark_that_parametrizes_hides_the_cases" -->
The test itself still can, and a citation of a case that is not there names the ones that
are (`tests/test_p.py declares no test "test_named" > "large"; tests there: "test_named",
"test_named" > "small", …`). <!-- test: tests/test_citations.py > "Citations" > "test_a_case_is_cited_by_its_id" -->
A case attests what its test uses. <!-- test: tests/test_citations.py > "Citations" > "test_a_case_attests_what_its_test_uses" -->

## Which Python runs your code

The adapter itself only reads syntax. Attested code runs on, in order of preference: the
interpreter in `DOCATTEST_PYTHON`; the active virtualenv (`VIRTUAL_ENV`); the project's
`.venv`; the interpreter the adapter runs on. So an adapter installed once (with `pipx`,
say) still runs each project's code in that project's environment. `DOCATTEST_PYTHON`
comes first even when the interpreter it names cannot be started:

```console check cwd=tests/fixture
$ DOCATTEST_PYTHON=/nowhere/python docattest check docs/pricing.md
docattest: cannot start the Python interpreter /nowhere/python that attested code runs on: No such file or directory
[exit 1]
```

<!-- cases: interpreter -->
| DOCATTEST_PYTHON | virtualenv | .venv | runs on |
|---|---|---|---|
| set | active | exists | `DOCATTEST_PYTHON` |
| not set | active | exists | the active virtualenv |
| not set | none | exists | the project's `.venv` |
| not set | none | none | the adapter's own interpreter |

## What is never your project's own

The adapter tells the core which files and directories to leave alone, wherever they are:
`__pycache__`, `site-packages`, `node_modules`, and anything named `*.egg-info` or
`*.dist-info`. <!-- fact: IGNORED_NAMES -->
So is any directory that holds a `pyvenv.cfg` <!-- fact: ", ".join(IGNORED_MARKERS) -->, which is a
virtualenv whatever it is called. Hidden directories (`.venv`, `.tox`) are left out by the
core itself.

## Limits

- No type checking: `compile` is syntax, imports and names (see above).
- Exports are read from syntax. An `__all__` that is computed at import time is not read;
  the report warns and the public names are used instead.
- `impact` works on top-level functions, classes and assignments. Renaming a function
  together with its callers reads as a change to all of them.
- Without `os.fork` (Windows) every block starts a new interpreter. <!-- test: tests/test_running.py > "Running" > "test_without_fork_every_job_gets_its_own_process" -->
  This path has a test on POSIX but has not been run on Windows.
- If the adapter is killed with `SIGKILL`, a block that is still running is not stopped.

## Working on this package

[CONTRIBUTING.md](CONTRIBUTING.md) maps the package's sources, says how to run its tests,
lint and type checks, and how this guide is checked.
