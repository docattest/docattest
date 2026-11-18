# Working on the Python package

The distribution holds two import packages. `docattest/` is the launcher: it finds the
core, hands it what it needs in the environment and `exec`s it. Beside it is `cells`, the
helper a setup module imports to read the code spans of a table's cells. `docattest_python/` is the
adapter, and its subpackages follow the path of a request: <!-- fact: [p.split(".")[1] + "/" for p in manifest("pyproject.toml")["tool"]["setuptools"]["packages"] if p.count(".") == 1] -->
- `server/` reads the core's requests and answers them. Every request's params are read
  into typed values there, so a field that is missing or of the wrong type is refused
  where it enters, as an `internal` error that names it.
- `session/` is the project that `open` describes and the parts that answer questions
  about it: a workspace (the project's files, modules, surfaces and setup module), what
  `compile` reports, the symbols and tests of attestations, and the work sent to the pool.
- `analysis/` reads Python modules without importing them: what a module binds and
  exports, the values of its constants, which names a piece of code uses, what a
  configured surface resolves to and what a fact may read of it, and which exports a change
  affects. Its `declared_tests/` reads the tests a test file declares, with the cases that
  `parametrize` and `subTest` write. It knows nothing of sessions or processes.
- `workers/` is where attested code runs: the interpreter it runs on, the pool of worker
  processes, and the worker itself. The worker (`worker.py`, started by its path, and its
  parts in `docattest_worker/`) uses only the standard library and may run on the
  project's own interpreter, so it imports nothing from the rest of the adapter. What the
  adapter sends it and what it writes back are typed messages defined beside it, read
  back on each side with the same field reader.

At the top of `docattest_python/` sit what every layer shares: the attestations as typed
values, the two kinds of error a response can carry, and what Python says of itself
without a project (`hello` and the questions about text). `protocol/` there is the copy of
the repository's `protocol/` data the package ships, written by `python3 sync_protocol.py`
and by every build, never by hand; it is kept in the tree so the package runs from a
checkout without a build step.

Beside the packages: `bin/docattest-adapter-python` runs the adapter from a checkout, for a
bare core; `build_wheel.py`, `setup.py` and `pyproject.toml` are the packaging, and
`pyproject.toml` also holds the lint, format and type-checking settings; `conformance.json`
is this adapter's kit for the protocol's conformance suite; `docattest.config.json` and
`docattest_setup.py` check the guide and this page (below); and `tests/` holds the unit
tests, with `tests/fixture/`, a small project the tests and the smoke test check.

## Tests

```sh
python3 -m unittest discover -s adapters/python/tests -v
```

Most tests drive the server over its pipes, the way the core does (`tests/talk.py`), and
hold every request and response to the protocol's schemas (`tests/schema.py` is a small
validator; the adapter itself needs none). Others start the worker and the pool directly,
run the real core against `tests/fixture/` through the launcher (skipped unless a core
binary is found; set `DOCATTEST_CORE` <!-- fact @launcher: CORE_VARIABLE -->, or build it in `core/target/release`), cover where
the launcher looks for the core and what it hands over, check that the package's copy of
the protocol data is the repository's `protocol/`, and run the core's conformance suite
with this adapter's kit (skipped, as the core's tests are, unless a core binary is found).

The suites that test this adapter together with the rest (the mirror test, which runs
benchmark scenarios through both the TypeScript and the Python adapter and compares the
verdicts, and the schema recordings) are in [tests/](../../tests/README.md).

## The guide is checked

docattest checks the [guide](README.md) and this page with this adapter, the way it
checks any Python project's pages. [docattest.config.json](docattest.config.json) names three surfaces: the adapter's
`language` module, the launcher, and the test fixture's `shoplib`, so the guide's examples
are compiled against a real package. [docattest_setup.py](docattest_setup.py) holds the
named facts (read from `pyproject.toml` and the package's protocol data) and the runners of
the tables. From the root of the repository, with `DOCATTEST_CORE` set as above:

```sh
PYTHONPATH="$PWD/adapters/python" python3 -m docattest check --root adapters/python
```

This directory is also one of the `members` of the repository's root config, so the one
`docattest check` at the root (`npm run test:docs`) checks it too, with this adapter.

## Lint, format and types

[ruff](https://docs.astral.sh/ruff/) and [pyright](https://microsoft.github.io/pyright/),
pinned and configured in `pyproject.toml`, installed into a virtualenv of their own:

```sh
cd adapters/python
python3 -m venv .venv && .venv/bin/pip install --group lint    # pip 25.1 or later
.venv/bin/ruff check . && .venv/bin/ruff format --check . && .venv/bin/pyright
```

pyright checks the package strictly against Python 3.10 <!-- fact: manifest("pyproject.toml")["tool"]["pyright"]["pythonVersion"] -->,
the oldest the package supports, and the tests in its standard mode. Its first run downloads the Node it runs on.

`tests/fixture/` is left as written: its pages cite its lines.
