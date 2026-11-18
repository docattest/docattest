# docattest — framework specification

`docattest` makes prose answer to CI. A documentation page carries its own proof, the build
fails at the page and line where the prose is no longer true, and the same information
links every public symbol back to the pages that describe it.

Status: implemented (0.1.0, TypeScript adapter). Judged against [ACCEPTANCE.md](ACCEPTANCE.md)
and [CORPUS.md](CORPUS.md). The authoring convention is [PATTERN.md](PATTERN.md); this file is
the machinery. Where this file and `docs/` disagree, `docs/` is what runs; fix this file.

Examples throughout use an imaginary library `@example/client` with `createClient`,
`retry`, `RETRY_DEFAULTS`, `ErrorCodes`, `validateConfig`, `Region` and a server that reads
environment variables listed in `ENV_KEYS`. Substitute your own.

## 1. Principle

Docs drift because the expectation lives somewhere other than the prose. Mechanisms
that bridge that gap from outside (markers, test citations, hashes of test bodies)
cannot distinguish a refactor from a behaviour change and become noise; on one real
history such a mechanism fired 268 times and the docs moved zero times afterwards.
Execution can distinguish them. So:

> The expectation lives in the page. CI runs the page. A failure names the line.

Nothing is rendered, generated or acknowledged. A page is true, false at a given line,
or uncovered. And because every proof names the symbols it exercises, the parse that
checks the docs also yields the map from code to docs.

## 2. Vocabulary

- **Page**: a Markdown file under check. Config decides which pages are *living* (checked)
  and which are *frozen* (listed, never checked). Doc comments in source are pages too.
- **Attestation**: one unit of proof. Kinds: `example`, `fact`, `cases`, `snapshot`, `test`. Each
  has a location (file, line), a verdict (`pass`, `fail`, `error`, `skipped`), and the
  set of **symbols** it exercises.
- **Surface**: an importable entry point attestations may reference, normally the
  published package. Facts may observe only what a user of the surface could observe.
- **Adapter**: the language-specific part. Turns attestations into executable cases,
  maps results back to locations, and resolves which surface symbols each attestation
  touches. The core is language-agnostic.
- **Setup module**: one file per repo exporting named case runners for tables (`cases`) and
  named fact helpers (`facts`). The only docattest-specific code outside pages.
- **Index**: the derived map `symbol → [{page, line, kind, claim}]`, and its complement,
  surface symbols with no attestation anywhere.

## 3. Attestation kinds

### 3.1 `example` — a fenced block that compiles and runs

    A request that keeps failing is retried, and the last error is the one thrown.

    ```ts check
    import { retry } from '@example/client';
    let calls = 0;
    await assert.rejects(
      retry(async () => { calls++; throw new Error(`attempt ${calls}`); }, { maxAttempts: 3, sleep: async () => {} }),
      { message: 'attempt 3' },
    );
    assert.equal(calls, 3);
    ```

- Info string: `<lang> check` compiles and runs; `<lang> compile` compiles only; no tag
  means documentation only and counts as uncovered.
- Renderer words in the info string (`twoslash`) are accepted on a `check` or `compile`
  block only when listed in the config's `ignoreTags`; bracketed titles (`[TypeScript
  SDK]`) are always ignored. On an untagged block a renderer word leaves it documentation,
  but a word that is likely a misspelling of `check`, `compile` or `network` (within a
  third of the tag's length in edits, at least one, ignoring case) on a block in a language
  the adapter runs is an `error` naming the tag, so `ts chekc` fails instead of silently
  becoming an untagged block.
- `assert` (the host language's standard assertion) is in scope. Assertions are the
  proof. A `check` block without an assertion runs as a smoke test and is reported as
  *weak* coverage, never as a pass of a claim.
- Lines above `// ---cut---` are compiled and run but meant to be hidden by the
  renderer (the twoslash convention). Hidden lines are setup, never the behaviour under
  test; PATTERN.md says how to tell.
- Each block runs in its own module with a fresh import of the surface. No state is
  shared; order never matters.
- Network is disabled unless the block says `check network`; such blocks run only when
  the environment allows (`DOCATTEST_NETWORK=1`) and are otherwise `skipped`, visibly.
- Per-block timeout, default 10 s (`runtime.timeoutMs`). A timeout is a `fail`, and it is
  enforced, not observed: the block's worker is terminated, so a block stuck at an `await`
  or in a busy loop stops, its timers never fire, and a fresh worker takes its place.
- An error that escapes the block (a rejection nobody awaited, an exception thrown from a
  timer) while it runs fails it: `unhandled error from attested code: …`.
- Fenced blocks are found at any depth: a `check` block inside a list item or a block
  quote is an example like any other.
- `import.meta.url`, `import.meta.filename` and `import.meta.dirname` refer to the block's
  virtual file, which sits next to its page (`docs/__docattest_<page>_L<line>.ts`).

#### Blocks the core runs itself: `console check` and `md check`

In any language, two block kinds are run by the core, reported as `example`, and never
sent to an adapter ([console.md](../console.md) holds the rules and their proofs):
`console check [cwd=<dir>]` is `$ ` commands, each followed by what it prints (stdout then
stderr; `...` for any lines, a last `[exit N]` for the exit code), each run with `sh -c`
in its own process group, killed at `runtime.timeoutMs`; `docattest` in a command runs the
core's own executable, and `DOCATTEST_CONSOLE_RUNNING` stops a block that checks its own
page. `md check fixture=<dir> [as=<path>]` checks the block as a page of a temporary copy
of the fixture, in process, with the fixture's config and adapter.

### 3.2 `fact` — a literal in the prose asserted against the code

    A failing call is made at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->

    The server reads these environment variables: <!-- fact: ENV_KEYS -->
    - `API_TOKEN`
    - `REGION`
    - `PORT`

- Syntax: `<!-- fact[ @surface][ ordered][ column=<name> | field=<path>]: <expression> -->`. Invisible when
  rendered. The adapter evaluates the expression against the named surface (default if
  omitted) and compares it to the **target**. A directive may span lines; it binds from the
  line where it ends, and results report the line where it starts. A `<!-- fact` or
  `<!-- cases` that is never closed is an `error`, not text.
- Target binding is positional and strict. The deciding question is whether the
  directive ends its line and what the next line starts:
  - directive ends its line and the next line starts a list → **set** compare over items
    (first inline code span of each item, else item text);
  - …next line starts a table, with `column=<name>` on the directive → **set** compare
    over that column;
  - …next line starts a fenced block → **text** compare, whitespace-normalised;
  - otherwise, the nearest inline code span, bare number or bare version (`1.2.3-rc.1`,
    or `v1.2.3`, read as `1.2.3`) before the directive on the same line → **scalar**
    compare;
  - …unless that is a code span ending a sentence of two or more (counted from the
    previous sentence end or comment in the same paragraph, list item or cell) → **set**
    compare over the sentence's code spans when the value is a list (`ordered` allowed),
    and a scalar compare with the last of them when it is not;
  - none of these, and the directive ends its paragraph (or heading, or HTML block) with a
    list or fenced block right after it, blank lines or not → that list or block, as for
    the next line ("The keys are: `<!-- fact: … -->`", a blank line, a list);
  - none of these → `error`: "fact at L12 has no target". Fail closed.
  Prose may precede the directive on its line ("The server reads: `<!-- fact: … -->`")
  without turning a list binding into a scalar one. A directive with `column=` can only
  mean a table, so it binds the next table across blank lines, and a code span earlier
  on its line never makes it a scalar; with no table following it is an `error`. A column
  name with spaces is quoted (`column="route path"`, or single quotes); a bare name runs to
  the next space, and the error for the word after it says how to quote.
- A directive with `field=<path>` binds one value of the next fenced block, across blank
  lines and the directives stacked above it, as `column=` binds a table. The block's
  language is `json`, `toml`, `yaml` or `yml`, and it is parsed as a manifest of that format
  is. The path is keys parted by dots; a key of digits indexes a list; a key with a dot or
  space is quoted (`limits."max retries"`). A string, number, boolean or null value is a
  **scalar** compare; a list of them is a **set** compare (`ordered` allowed). An object, a
  list of non-scalars, another language, a block that does not parse and a path that is not
  there are each an `error` at the fact, naming what is there instead.
- Inside a table cell a fact binds within that cell: the nearest code span, number or
  version before it, or the cell's sentence of code spans, as above. (Cells of a table bound by `cases` are row data, so comments
  in them are never directives.)
- Lists and table columns compare item values: the first inline code span of an item, or
  every inline code span of a cell (a cell may group several, `` `A`, `B` ``), else a bold
  lead term, else the visible text.
- **Named facts.** The setup module may export `facts`, an object of helpers; an expression
  may call them by bare name: `<!-- fact column=Field: errorFields() -->`. The helper reads
  the surface and returns a value; the expectation stays on the page. The setup module is
  loaded for a fact only when its expression uses a name the surface does not export, so
  ordinary facts never run the setup module's top level. A name that is both a surface
  export and a setup fact is an `error` (fail closed), detected from the setup module's
  syntax where it can be and at evaluation otherwise.
- Facts of one page are evaluated against one surface in one fresh module registry, each
  with its own time budget.
- Comparison: scalars as strings after `String(value)`; sets order-insensitive unless
  `ordered`; text after collapsing whitespace. Messages carry both sides:
  `missing: LOG_LEVEL (3 items on the page, 4 in ENV_KEYS)`: what differs first, the counts
  only when they differ. An unordered set counts distinct values (a column may repeat one),
  and an empty value shows as `""`. A number or version compares without a leading `v`
  (`v1.2.3` is `1.2.3`). A block compares token by token (a run of letters, digits, `_` and
  `$`, or any other single character), so whitespace counts only where it parts two runs.
  A block that differs names the first line of the block where it does:
  `the block differs from SHAPE at line 3 (whitespace ignored): expected "y: number", got "y: string"`.
- A fact never rewrites the page. The fix is a human or an agent editing the prose.

### 3.3 `cases` — a decision table that runs

    <!-- cases: validateConfig -->
    | case                      | config                              | result                     |
    |---------------------------|-------------------------------------|----------------------------|
    | unknown key is rejected   | `{ region: 'eu', bogus: 1 }`        | error at `bogus`           |
    | region must be known      | `{ region: 'mars' }`                | error at `region`          |
    | minimal config is valid   | `{ region: 'eu' }`                  | valid                      |

- `<!-- cases: <runner> -->` at the end of a line before a table binds it to a runner
  exported from the setup module: `export const cases = { validateConfig(row) { … } }`.
  Like `column=`, the directive can only mean a table, so blank lines may separate them;
  anything else in between, or no table at all, is an `error`.
- Header cells become row keys. Each row is one test named `<page>:<line> <first cell>`.
  The runner receives each cell as written (code spans and comments kept, `\|` unescaped),
  unlike a fact over a column, which compares a cell's code spans; it throws to fail. Each row has its own time budget; a row that times out fails and the
  table resumes at the next row on a fresh worker.
- Unknown runner → `error`. Header missing a column the runner declares
  (`validateConfig.columns = ['config', 'result']`) → `error` before any row runs. Rows
  are the expectations and live in the page; the runner is glue and may not hold
  expectations.

### 3.4 `snapshot` — a pinned fact about the outside world

    docs/.snapshots/regions.json
    { "source": "https://api.example.com/regions", "fetchedAt": "2026-10-29", "data": { … } }

- Facts and cases may reference `snapshot('regions')`, which resolves to `data`.
- `docattest refresh [name]` refetches from `source` and rewrites the file; the diff is
  reviewed like code. Nothing else touches the network.
- A snapshot older than `snapshots.maxAgeDays` yields a `warning` so the pin's own
  staleness is visible without failing the build.

### 3.5 Doc comments

An `@example` block in a doc comment whose first line is `// check` or `// compile` is an
`example` located at the comment's file and line. Reference generators copy these into
the published reference, so the reference inherits the proof, and the comment's symbol
is the attestation's symbol for the index.

### 3.6 `test` — a citation of an existing test

    A call that keeps failing throws the last error. <!-- test: test/retry.test.ts > "throws the last error" -->

- Binds the sentence before it to a test the project already has. `pass` when the file
  declares a test with that title (enclosing `describe` titles may be given as further
  `> "…"` segments), `error` when the file or the title is missing or the directive is
  malformed.
- The test is not run: the project's own test job does that. A citation cannot see a test
  that is skipped, failing or weakened, so it is reported as its own kind and counted
  separately (`cited`) in coverage. See `docs/citations.md`.

## 4. Living and frozen

```json
{
  "pages":     ["docs/**/*.md", "README.md"],
  "frozen":    ["docs/decisions/**", "docs/changelog/**"],
  "sources":   ["src/**/*.ts"],
  "surfaces":  {
    "default": "@example/client",
    "server":  { "import": "@example/server", "path": "packages/server/dist/index.js" }
  },
  "setup":     "docs/docattest.setup.ts",
  "snapshots": { "dir": "docs/.snapshots", "maxAgeDays": 90 },
  "adapter":   "typescript",
  "runtime":   { "timeoutMs": 10000, "workers": 8 },
  "ignoreTags": ["twoslash"]
}
```

- Globs are matched with picomatch, dotfiles included, relative to the project root. A
  list of globs only adds files: a glob that starts with `!` (picomatch's negation, which
  matches every file it does not name) is a config error, and `frozen` leaves pages out.
- The project's files are those below the root, less hidden directories, what the
  adapter's language ignores, and, in a git work tree, what git ignores and does not track.
- A glob of `pages` or `sources` that matches no file, a `setup` that is not there or
  that nothing reads (no living page has a cases table or a fact), and a glob of
  `undocumented` that matches no symbol its surface exports, are warnings in the report's
  `warnings`: they never change the exit code.
- A **surface** is one of three forms. A bare specifier (`@example/client`) is imported as
  itself and resolved the way Node resolves it from the project root (import conditions are
  used when a package has no `require` export). A `./path` is a file in the project,
  imported under the `name` in `package.json` (`<name>/<surface>` for surfaces other than
  `default`; `docattest-fixture` for in-memory projects). The object form names both:
  examples and facts import `import`, and it resolves to `path`, so a monorepo can keep one
  config at its root and still point a package's pages at that package's build. A built
  `.js` entry is type-checked against its sibling `.d.ts` when there is one.
- `adapter` names the language adapter: any name of lower-case letters, digits and `_`
  that starts with a letter. No list of adapters is kept, so the name is not checked
  against one; whether such an adapter exists is found out when a command first needs the
  language (section 6), and not finding it is a user error.
- `runtime.workers` sizes the worker pool; the default is `os.availableParallelism()`.
- The config is validated in one pass: every problem is reported at once, by key, and
  unknown keys are errors, so a misspelt key never silently does nothing.
- `members` lists directories below the project that hold projects of their own, in any
  language. `check` without pages checks the project, then each member with its own config
  and adapter (and the members' members), and reports them one after another with one exit
  code. A member's adapter is found as any adapter is, unless its entry gives an
  `adapterCommand`, read from the directory of the config that lists it: a launcher can
  start only its own language's adapter. See `docs/cli.md`.

- Living = `pages` minus `frozen`. A living page with no attestations is **uncovered**:
  reported, not failed. Frontmatter `docattest: false` freezes one page from inside
  (frontmatter is YAML; invalid YAML leaves the page living).
- Pages are CommonMark with GitHub extensions (tables and the rest), parsed with mdast;
  every line number comes from the parser's positions.
- **Checking some pages.** `check({ pages })` and `docattest check <path or glob>…` run only
  the attestations on the matching living pages; patterns are relative to the project root.
  Coverage still describes every living page (pages not run count the results they would
  produce), so checking one page never makes the rest look uncovered. A pattern that
  matches no living page is an error, and so is one that matches only frozen pages.
- Frozen pages are never parsed. Their count is shown so coverage describes maintained
  docs only; the text report names the uncovered pages (wrapped to 100 columns, and only
  when every page ran) but never the frozen ones, which the report object lists.

## 5. Duality: the index

Every attestation knows its symbols. The adapter resolves identifiers imported from a
surface in examples, identifiers in fact expressions, and the owning symbol of a doc
comment. The core keeps `symbols: string[]` per attestation and derives:

- **`docattest where <symbol>`** — pages and lines that attest the symbol, each with its
  *claim*, chosen in this order:
  1. a directive after prose in its paragraph, heading or list item claims that prose,
     up to the directive (a fact whose sentence carries the literal);
  2. otherwise, the paragraph or heading immediately before the attestation's block in
     the same container: within a list item or a quote, not the page around them. A
     fenced block, a directive on its own line and a directive in a table cell (whose
     block is the table) claim this way;
  3. otherwise, for a directive in a table cell, the cell's own text.
  The claim is what an agent reads to know what the proof defends. This is the
  code-to-docs link, and it is never stale because it is computed from the same parse
  that checks the docs.
- **`docattest uncovered --surface [name]`** — public symbols of a surface with no
  attestation against that surface. The dual of page coverage: a documentation backlog by
  symbol. The config's `undocumented` gives, by surface, globs of the names that are not
  meant to be documented; those no page attests are left out of the backlog and counted
  apart, and one a page attests counts as attested. Coverage is keyed by surface and name: a name attested on one surface says
  nothing about another surface that exports the same name. An example's symbols belong to
  the surface it imports them from; a fact's to the surface it is evaluated against (and,
  for a named fact, to the surface identifiers the helper's code imports); a cases table's
  to the surface identifiers its runner imports; a doc-comment example's to the surfaces
  whose export resolves to the commented declaration (or, when a surface points at built
  output, to every surface exporting the name).
- **`docattest index --json`** — the whole map, for editors, reference generators and
  skills. Cheap: parsing only, no execution.

- **`docattest impact [--base <ref>]`** — from the difference between a git ref (default
  `HEAD`) and the working tree to the prose it touches: the exports whose declaration
  changed or that reach a changed declaration through the code they use, everything
  `where` lists for them, and pages that only mention them in a code span (unproven). A
  cited test is listed as cited, not proven: docattest checks it exists, never that it
  passes. Comments and formatting are not changes. See `docs/impact.md`.
- **`docattest audit`** — the other blind side: literals on living pages that look copied
  from a surface and carry no directive, as suggested facts (`match`) or probable drift
  (`drift`). Advisory, always exit 0, precision over recall. See `docs/audit.md`.

The index is not committed (criterion 10). It regenerates in seconds, so tooling runs it
on demand. A reference generator may embed "attested in page L12" links at build time.

For agents the workflow is: before changing an exported symbol, `docattest where` it and
load those pages, because they are the prose the change will affect and they are
unlikely to be in context otherwise. After the change, `docattest check <page>` for each
of them, then `docattest check`, names the sentences that broke. PATTERN.md spells this
out, and `docattest skill install` puts a condensed version where coding agents look for
skills (`.claude/skills/docattest/SKILL.md`, or `~/.claude/…` with `--global`; an edited
copy is never overwritten without `--force`).

docattest is not tied to one agent. What it asks of any agent is two plain commands,
`docattest impact --file <path>` after an edit (silent, exit 0, when the edit touches no
documented export; exit 1 when the path, read from the working directory, is outside the
project or names no file that is there or in the diff) and `docattest check` before finishing; neither reads an agent's event
format, and the project root is found by walking up from the working directory (and from
the edited file) to `docattest.config.json`, so no agent's variable is needed to say where
the project is. `docattest agents install` writes those two instructions into a marked
section of `AGENTS.md`, and `docattest hooks install --git` adds a pre-commit hook that
runs the check; together they are the baseline, and need nothing of the agent. For an
agent that has hooks, `hooks install --agent <name>` and the hook modes (`impact --hook`,
`check --hook`) apply an *agent descriptor*: one JSON file per agent
(`protocol/agents/<name>.json`, format in `protocol/schema/agent.json`) that says where the
agent's settings are and how hooks nest in them, which event runs which action, where the
event holds the edited file and the sign that the agent is already continuing, what JSON
the answer is, how the agent writes the project root in a command, and where it looks for a
skill. Claude Code is the first descriptor and the default; no implementation names an
agent anywhere else, and a user supplies a descriptor for another agent as
`--agent ./my-agent.json` (`docs/agents.md`).

## 6. Architecture

```
core (language-agnostic)                adapter (per language)
──────────────────────                  ───────────────────────
parse pages + doc comments      ──►     compile(examples) → diagnostics
bind facts to targets                   evaluate(facts) → observations
load snapshots                          run(examples, cases) → results
plan: attestations[]            ◄──     symbols(attestation) → {name, surface}[]
                                        exports(surface) → string[]
index: symbol → attestations            tests(file) → TestListing
report: text / json, coverage
exit code
```

Adapter contract, six functions:

- `compile(examples): Diagnostic[]` keyed by attestation id.
- `evaluate(facts): Map<id, Evaluation>`; pure, no side effects. An evaluation is either
  an observation of the value (`String(value)`, its items when it is a list, a short
  description) or the reason there is none. Observations are plain data so they can cross
  a thread boundary; the core compares them with the target.
- `run(examples, cases): Result[]`; isolated execution.
- `symbols(attestation): {name, surface?}[]`; surface identifiers the attestation exercises,
  each with the surface it was reached through.
- `exports(surface): string[] | undefined`; public names a surface offers, for surface
  coverage and `uncovered`. Undefined when the surface cannot be resolved.
- `tests(file): TestListing`; the tests a file declares, with their enclosing suites, for
  citations.

Around the contract, a **language plugin** tells the core which fenced-block languages it
runs, which source files may hold doc comments and what examples they hold, and creates an
**adapter session** per check. The session adds two things the core needs beyond the six
functions: `describeSurfaces()`, how each surface resolved (for the report), and `close()`,
which releases workers. Two more are optional: `observables(surface)`, the shallow values a
surface offers with the fact expression that reads each (for `audit`), and
`affected(change)`, the exports a source change reaches (for `impact`). The adapter server
loads the plugin on demand; nothing else in the package imports a compiler, and lint
enforces it.

First adapter: TypeScript (the peer dependency `typescript` >= 5.4 < 7; TypeScript 7, the
native port, has no compatible JavaScript API yet). A compiler program over the project's
files does `compile`, `symbols` and `exports`. Examples, facts and cases run on a pool of
`worker_threads`, reused from page to page, with one fresh module registry per block, per
cases table and per page of facts, so docattest needs no test runner and adds none to the
project. Workers never load the compiler: when attested code requires a project module,
the worker asks the session synchronously and receives CommonJS; dependencies load
natively. Results are collected per attestation and ordered by page and line, so the
number of workers never changes the report. Second adapter: Python. The core never
imports a language.

An adapter may also be a separate program, written in the language it serves and driven over
the protocol in [PROTOCOL.md](../../protocol/PROTOCOL.md). The docattest core (`core/`) runs
every adapter this way, TypeScript included (`docattest-adapter-typescript`, which this
package installs; `adapters/typescript`), Python (`adapters/python`), Go
(`adapters/go`) and Rust (`adapters/rust`). The core is never
installed or run by itself: each language package provides the `docattest` command as a
launcher, which finds the core (`DOCATTEST_CORE`, the binary the package brought, or a
per-user cache shared by every language package), tells it how to start the package's
adapter (`DOCATTEST_ADAPTER_<NAME>`) and what command runs docattest in the project
(`DOCATTEST_COMMAND`, which `hooks install` and `agents install` write down, with `{root}`
for the project root), and then becomes it. This package's launcher is its `docattest`
command (`adapters/typescript/src/launcher/`), and its programmatic API (`createProject`)
runs the same core the same way: an in-memory project is written to a temporary directory
for each call, and a base given as files is committed there with git. The core is
docattest's only implementation; a TypeScript implementation preceded it and was retired
once the core matched it byte for byte, and its outputs are kept as the recordings the core
is held to ([tests/README.md](../../tests/README.md)). Wording and conventions that belong
to a language (what a suite is called, how a package and its version are written, how a
value is displayed, which names are never the project's own) are the adapter's, given to the
core as data; the shapes are published as JSON Schema in `protocol/schema/`, and
`docattest conformance run` holds an adapter in any language to them.

Source layout, for contributors: the core is `core/src` (its modules are listed in
`core/README.md`). The npm package's `adapters/typescript/src/` holds `adapters/typescript`
(program, symbols, surface resolution, the session's worker pool in `run`, and the worker
side in `worker`), `protocol` (the adapter server), `launcher` (the `docattest` command:
finding the core), `client` (`createProject`, running the core), `api` (public types and
constants), and `files` and `util` (file sources, globs and helpers the adapter uses).

## 7. Correctness rules

1. **Fail closed.** Ambiguous binding, unknown runner, missing surface, unparsable
   expression: `error`, and the build fails. A typo never silently uncovers a page.
2. **Execution, never change detection.** No hashes, lockfiles or history. Whether a page
   is true depends only on the current page and the current code. Whether a proof was
   removed or weakened is a property of a change, so `check --base <ref>` reads the base
   from git on demand and reports `orphaned-claim` (fail), `frozen-page`, `downgrade` and
   `weakened` (warnings). Nothing is stored. See `docs/changes.md`.
3. **Public observation.** Facts and examples use surfaces users can import. Internal
   constants are either exported or not asserted.
4. **Isolation and determinism.** One module per example, fresh surface import, network
   off, timeouts on. Same inputs, same verdict.
5. **Locations are mandatory.** Every message names page and line.
6. **No generated artefacts in the repo.** Compiled tests and the index are temporary.
   Snapshots are the only committed by-product and only `refresh` writes them.
7. **No model calls.** Judgement is by running code.

## 8. Report

    docs/errors.md
      ✗ L24 fact: missing: AUTH_ERROR (9 items on the page, 10 in ErrorCodes)
    docs/config.md
      ✗ L31 example: AssertionError: expected false, got true  (assert.equal, block line 4)
    results: 116 pass, 2 fail, 0 error, 0 skipped
    coverage: 42 living pages, 31 covered (118 attestations: 104 proofs, 2 of them weak, and 14 citations), 11 uncovered, 15 frozen
    surface default: 57 symbols, 44 attested, 13 not attested by any page
      @example/client → node_modules/@example/client/dist/index.js (@example/client@4.2.0)
    snapshots: regions (48 days)
    FAILED

`--quiet` prints only what needs attention: the `fail` and `error` results under their
pages, each config warning as `warning: …`, each surface warning as
`surface <name>: warning: …`, the stale snapshots, and with
`--base` the changes, if any; then the verdict. A project where everything holds prints
`ok`. With members, a project that needs nothing is left out, and one that does is printed
under its `member <path> (<adapter>)` line.

`--json`: `{ results: [{page, line, kind, status, symbols, expected, actual, message}],
coverage, surfaces, snapshots, exitCode }`, where `surfaces` is keyed by surface name and
each entry is `{ symbols, attested, unattested, undocumented?, resolution?, warnings }`
(`undocumented` only when the config gives the surface a glob). `resolution` is
`{ import, entry, package?: { name, version? } }`: the specifier, the entry file it
resolved to (project-relative when inside the project) and the package that file belongs
to (nearest `package.json` with a name). Unattested symbols are reported as a backlog
("not attested by any page"), not as a defect.

A surface **warning** never changes the exit code. One is emitted when a surface did not
resolve, and when a bare-specifier surface resolves to an installed package directory that
is not the workspace package of the same name (not a symlink to it) and whose version
differs from it: the pages would otherwise be checked, silently, against old code. Workspace
packages come from `workspaces` in the root `package.json` and from `pnpm-workspace.yaml`.
The TypeScript adapter also follows each surface's imports through the project's files, and
warns for every workspace package they reach through a `node_modules` link into the project
whose build is missing (it does not resolve) or older than one of its TypeScript files: the
runtime loads the build, so the pages would otherwise be checked against code its sources no
longer hold.

Exit 1 on any `fail` or `error`; `skipped` and warnings never change the exit code. The
command line also exits 1 for a mistake in the request or the project (an unknown page, an
invalid config), with a message and no stack, and 2 for an internal error, which is a bug
in docattest and is printed with its stack.

## 9. CI placement

One blocking job on merge requests and the default branch, after the project's build,
before any site build. Budget: under a minute beyond compilation.

## 10. Non-goals

Rendering or generating documentation. Tracking whether docs changed. Judging prose
quality. Checking rationale. Replacing the reference generator. Language models.

## 11. Open decisions

- Table runners: rows as strings, or columns typed in the header. Decided: strings, as
  written.
- `await` inside facts for async surfaces. Decided: yes; each fact has its own budget.
- Whether `compile`-only examples count toward coverage. Leaning weak coverage.
- Whether `docattest where` should also search frozen pages read-only, for discovery.
  Leaning yes, labelled frozen.
