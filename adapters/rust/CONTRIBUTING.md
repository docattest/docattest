# Working on the Rust package

The crate builds two programs from one library. `src/bin/` holds their `main`s: the
`docattest` command, which hands over to the core it is built with (the hand-over lives
in the launcher module), and the adapter. The library is laid out by responsibility,
with dependencies pointing one way, from the protocol loop down to cargo:

- **The protocol** module holds the protocol's data types and the loop that reads
  requests, routes them and writes responses. It knows nothing of Rust.
- **The language** module answers what needs no project: the greeting, and the
  questions about text (how a block is read as a program, assertions, mentions,
  expression shapes, doc comments and their examples).
- **The session** module is the one project `open` creates, as the protocol sees it: it
  reads what `open` says and routes each method either to the analysis or to the
  running side.
- **The analysis** module answers from syntax: a crate's modules and exports, the
  surfaces, `symbols`, `tests`, `affected`, the names of the setup module, which
  constants a fact may read from source, and the docs helpers of a surface's module.
  The one thing it asks the compiler is what a module exports when a macro writes some
  of it.
- **The running** module compiles and runs: the crate generated beside the project, its
  programs for examples, facts, `observables` and cases tables, the project's own tests
  that citations name (`runTests`, by `cargo test` in the project), and the slots that
  bound how many run at once. It reads the project through the analysis, never the
  other way round.
- **The project** module asks cargo what the workspace is made of, and **the toolchain**
  module runs cargo, rustdoc and the programs cargo builds, with a time limit.
- **The harness** is the Rust source compiled into every generated program: showing a
  value of any type, a small JSON reader, serving items one by one, saying when a block's
  code starts, and the code spans of a table's cell, which setup files call. Its modules
  are modules of this crate, tested here, and are also embedded as text and written into
  a package of their own beside the generated crate, `docattest_harness`, so they use only
  `std`. A fact program that needs nothing of the project is a program of that package.

Each module is a directory or a file of `src/`, beside `lib.rs`, the launcher and the
programs:

```console check
$ ls src
analysis
bin
harness
language
launcher.rs
lib.rs
project.rs
protocol
running
session
toolchain
```

Beside the source, `conformance.json` is this adapter's kit for the protocol's
conformance suite, and `tests/fixture/` is a small Rust project the tests drive.

## Tests

```sh
cd adapters/rust
cargo test
```

The unit tests sit beside the code they test. `tests/statics.rs` drives a session's
answers from syntax on `tests/fixture/`, and `tests/session.rs` compiles and runs real
code against it with your cargo. `tests/compiled.rs` covers what the compiler is asked,
and `tests/workspace.rs` a workspace whose names the generated programs must not confuse
(a crate that declares a module of its own name, a private module) and the features they
are built with. `tests/constants.rs` covers the constants facts read from source, of a
library and of a binary, `tests/helpers.rs` what facts and runners reach through the
setup file's functions and through docs helpers, `tests/cases.rs` how the rows of cases
tables run, `tests/cited.rs` how `cargo test` runs the tests citations name, and
`tests/cells.rs` the cell helper against the core. These write small projects to the
temporary directory. What the tests
share (opening a session, writing a project, running the `docattest` command) is in
`tests/project/`. `tests/endtoend.rs` runs the conformance suite with this
adapter's kit (through the `docattest` command, whose core carries it) and the `docattest`
command on the fixture. <!-- test: tests/endtoend.rs > "the_adapter_conforms_to_the_protocol" -->

The repository's own core is the real project this adapter is held to: `core/` has a
`docattest.config.json`, and `docattest check --root core` checks the core's README and
its doc comments against the core. [The guide](README.md) and this page are checked the
same way, against this crate ([docattest.config.json](docattest.config.json)):
`docattest check --root adapters/rust` fails when a sentence of them with a proof behind
it stops being true. Its surfaces are the public modules of the library. Both are
`members` of the repository's root config, so the one `docattest check` at the root
(`npm run test:docs`) checks them too, with this adapter. The suites that test this
adapter together with the rest (the mirror test and the schema recordings) are in
[tests/](../../tests/README.md).

## Lint and format

```sh
cd adapters/rust
cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

`rustfmt.toml` is the core's. `tests/fixture/` is formatted by it too; its pages cite its
tests by name, not by line.
