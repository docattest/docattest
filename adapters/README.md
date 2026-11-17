# Language adapters

Each directory here is one language: the package a user of that language installs, holding
the language's **adapter**, a **launcher** for the docattest core, and a copy of the
protocol's data.

| directory | package | adapter command | README |
|---|---|---|---|
| [typescript/](typescript) | npm `docattest` | `docattest-adapter-typescript` | [typescript/README.md](typescript/README.md) |
| [python/](python) | PyPI `docattest` | `docattest-adapter-python` | [python/README.md](python/README.md) |
| [go/](go) | `go install` | `docattest-adapter-go` | [go/README.md](go/README.md) |
| [rust/](rust) | crates.io `docattest` | `docattest-adapter-rust` | [rust/README.md](rust/README.md) |

Everything that is the same in every language (configuration, Markdown, directives, binding,
comparison, the index, the report, git, `audit`, `check --base`, `impact`, hooks, exit
codes) is in the Rust core, [core/](../core/README.md), and is written once. An adapter
answers only what needs its language's compiler or runtime. The contract between the two is
[protocol/PROTOCOL.md](../protocol/PROTOCOL.md); this guide follows it section by section and
points at what the existing adapters do, so read it with the specification open.

## How to add a language

### 1. What an adapter is

A program, written in the language it serves, that the core starts as a child process when
a command first needs the language, and that answers one JSON request per line on standard
input with one JSON response per line on standard output (PROTOCOL.md §1). It lives for one
docattest invocation, listens on nothing, and exits when its input ends. A project selects
it with `"adapter": "<name>"` in `docattest.config.json`; the core runs the command in
`DOCATTEST_ADAPTER_<NAME>`, else `docattest-adapter-<name>` on the `PATH`. The core keeps no
list of adapters, so a core you did not build drives yours.

Two rules shape every adapter:

- **Stdout carries responses and nothing else.** Attested code prints; whatever it or the
  adapter's own dependencies write, by any route, must reach stderr. The TypeScript adapter
  runs its server as a child whose descriptor 1 is the parent's stderr and answers on
  descriptor 3 (`typescript/src/protocol/main.ts`); the Python adapter's workers point
  descriptor 1 at stderr before running anything (`python/docattest_python/workers/worker.py`);
  the Go and Rust adapters run attested code only in other programs, whose output they
  keep (`go/internal/toolchain/run.go`, `rust/src/toolchain/mod.rs`).
- **The adapter observes; the core decides.** It never sees a page or an expected value it
  does not need to run, never compares a fact with the page, and sorts nothing for the core
  (§2, §4). A value is reported as text, as the language displays it (`str(value)`,
  `String(value)`); the core compares strings.

### 2. The minimum it must answer

Start with `hello`, which comes first, is static and is answered at once, before loading a
compiler (§3, "Language level"):

```json
{ "protocol": 1, "adapter": { "name": "ruby", "version": "0.1.0" },
  "blockLanguages": ["ruby", "rb"],
  "sourceFiles": { "include": ["**/*.rb"], "exclude": [] },
  "ignoredNames": ["vendor", ".bundle"],
  "capabilities": [] }
```

Then every method that is not marked *(capability)* in §3 must answer, one item per item
sent, in order. Several have an honest "cannot tell" answer that is enough to begin with,
each costing one feature:

| method | the least it may answer | what that costs |
|---|---|---|
| `docComments` | `[]` | no examples from doc comments (`sources` checks nothing) |
| `assertions` | `true` for each example | a `check` block that asserts nothing is not reported as weak |
| `docProse` | `[]` for each file | `check --base` cannot follow a claim made in a doc comment |
| `mentions` | `null` for each span | `impact` lists no unproven mentions in code spans |
| `expressions` | the expression as its own shape, no names | a renamed fact expression is reported as weakened |
| `open`, `close` | `{}` | (required) |
| `compile` | `[]` (no diagnostics) | every `compile` block passes, proving nothing |
| `evaluate`, `run`, `symbols`, `exports`, `describeSurfaces` | real answers | these are the product: facts, examples, cases and the index |
| `inCode` | every name | a renamed fact expression is reported as weakened |

`evaluate` and `run` are where the work is: running each example in isolation from the
others, with `timeoutMs` per block, at most `workers` at once, and the network refused
unless `network` is true (`open`, §3). The TypeScript and Python adapters run blocks in
separate worker processes and kill one that overruns
(`typescript/src/language/run/`, `python/docattest_python/workers/pool.py`). A compiled
language has no worker to send a block to: the Go and Rust adapters build every block of a
run as a program of its own, in one run of the compiler, and then run each
(`go/internal/running/blocks.go`, `rust/src/running/blocks.rs`).

The protocol's rules about order, errors and mistakes (§1: `hello` first, `open` in arrival
order, `close` waiting, an unknown method or bad params answered with an `internal` error
that does not end the process, `"id": null` for an unreadable line, exit 0 at end of input)
are checked by the conformance suite, so you find out early.

### 3. What each optional part buys

| optional | declared in | what it enables |
|---|---|---|
| `tests` | `hello.capabilities` | citations (`<!-- test: file > "title" -->`); without it every citation is an error on its line |
| `observables` | `hello.capabilities` | `audit` and `inventory`; without it `inventory` is a user error and `audit` finds nothing |
| `affected` | `hello.capabilities` | `impact`, `impact --file` and the agents' `impact --hook`; without it `impact` is a user error |
| `ignoredMarkers` | `hello` | leave out directories marked by a file inside them (Python: a virtualenv's `pyvenv.cfg`) |
| `infoWords` | `hello` | words of your own on a code fence, passed to you in `tags` (Python: `continue`) |
| `vocabulary` | `hello` | your language's words in two of the core's sentences about citations |
| `warm` | sent by the core in `hello` | a hint that more requests follow, so you may start loading your compiler now |

### 4. The launcher

A user does not install the core or your adapter separately: they install your language's
package, and its `docattest` command is a **launcher** (§1, "Launchers") that:

1. **finds the core**: `DOCATTEST_CORE`; else the binary your package brought for this
   platform; else the shared per-user cache `<cache>/core/<version>/<os>-<arch>/docattest`.
   Finding none is an error that names all three and says how to get a core;
2. **checks it**: your package pins the core version and protocol it was built for (a copy
   of `protocol/core.json`). A core from `DOCATTEST_CORE` is asked `--version --json` and
   refused if its protocol differs; the bundled one is not run to be checked;
3. **hands over and gets out of the way**: sets `DOCATTEST_ADAPTER_<NAME>` to a JSON array
   that starts your adapter, and `DOCATTEST_COMMAND` to the shell command that runs
   docattest in the project (with `{root}` where it needs the project root), leaving either
   alone if already set, and then `exec`s the core so that arguments, streams, signals and
   the exit code are the core's own.

The launcher knows nothing of coding agents; `--agent` is passed through like any argument.
Read `typescript/src/launcher/`, `python/docattest/launcher.py` and `go/internal/launcher/`:
they are short, and their tests (`typescript/test/unit/launcher/`,
`python/tests/test_launcher.py`, `go/internal/launcher/launcher_test.go`) are the
specification of the details (platform names, messages, quoting).

### 5. Packaging: the install brings the core

The core is one native binary per platform, listed in
[protocol/platforms.json](../protocol/platforms.json) (`darwin-arm64`, `darwin-x64`,
`linux-arm64`, `linux-x64`, `windows-x64`; the Linux builds are static and serve glibc and
musl alike). Your package must deliver the right one at install time, with no install script
and no download at run time:

- **npm** does it with one optional dependency per platform, `@docattest/core-<platform>`,
  each restricted by `os` and `cpu` (`scripts/release/npm-platform.ts` makes them);
- **Python** does it with one wheel per platform, `py3-none-<tag>`, with the binary inside
  the launcher package (`python/build_wheel.py`);
- **Go** installs by building source, so the core travels as source too: one small Go
  module per platform that embeds that platform's core, which the launcher imports under
  the platform's build constraint and writes to the shared cache on its first run
  (`go/internal/bundled/`, `go/tools/coremodule/`). The go command downloads only the one
  module, and checks it against `go.sum`. A release tags those modules in this repository,
  outside `main` (`scripts/release/go-modules.ts`);
- **Rust** needs none of this: the core is a Rust library, so the Rust package depends on
  it and its `docattest` command is built with the core inside (`rust/src/bin/docattest.rs`).

Use your ecosystem's own mechanism for platform-specific artifacts. Then:

- copy the protocol's data into the package at build time from the one source
  (`node protocol/sync.ts <dir>`, or a script like `python/sync_protocol.py`), and add a
  test that fails when the copy differs (a package whose install builds from the
  repository's source, as Go's does, keeps the copy in the tree);
- add your package's version to [scripts/version.ts](../scripts/version.ts), so a release
  is one version everywhere;
- build it in [.github/workflows/release.yml](../.github/workflows/release.yml) from the
  cores the workflow already builds, and install it into a fresh project in
  [tests/smoke/](../tests/smoke) as the npm, pip and Go steps do.

### 6. Test it

**Conformance.** Write a *kit*, `adapters/<name>/conformance.json`: a small project in
memory and labelled pieces of code in your language (§5). Copy
[python/conformance.json](python/conformance.json) (or [go/conformance.json](go/conformance.json),
for a compiled language) and translate every value; `cases` and
`affected` may be left out, every other key is read. Its shape is
[protocol/schema/kit.json](../protocol/schema/kit.json). Then:

```sh
docattest conformance run adapters/<name>/conformance.json -- <program> <arguments>...
```

The core carries the suite, so any `docattest` runs it (a core built from this repository is
`core/target/release/docattest`, after `npm run build:core`). It prints
`conformance (<name>): N of N checks hold` when every check holds. Run it from your
package's own tests too, as the existing packages do.

**Against a real core.** Build the core (`npm run build:core`) and run it on a sample
project with your adapter named in the environment:

```sh
export DOCATTEST_ADAPTER_<NAME>='["<program>", "<arguments>"]'
core/target/release/docattest check --root path/to/sample
```

**The schemas.** [tests/schema.ts](../tests/schema.ts) records what the core really sends
to each adapter on a real project and validates every line; add your adapter and a sample
project to its list.

**The mirror test.** [tests/mirror.ts](../tests/mirror.ts) takes benchmark scenarios
written for TypeScript, translates each page and source file to every other language by
hand, runs all of them through the same core and requires the same verdict and exit code.
Each language is one file of tables in [tests/mirror/](../tests/mirror) (`python.ts`,
`go.ts`, `rust.ts`); to hold another to the same bar, add its file in the same shape, list it in
`tests/mirror.ts`, and run it the same way
(`node tests/mirror.ts --core core/target/release/docattest`).

See [tests/README.md](../tests/README.md) for what each suite proves.

### 7. Checklist

- [ ] `adapters/<name>/` with the package, a README for its users, and a license file the
      package ships
- [ ] an adapter command, `docattest-adapter-<name>`, that answers `hello` before loading
      anything heavy
- [ ] every required method of §3, and the capabilities you can support listed in `hello`
- [ ] stdout carries only responses, whatever attested code does
- [ ] examples isolated from each other, with timeouts, workers and the network rule of
      `open`
- [ ] a launcher that finds, checks and `exec`s the core as §1 says, with tests
- [ ] the protocol data copied at build time, with a test that the copy is the source
- [ ] a conformance kit, and the suite passing in your package's tests and in CI
- [ ] platform packages that bring the core; the version in `scripts/version.ts`; the
      release workflow and the smoke test install it
- [ ] user documentation: what each kind of proof means in your language (see
      [python/README.md](python/README.md), [go/README.md](go/README.md) and
      [rust/README.md](rust/README.md) for the model)
- [ ] a line in `CHANGELOG.md`, and the language added to the root README
