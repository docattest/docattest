# The adapter protocol

How the docattest core talks to a language adapter when the two are separate programs.
The core holds every rule that is the same in every language: configuration, pages,
directives, binding, comparison, the index, the report, exit codes. An adapter is written
in the language it serves and answers only what needs that language's compiler or
runtime. This file is the whole contract between them.

Status: version 1, draft. Four adapters speak it: TypeScript
([`adapters/typescript/`](../adapters/typescript/README.md)), Python
([`adapters/python/`](../adapters/python/README.md)), Go
([`adapters/go/`](../adapters/go/README.md)) and Rust
([`adapters/rust/`](../adapters/rust/README.md)). How to write another:
[adapters/README.md](../adapters/README.md).

## 1. Shape

- **Not a service.** The core starts the adapter as a child process when a command first
  needs the language, talks to it over the child's standard input and output, and closes
  its input when done; the adapter then exits. One adapter process lives for one docattest
  invocation at most. Nothing listens on a socket, nothing is left running, nothing is
  shared between invocations. A command that needs no language starts no adapter.
- **Pipes.** Requests go to the adapter's stdin, responses come from its stdout, one JSON
  value per line (UTF-8, no embedded newlines). Stdout carries nothing else: output from
  attested code must never reach it. The adapter's stderr is passed through for a human
  to read and is never parsed.
- **Requests and responses.** A request is `{ "id": 1, "method": "evaluate", "params": {…} }`.
  The response is `{ "id": 1, "result": … }` or
  `{ "id": 1, "error": { "kind": "user" | "internal", "message": "…" } }`. Ids are positive
  integers chosen by the core, unique within the process. `params` may be omitted when a
  method takes none. Lines end with `\n`; a blank line is ignored. The core may send several requests before
  reading a response, and the adapter may answer in any order; that is how compiling,
  evaluating and running overlap.
- **Errors.** `user` is a mistake in the project (a surface that cannot be loaded, a
  missing file): the core prints the message exactly as given, adding nothing, and exits 1.
  Every message docattest prints for a user begins `docattest: `, and since the core adds
  nothing, the adapter writes that prefix itself. `internal` is a bug in the adapter: the
  core reports it and exits 2. An adapter that exits early, or writes a line that is not a
  response, is an internal error, and so is an answer that lacks something this file
  requires of it: the core has no words of its own to put in its place.
- **Finding the adapter.** For a config that says `"adapter": "<name>"` the core runs the
  command in the environment variable `DOCATTEST_ADAPTER_<NAME>` (the name in upper case).
  It is how a launcher (below) says where its adapter is: a JSON array of strings, the
  program and its arguments, so a path may hold spaces. For a bare core in development the
  variable may instead hold a command line, which is split on spaces; and when it is not
  set, `docattest-adapter-<name>` is looked for on the `PATH`. Not finding one is a user
  error that names both. The core looks nowhere else: where a language's packages are
  installed is not its business. It holds no list of adapters either: any name of
  lower-case letters, digits and `_` that starts with a letter is a valid config, and
  whether there is such an adapter is found out here.

- **Order.** `hello` comes first and is answered before anything else is sent; a `hello`
  refused for its protocol number may be sent again with another. `open` is handled in
  arrival order, so the core may send session methods straight after it without waiting.
  `close` waits for every earlier request to be answered; a session method after `close`
  is an error. At end of input the adapter writes the answers still in flight, closes the
  session and exits 0.
- **Mistakes by the core.** An unknown method, a session method before `open`, a second
  `open`, a method before `hello`, and params of the wrong shape are answered with an
  `internal` error and do not end the process. A line that cannot be read, or that has no
  usable id, is answered with `"id": null`.
- **The adapter's own output.** Whatever the adapter process or anything it loads prints,
  by any route, goes to stderr. Only responses reach stdout.

### Launchers

Nobody installs the core by itself. A user installs a language package (from npm, from
PyPI, from crates.io, or with `go install`), and that package brings the core with it and provides the `docattest` command: a
*launcher*, written in the package's language, which knows what the core must not, namely
how docattest was installed. A launcher does three things and then gets out of the way.

1. **It finds the core**, in this order, the same in every package:
   1. `DOCATTEST_CORE`, the path of a core binary;
   2. the binary the package brought for this platform (npm: the optional dependency
      `@docattest/core-<os>-<arch>`, as `bin/docattest`; Python: `docattest/bin/docattest`
      inside a platform wheel; Go: `docattest-core` beside the launcher, where a release
      archive carries it, or inside the launcher itself, which then writes it to the cache
      below when the cache has none; Rust: the command itself, which is built with the
      core inside it and runs it without starting another program);
   3. a cache shared by every language package of one user:
      `<cache>/core/<core version>/<os>-<arch>/docattest`, where `<cache>` is
      `DOCATTEST_CACHE`, else `docattest` in the user's cache directory
      (`~/Library/Caches` on macOS, `%LOCALAPPDATA%` on Windows, `$XDG_CACHE_HOME` when it
      is absolute or else `~/.cache` elsewhere). `<os>` is `darwin`, `linux` or `windows`;
      `<arch>` is `x64` or `arm64`; the file is `docattest.exe` on Windows.

   No core in any of them is an error that names all three, and then the package it
   expected and how to get a core. Nothing is downloaded. The platforms a core is built
   for, and the npm package that carries each, are listed once, in
   `protocol/platforms.json`, which every package copies beside `core.json`. A Linux core
   is static, so one binary (one npm package, one wheel) serves glibc and musl alike.
2. **It checks that the core is one it can work with.** A package is built for one core
   version and one protocol version, which it pins: it ships a copy of `protocol/core.json`
   (`{ "version": "0.1.0", "protocol": 1 }`) and of the schemas of §2. The bundled binary
   is the package's own, and a cache path names the exact version, so neither is run to be
   checked: on the usual path a launcher costs no second process. A core named by
   `DOCATTEST_CORE` is asked: `docattest --version --json` prints
   `{"version":"…","protocol":1}` at once, starting no adapter, and the launcher refuses a
   core whose `protocol` is not its own.
3. **It hands over** what the core cannot know, in the environment, and replaces itself
   with the core (`exec`, or the nearest thing the language has): arguments, standard
   streams, signals and the exit code are the core's own.

   | variable | value | read by |
   |---|---|---|
   | `DOCATTEST_ADAPTER_<NAME>` | how to start the package's adapter: a JSON array, the program and its arguments | every command that needs the language |
   | `DOCATTEST_COMMAND` | a shell command that runs docattest in this project, on one line; the core appends arguments to it. `{root}` in it stands for the project root | `hooks install`, which writes it into a coding agent's settings or a git hook, and `agents install`, which writes it into `AGENTS.md` |

   A variable that is already set is left alone, so a developer's own adapter and a
   wrapper's own command stand. `DOCATTEST_COMMAND` is a shell command and not an array
   because a hook is one: the launcher quotes what needs quoting. A launcher knows how
   docattest was installed and nothing of who will run the command, so where the command
   needs the project root it writes `{root}` (`{root}/.venv/bin/docattest`), and the core
   puts there what the reader of the command calls the root: a coding agent's own spelling
   in that agent's hook (`"$CLAUDE_PROJECT_DIR"`, from the agent's descriptor), and `.` in
   `AGENTS.md` and in a git hook, which are run in the project root. The npm launcher says
   `npx --no docattest`. The Python launcher says the virtualenv's own script by a path
   relative to the project when the virtualenv is inside it, else `docattest` when that is
   on the `PATH`, else its interpreter's full path. A bare core, told nothing, writes
   `docattest`. The project root a launcher means is the one the core will find: `--root`,
   else the nearest directory at or above the working directory that holds a
   `docattest.config.json`, else the working directory.

   **Coding agents are not the protocol's business, and not a launcher's.** What the core
   knows of one is a descriptor, `protocol/agents/<name>.json` (its format is
   `protocol/schema/agent.json`; [docs/agents.md](../docs/agents.md) explains it and
[agents/README.md](agents/README.md) says how to add one), built into the core and
   copied into each package beside the schemas by `protocol/sync.ts`. A launcher passes
   `--agent` through like any other argument.

## 2. Data

Every value has a schema. The shapes of every request, response and data type are
published as JSON Schema (draft 2020-12) in `protocol/schema/`, which is their definition:

| file | what |
|---|---|
| `messages.json` | a request line, a response line, an error |
| `methods.json` | for each method, `#/$defs/<method>.params` and `#/$defs/<method>.result` |
| `attestations.json` | the attestations the core sends: `ExampleAttestation`, `FactAttestation`, `CasesAttestation`, `TestAttestation` and their parts |
| `language.json` | what a language says of itself: `Hello`, `SourceFiles`, `Vocabulary`, and its answers about text (`DocProse`, `ExpressionShape`) |
| `results.json` | what a project answers: `Result`, `Diagnostic`, `Evaluation`, `Observation`, `Observable`, `SymbolRef`, `TestListing` |
| `project.json` | the project as `open` describes it: its surfaces (`ConfiguredSurface`, `SurfaceDescription`), its snapshots and its manifests |
| `kit.json` | an adapter's conformance kit (`Kit`, §5), which the conformance suite reads; it never crosses the pipe |

They use a small part of JSON Schema (`type`, `properties`, `required`,
`additionalProperties`, `items`, `enum`, `const`, `anyOf`, `minimum`, `minLength`, `$ref`),
so a small validator reads them; the core carries one (`core/src/schema/`). Each
language package ships a copy, made from this one source when it is built, and tests on each
side hold real messages to them (§5). The TypeScript adapter's types
(`adapters/typescript/src/api/`) have the same names and the same fields, but they follow
the schemas, not the other way round.

Optional fields are omitted, never `null`, except where a schema says `null`. Where an
answer is keyed (by attestation id, by surface name) it is an object, and the order of its
keys carries no meaning. Paths are relative to the project root with `/` separators.

**A value is text as its language displays it.** `Observation.text` is the value as the
adapter's language shows it to a reader, and `items` are its items shown the same way:
`String(value)` in JavaScript, `str(value)` in Python, `fmt.Sprint(value)` in Go, `Display`
(else `Debug`) in Rust. The core
compares a literal on a
page with it character for character and knows no language's spelling of anything, so a
page is written as its language writes values: `true` in a TypeScript project, `True` in
a Python one. The one thing the core must know about a value's type is whether it is text
(only text can be what a fenced block copies, which is what `audit` looks for), and the
adapter says so with `"kind": "text"`. `description` is for messages and is never parsed.

**Order is the core's.** Wherever the core sorts (the symbols of the index, missing and
unexpected items) it compares strings by UTF-16 code units, in every language, so that a
report is the same whichever adapter served it. An adapter sorts nothing for the core.

## 3. Methods

### Language level (no project needed)

| method | params | result |
|--------|--------|--------|
| `hello` | `{ "protocol": 1, "warm": true }` | `Hello`: `{ "protocol": 1, "adapter": { "name", "version" }, "blockLanguages": string[], "sourceFiles": { "include": string[], "exclude": string[] }, "ignoredNames": string[], "capabilities": string[] }`, and optionally `"ignoredMarkers": string[]`, `"infoWords": string[]`, `"vocabulary": { … }` |
| `docComments` | `{ "files": [{ "page", "text" }] }` | `ExampleAttestation[]`: the examples the doc comments of those source files hold (an `@example` in JSDoc, a `>>>` session in a docstring), in order; files the language does not handle yield none |
| `assertions` | `{ "examples": [{ "lang", "code" }] }` | `boolean[]`: whether each example asserts anything |
| `docProse` | `{ "files": [{ "page", "text" }] }` | `[{ "text", "start", "end" }][]`: for each file, its doc comments as prose, in order |
| `mentions` | `{ "spans": string[], "symbols": string[] }` | `(string \| null)[]`: for each inline code span, the one of `symbols` it names |
| `expressions` | `{ "expressions": string[] }` | `[{ "shape": string, "names": string[] }]`: for each fact expression, its shape and its names |

`hello` is always first. A protocol number the adapter does not speak is a `user` error.
`blockLanguages` are the info-string languages whose fenced blocks the adapter runs, lower
case. `capabilities` lists the optional methods it implements: `tests`, `observables`,
`affected`. Everything in the answer is fixed for the adapter, not per project.

**`hello` is answered at once.** The answer is static, and the core may need nothing else
(an edit hook asks only whether the edited file is source code), so an adapter answers
before it loads its compiler or runtime. `warm` is the core's side of that bargain. It is
optional, and `true` when the core already knows that more requests follow whatever the
answer says; the adapter may then start loading what its other methods need as soon as the
answer is written, instead of when the first of them arrives. It is a hint about time only:
every answer is the same with it and without it, and an adapter may ignore it. Without it
the adapter loads nothing until a request needs it.

Every other method on this level takes many items and answers one for each, in the same
order: one round trip, however many pages or files there are. They are questions about
text, so they need no project and no session, and an adapter should answer them without its
compiler where it can.

`sourceFiles` says which project files are source files of the adapter's language: the
files whose doc comments may hold examples, and whose edits are edits to code. It is data,
not a method, so the core answers for any path without knowing the language and without a
round trip. The TypeScript adapter answers

```json
{ "include": ["**/*.{ts,tsx,mts,mtsx,cts,ctsx,js,jsx,mjs,mjsx,cjs,cjsx}"],
  "exclude": ["**/*.d.{ts,mts,cts}"] }
```

and the core applies any such answer like this:

- **The rule.** A file is a source file when its path matches at least one glob of
  `include` and no glob of `exclude`. Both lists are required and either may be empty; with
  an empty `include` the language has no source files.
- **The path** is the file's path relative to the project root, with `/` separators on
  every platform, no leading `./` and no empty, `.` or `..` segment. A file outside the
  project is never a source file, and the core does not look.
- **The globs** are globs as the config's `pages` and `sources` are, read the same way:
  picomatch 4 syntax, each matched against the whole path and never against the file name
  alone, so `*.py` names top-level files only and `**/*.py` names them at any depth. A
  leading `./` on a glob is dropped. A glob must not begin with `!`: leaving files out is
  what `exclude` is for.
- **Dotfiles.** `*`, `**` and `?` also match names that begin with a dot (picomatch's
  `dot: true`): a hidden file, or a file in a hidden directory, is a source file unless
  `exclude` says otherwise.
- **Case.** Letters match themselves only, on every platform and whatever the file system
  does, and a path is compared as it is spelled, with no Unicode normalisation. An adapter
  that takes `A.PY` for a source file lists that spelling.

The core uses the answer wherever it must tell code from everything else: to choose, among
the files the config's `sources` matches, the ones it sends `docComments` (in the working
tree, and in the project as it was at a git ref); to tell why a file stopped being a living
page; and to decide whether an edited file is worth an `impact`. An adapter must treat
exactly these files as its sources: `docComments` and `docProse` yield none for any other
file. The answers of `docComments` are one flat list, which the core groups by each
attestation's `page`.

`ignoredNames` say which files and directories are never the project's own, wherever they
are: installed dependencies, build output (`node_modules` for the TypeScript adapter;
`__pycache__`, `site-packages` and `*.egg-info` for the Python one). It is data for the
same reason `sourceFiles` is. Each entry is a glob for *one name*, never for a path: it is
read as the globs above are (picomatch syntax, dotfiles match, case as spelled), but
matched against a single path segment, so it holds no `/` and does not begin with `!`. The
rule: a project-relative path is ignored when any of its segments, directories and the
file name alike, matches one of them.

`ignoredMarkers` (optional) are names of files that mark the directory holding them as not
the project's own, whatever it is called: a Python virtualenv is the directory that holds
`pyvenv.cfg`. The rule: a path is ignored when a directory above it, below the project
root, directly holds a file with one of these names. The root itself is the project
whatever it holds.

Both rules are about one directory entry, which is the point of them: a walk decides at
the directory, from its name or from the names of the files in it, and never looks at
what lies below. The core applies them wherever it decides what the project's files are:
listing the working tree, listing the project at a git ref, and deciding whether an edited
file is the project's. Hidden directories (a name that begins with a dot) are tool state
in every language, and the core leaves them out by itself. `ignoredNames` is required and
may be empty.

**Info strings.** A fenced block's info string is its language, then words. The core reads
three (`check`, `compile`, `network`), ignores the ones the config lists in `ignoreTags`,
drops a bracketed title (`[Tab]`), and ignores every *attribute*: `key=value`, the value
quoted with `"` or `'` when it holds spaces, as documentation renderers write them
(`python title="x.py" hl_lines="1 2" check`). Any other word on a block in one of
`blockLanguages` is an error, so that a typo of `check` cannot pass in silence.

`infoWords` (optional) are words of the adapter's own. The core accepts them and passes
them on: an example whose info string holds some of them carries them in `tags`, in the
order written. An entry that ends in `=` names an attribute, passed on as `key=value` with
its quotes removed. What a word means is the adapter's alone; the core never looks at
`tags`. A word of the adapter's is not a mode: a block without `check` or `compile` is
still documentation. The Python adapter declares `continue`, by which a block goes on from
the blocks before it on its page as one interpreter session. It needs nothing else for
that: every example carries its `page` and `line`, and `open.examples` (and `compile`, and
`symbols`) name all the examples of the page.

`vocabulary` (optional) holds the few words of the core's sentences that belong to the
language. Wherever an adapter can give a finished string in an answer it does that instead
(`DeclaredSuite.kind`, `SurfaceResolution.package.display`); these are the ones the core
must put into a sentence of its own, and each has a default in no language's terms.

| word | used in | default | TypeScript adapter |
|---|---|---|---|
| `untitled` | a test's path, for an enclosing title that cannot be cited | `(untitled)` | `(computed title)` |
| `noTests` | `<file> declares no test "x"; <noTests>` | `it declares no test` | `it declares no test with a literal title` |

A word the core does not know is ignored, so a vocabulary may grow.

**`assertions`.** An example tagged `check` that asserts nothing runs and proves nothing;
the report counts it as weak. What an assertion looks like is the language's to say. The
core asks about every `check` block of the project in one request, giving each block's
info-string language as written and its code, and reads `false` as weak. Doc-comment
examples come from `docComments` with `weak` already decided. An adapter that cannot tell
answers `true`.

**`docProse`.** A source file is a page too: a claim may live in the description of a doc
comment, and `check --base` must find it again after an edit. For each file the answer
lists every doc comment, in order: `start` and `end` are its first and last line, counted
from 1, and `text` is its description (what comes before the first tag, in a language that
has tags) on one line, exactly as the `claim` of an example in that comment is written:
comments in the prose (`<!-- … -->`) replaced by a space, every run of whitespace collapsed
to one space, trimmed, markup kept. The core compares the two as strings.

**`mentions`.** `impact` lists the inline code spans of the pages that name an affected
symbol without proving anything about it. Whether `` `new Client()` `` names `Client`, and
whether `` `retry.py` `` names `retry` or a file, depends on the language. `spans` are the
texts of code spans as written on the page, `symbols` the affected names; the answer gives,
for each span, the symbol it names or `null`. A name is always part of the span that names
it, so the core sends only spans that contain one of the symbols, each once. An adapter
that cannot tell answers `null`.

**`expressions`.** When a fact's expression changes while the literal it defends does not,
`check --base` warns, unless the change is a rename: the same expression with other names,
the old ones gone from the code. The core sends both expressions (whitespace collapsed) and
decides from the answers. `shape` is the expression with every name replaced by a
placeholder, in a form of the adapter's choosing: two expressions have the same shape
exactly when they differ in nothing but names, and the core only ever compares shapes for
equality. `names` are the names in the order they occur. An adapter that cannot tell
answers the expression itself as the shape and no names; then nothing is a rename.

### Session level

`open` creates the one session of this process; every method below it needs one.

`open` params:

```json
{ "root": "/abs/project",
  "files": { "docs/a.md": "…" },
  "surfaces": [{ "name": "default", "entry": "./src/index.ts" },
               { "name": "client", "entry": { "import": "@acme/client", "path": "packages/client/src/index.ts" } }],
  "setup": "docs/docattest.setup.ts",
  "timeoutMs": 10000, "workers": 8, "network": false,
  "snapshots": { "dir": "docs/.snapshots", "data": { "regions": [] } },
  "manifests": { "package.json": { "engines": { "node": ">=22.12" } } },
  "examples": [] }
```

`root` is always present. `files` is present only for a project that exists in memory
(nothing of it is on disk: the base of a `check --base`, a test): it then holds every file
of the project by path, and nothing is read from `root`. `examples` is every example a
`compile` or `run` of this session may name: the well-formed examples of the pages being
checked, each with its `page` and `line`. An adapter may prepare for them (build one
compiler program around them) and may rely on a later `run` naming only examples that are
in it. It is empty for a session that runs nothing. The result is `{}`.

`surfaces` are the config's surfaces in its order, each `entry` exactly as the config wrote
it: a string, or an object with an `import` and a `path`, both strings. The core checks
that much and no more, and its config error says no more. What an entry means
is the adapter's to say: whether a string is a file of the project or the name of a
package, a module or a crate, what examples import it as, and which project files it is
read from. The core knows a surface by its `name` and nothing else, and reads no packaging
file (`package.json`, `pyproject.toml`, `Cargo.toml`) of any language to find one.

`manifests` are the project files that fact expressions name as `manifest("<path>")`, with
the path written out as a string literal: their data, by the path as the expression writes
it. The core reads and parses each by its format (`.json`, `.toml`, and `go.mod`, read for
its `module`, `go` and `toolchain` directives), knowing nothing of what a field means, and
every one is an object. The adapter gives fact expressions a function `manifest` that
returns the data of a path, in the language's own kind of value as `snapshot` does, and
evaluates a call for a path the session does not have to a failure whose message names it.
A manifest that cannot be read never reaches the adapter: the core reports each fact that
names it as an `error`, and does not ask for it to be evaluated.

| method | params | result |
|--------|--------|--------|
| `compile` | `{ "examples": ExampleAttestation[] }` | `Diagnostic[]` |
| `evaluate` | `{ "facts": FactAttestation[] }` | `{ [id]: Evaluation }` |
| `run` | `{ "examples": ExampleAttestation[], "cases": CasesAttestation[] }` | `Result[]` |
| `symbols` | `{ "attestations": Attestation[] }` | `{ [id]: SymbolRef[] }` |
| `exports` | `{ "surface": "default" }` | `string[]`, or `null` when the surface cannot be resolved |
| `describeSurfaces` | `{}` | `{ [surface]: SurfaceDescription }` |
| `tests` (capability) | `{ "file": "test/a.test.ts" }` | `TestListing` |
| `observables` (capability) | `{ "surface": "default" }` | `Observable[]`; a surface that cannot be loaded is a `user` error |
| `affected` (capability) | `{ "base": { "src/a.ts": "…", "src/new.ts": null }, "changed": ["src/a.ts", "src/new.ts"] }` | `SymbolRef[]` |
| `inCode` | `{ "names": ["LIMIT"], "files": ["src/a.ts"] }` | `string[]`: those of `names` the project's code still holds |
| `close` | `{}` | `{}`; the adapter releases its workers |

A method marked (capability) exists only when `hello.capabilities` lists it, and the core
asks only what is listed. Without `tests` a citation cannot be checked, so every
`<!-- test: … -->` is reported as an error on its own line (`test at L12 cannot be cited:
the <name> adapter does not read test files`); without `observables`, `audit` has
nothing to compare pages with and `inventory` is a user error; without `affected`,
`impact` is a user error.

`observables` reads values and calls nothing of a surface: a function may do anything, and
what it returns is not known until it runs. The setup module's named facts are the one
exception, since they exist for pages to read. The answer for the surface `default`, which a
fact without `@` reads, also holds each named fact that can be called without arguments,
called, and what lies below what it returns, each with the expression that reads it
(`helper()`, `helper().version`); a named fact that takes arguments, or whose name the
surface exports, is left out. Every value offered must print the same in every run: one
whose text shows an address is left out, and one whose order changes from run to run (a set
of strings in Python) is offered in a fixed order.

In a `TestListing`, `tests` are what can be cited and `suites` what may be cited by mistake
for a test. A suite says what it is in `kind`, with its article (`a describe block`,
`a class`): the core's message reads `"x" in <file> is <kind>, not a test`.

A `SurfaceDescription` says where a surface resolved. `resolution.package.display` is the
package and its version as the language writes them (`name@1.2.0`, `name 1.2.0`); the text
report prints it between brackets, and `--json` carries the resolution as it was given.

A `Diagnostic` is a reason an example cannot be accepted without running it: a type error
where the language has a compiler, a syntax error or an import that cannot be found where
it has not. The core reports it as `<message> (block line <n>)`, and an example in
`compile` mode with none as `compiles`.

`symbols` takes many attestations in one request so that the index costs one round trip.
In `affected`, `base` gives the contents at the base revision of every changed path
(`null`: the file did not exist), and everything else is as it is now. Every path in
`changed` has an entry in `base`.

`inCode` is the second half of the rename question (`expressions`): which of `names` still
occur as a name in the project's code. The code is the project files listed in `files` (the
ones the config's `sources` matches) together with the files the adapter reads the surfaces
from, which only it knows. The answer keeps the order of `names`. It is asked once per
`check --base`, and only if some fact kept its shape and dropped a name. An adapter that
cannot tell answers every name; then nothing is a rename.

A result's location is its `page` and `line`; the core refuses a result without them.

## 4. What the core must not ask, and the adapter must not decide

The adapter never sees a page, a directive, a target or an expected value it does not
need to run, and never decides a verdict about a fact: it observes, the core compares.
The core never parses source code, never evaluates an expression, and never knows a
language's syntax, what its files are called or how its projects are packaged. So the
adapter says, in `hello`, which files are source files and which names are never the
project's own; and it answers what a doc comment says (`docComments`, `docProse`), whether
an example asserts (`assertions`), which symbol a code span names (`mentions`), what an
expression's names are (`expressions`), which names the code still holds (`inCode`) and
what a configured surface is (`open`). It says which info-string words are its own, what a
suite is called and how a package is written. The core holds no list of adapters either,
and does not know how it was installed: the launcher of the language package says where
the adapter is and what command runs docattest. A core built today drives an adapter for a
language it has never heard of.

What stays in the core is what is the same in every language: Markdown, directives and
their syntax, the comparison of values as text, git, hidden directories as tool state, and
the file formats manifests are written in. The one thing the core finds in an expression is
docattest's own call `manifest("<path>")`, as text, so that it can read the file before
anything runs. A rule that would have to be written twice belongs in the core.

A base revision (`check --base`) is read with the adapter of the working tree, whatever
adapter the config committed there named: one adapter process serves one invocation.

## 5. Conformance

An adapter conforms when it passes the conformance suite, which is written for an adapter
in any language and knows none. The core carries it, so every docattest install can run it,
with nothing else to install:

```sh
docattest conformance run kit.json -- python3 /path/to/adapter
```

It starts the adapter as the core does and holds it to this file: `hello` (static, the same
warm, refused for another protocol number and sent again); the mistakes of §1, each an
`internal` error that leaves the adapter serving; the questions about text (one answer for
each item, in order; none for a file that is not source); every session method (a verdict
for each example at its own page and line, an answer for every fact, an entry for every
attestation in `symbols`, `null` for a surface that cannot be resolved); capabilities (a
method not listed is unknown, one that is listed answers); a stdout that carries nothing
but responses whatever attested code prints; answers in the order requests finish; `close`
waiting for what was sent before it; and the end of input. Every line the adapter writes,
and every line the suite sends, is held to the schemas of §2.

What the suite must be told about a language is the adapter's *kit*, a JSON file whose
shape is `schema/kit.json`: a small
project in memory (`files`, `surfaces`, `setup`), and labelled pieces of code, namely
examples that pass, fail, do not compile, assert nothing, print, and take half a second;
fact expressions with the text and items they display, and one that reads a field of the
suite's manifest; a source file with a doc comment;
a test file; a mention; an expression and a renaming of it; a change and the exports it
affects. `adapters/typescript/conformance.json`, `adapters/python/conformance.json`,
`adapters/go/conformance.json` and `adapters/rust/conformance.json` are the ones that exist, and any is the model for another.

Three more things hold the schemas and the implementations together. Each adapter's own
tests validate every message they exchange (`adapters/typescript/test/protocol/`,
`adapters/python/tests/`; the Go and Rust adapters' run the conformance suite, which does).
`node tests/schema.ts --candidate <core>` records what the core
really sends to each adapter on real projects (`docattest conformance record`) and
validates every line (`docattest conformance validate`). And a test in each package fails when its copy of the schemas differs
from the source.

**The core has one more bar**, which is not conformance and asks nothing more of any
adapter: what it prints must stay what is recorded. The core is docattest's only
implementation of everything above the protocol; a TypeScript implementation preceded it and
was retired once the core had matched it byte for byte, and its outputs were kept as
recordings (`tests/parity/recordings/`). `tests/parity/bench.ts` runs the core, with the
TypeScript adapter, on every benchmark scenario and on real projects, and the other
harnesses in `tests/parity/` on the corners, and each compares what it prints and writes
with those recordings; `tests/parity/bench.ts` also compares its run time with the medians
the reference took. A deliberate change of behaviour is made in the core and then recorded
again with `--update`, which says which cases changed
([tests/README.md](../tests/README.md)).
