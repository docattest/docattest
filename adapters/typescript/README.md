# docattest

**Documentation that CI can fail**, for TypeScript and JavaScript projects.

Try it in your browser first: the [playground](https://docattest.github.io/docattest/playground.html)
lets you break a small project's code and watch docattest catch its docs.

A Markdown page carries its own proof: examples that run, values in the prose that are
compared with your code, tables whose rows are tests, and citations of tests you already
have. When the code changes and a sentence stops being true, `docattest check` fails at that
page and line. Here the page said a call is tried `3` times and listed two environment
variables; the code now says 5, and reads a third:

```console check cwd=docs/.fixtures/acme-client
$ docattest check
docs/retries.md
  ✗ L1 fact: expected 3, got 5 from RETRY_DEFAULTS.maxAttempts
  ✗ L3 fact: missing: LOG_LEVEL (2 items on the page, 3 in ENV_KEYS)
results: 0 pass, 2 fail, 0 error, 0 skipped
...
FAILED
[exit 1]
```

This is the npm package. The same tool for Python is the PyPI package `docattest`, and
for Go a module that `go install` brings ([adapters/go](../go/README.md)), and for Rust the
crate `docattest` ([adapters/rust](../rust/README.md)); all run one shared core. The project's front page is the
[repository README](../../README.md), and the full documentation is in
[docs/](../../docs/index.md).

## Install

```sh
npm install --save-dev docattest typescript
```

Node 22.12 or later. <!-- fact: manifest("adapters/typescript/package.json").engines.node.replace(">=", "") -->
TypeScript from `5.4` up to (not including) `7` <!-- fact ordered: manifest("adapters/typescript/package.json").peerDependencies.typescript.match(/[\d.]+/g) --> is a
peer dependency: examples are type-checked with your project's own compiler.

The install brings the docattest core, a small native program, as the one optional
dependency of these that is built for your machine: <!-- fact: Object.keys(manifest("adapters/typescript/package.json").optionalDependencies) -->
- `@docattest/core-darwin-arm64`: macOS on Apple silicon
- `@docattest/core-darwin-x64`: macOS on Intel
- `@docattest/core-linux-arm64`: Linux arm64, glibc or musl
- `@docattest/core-linux-x64`: Linux x86-64, glibc or musl
- `@docattest/core-windows-x64`: Windows x64

There is no install script, and nothing is downloaded when docattest runs. If your install
leaves optional dependencies out (`--omit=optional`, or a lockfile written on another
platform), `docattest` says so and how to fix it.

## Quick start

1. Run `npx docattest init` at the project root to generate `docattest.config.json` and run
   an initial gap audit (or create it by hand):

   ```json
   {
     "pages": ["docs/**/*.md"],
     "surfaces": { "default": "./src/index.ts" },
     "adapter": "typescript"
   }
   ```

   The *surface* is your public API: what pages may import (under your package's name) and
   what facts may read.

2. Run `npx docattest audit --apply` to bind matching claims automatically, or add proofs to
   a page such as `docs/retries.md`:

   ````md check fixture=docs/.fixtures/acme-client as=docs/retries.md
   A failing call is tried at most `5` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->

   ```ts check
   import { RETRY_DEFAULTS } from '@acme/client';
   assert.ok(RETRY_DEFAULTS.backoffMs > 0);
   ```
   ````

3. Run `npx docattest check`. It exits 1 when a page and the code disagree.

## What TypeScript brings

| Proof | You write | It fails when |
|---|---|---|
| **Example** | a block tagged `ts check` (runs, with `assert` or trailing `// Output:` / `// Unordered output:` comments) or `ts compile` (type-checks only); `js`, `tsx` and friends too | it throws, an `assert` or expected output fails, or it no longer type-checks |
| **Fact** | `<!-- fact: expression -->` after a value, or above a list, table column or code block | the value on the page differs from `String(value)` of the expression |
| **Cases** | `<!-- cases: runner -->` above a table, with the runner in your setup module | a row's expected result is not what the runner produces |
| **Citation** | `<!-- test: test/retry.test.ts > "describe title" > "test title" -->` | the file no longer declares an `it` or `test` with that title |
| **Doc comment** | an `@example` whose first line is `// check` or `// compile`, in a file under `sources` | as for an example |

Every example is its own module with a fresh import of your package, runs in a worker with a
time limit, and has the network off: a `check network` block runs only when
`DOCATTEST_NETWORK=1` is set, and is skipped otherwise. The time limit counts from when the
worker is ready to run the example: starting a new worker is not counted. <!-- test: adapters/typescript/test/unit/language/run/pool.test.ts > "runJob" > "does not count a new worker's start-up against the job's time" --> Lines above `// ---cut---` run but
are setup that renderers can hide. A surface is an entry file (`./src/index.ts`), a package
name resolved as Node would, or both (`{ "import": "@acme/client", "path":
"packages/client/dist/index.js" }`), which is how a monorepo points a page at one package's
build.

A fact expression reads the surface's exports by their names, `snapshot("name")`,
`manifest("path")`, `file("path")`, `matched("path", "regex")` and
`manifestField("path", "keys")`. `manifest` gives the parsed data of a project file such as
`package.json`, so a claim about what the package declares needs no setup code. The
versions at the top of this page are read so ([manifests](../../docs/manifests.md)). `file`
gives any other project file's text, a string, for its own methods and regular
expressions: this package is released under the `MIT` license. <!-- fact: file("adapters/typescript/LICENSE").match(/^(\w+) License/)[1] -->
`matched` and `manifestField` take one value out of a file in one call, worked out by the
core ([manifests](../../docs/manifests.md#one-value-of-a-file-in-one-call)): what a regex
matches, a string, and the value at a path of keys of a manifest. A regex is a string, its
backslashes doubled: the license is `MIT`, <!-- fact: matched("adapters/typescript/LICENSE", "^(\\w+) License") -->
for Node `>=22.12`. <!-- fact: manifestField("adapters/typescript/package.json", "engines.node") -->

### Facts read constants from source

A program's most documented values, its port or its default timeout, are often not
exported, or live in an entry script that runs when it is imported. So a fact may also read
a constant that the surface's module declares at its top level, exported or not. Its value
is read from the module's source, and a fact that reads only constants never loads the
module: nothing of it runs. A surface may therefore be a command's entry file
(`"cli": "./src/main.ts"`), whose constants are read however the script behaves on import.
A fact that names anything else of it, such as a function, loads it, as an example does. <!-- test: adapters/typescript/test/unit/language/run/facts.test.ts > "a fact over a constant" > "never runs the module, so an entry script that runs on import can be a surface" -->

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/main.ts' }, adapter: 'typescript' }),
    'src/main.ts': [
      'const SECOND = 1000;',
      'const requestTimeout = 30 * SECOND;',
      "const regions = ['nl', 'de'] as const;",
      "export function usage(): string { return 'usage: shopd'; }",
      "throw new Error('shopd needs a terminal');",
    ].join('\n'),
    'docs/shopd.md': [
      'A request is cancelled after `30` seconds. <!-- fact: requestTimeout / SECOND -->',
      'It serves: <!-- fact: regions -->\n\n- `nl`\n- `de`',
      'Run it as `usage: shopd`. <!-- fact: usage() -->',
    ].join('\n\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['pass', 'pass', 'error']);
assert.match(report.results[2].message, /failed to load: Error: shopd needs a terminal/);
```

A constant is a `const` whose initializer is written in literals: strings, numbers, bigints,
booleans, `null`, templates, arrays and objects (`as const` or not), operators on them, and
the names of other constants declared above it in the module. <!-- test: adapters/typescript/test/unit/language/syntax/constant-value.test.ts > "foldConstant" > "applies operators, and reads names, members and spreads of folded values" -->
A `const` that calls anything, names an import, or whose value the module changes in place
(`LIST.push(…)`, `CONFIG.port = …`) is not one. <!-- test: adapters/typescript/test/unit/language/syntax/module-names.test.ts > "moduleNames" > "reads no const the module changes in place, or that names one declared below it" -->
Any other name the module declares and does not export is an error that says only
constants can be read from source: `round is a function that surface "default" does not
export; of its unexported names, a fact reads only constants`. <!-- test: adapters/typescript/test/unit/language/run/facts.test.ts > "a fact over a constant" > "reads only constants of the names the surface does not export" -->
The value is shown as an export's is, through `String(value)`, and an array is a list.

A fact attests the constants it reads, exported or not, as `where` shows; `uncovered` lists
only exports. <!-- test: adapters/typescript/test/unit/language/run/facts.test.ts > "a constant a fact reads" > "is attested, as `where` shows, while `uncovered` lists only exports" -->
So `impact` lists a change to such a constant as it lists a change to an export, with the
facts that read it. <!-- test: adapters/typescript/test/unit/language/run/facts.test.ts > "a constant a fact reads" > "is listed by `impact` when it changes, with the facts that read it" -->

A value holds the values of the names its initializer uses, so a fact that reads it attests
them too, at any depth: a fact over `ENV`, built from `COMMAND_PATH`, attests
`COMMAND_PATH`. A private constant used there is credited for `where`, and `uncovered`
leaves it out. A named fact and a cases runner attest the same of the values they read. A
function's body is not followed: a fact over `total()` attests `total`, not the names its
body reads. <!-- test: adapters/typescript/test/unit/language/symbols.test.ts > "a fact that reads a value built from other names" > "attests the names its initializer uses, at any depth, private ones too" --> <!-- test: adapters/typescript/test/unit/language/symbols.test.ts > "a named fact that reads a value built from other names" > "attests the names its initializer uses, at any depth, private ones too" --> <!-- test: adapters/typescript/test/unit/language/symbols.test.ts > "a cases runner that reads a value built from other names" > "attests the names its initializer uses, at any depth, private ones too" -->
`impact` lists the fact when one of those names changes. <!-- test: adapters/typescript/test/unit/language/symbols.test.ts > "a fact that reads a value built from other names" > "is listed by `impact` when one of those names changes" -->

A value that is not a constant, such as a `Set` the module builds, is read only by loading
a module that exports it: no other module, the setup module included, can reach a module's
private names. A module of the package exports such a value when the package's other
modules use it. When the package's entry does not re-export it, that module can be a
surface of its own (`"internal": "./src/caps.ts"`): facts read and attest the value there,
and the package's API stays as it is. <!-- test: adapters/typescript/test/unit/language/run/facts.test.ts > "a value the package entry does not re-export" > "is read and attested on a surface of the module that exports it" -->

### Named facts: the setup module

A fact may call a named fact of the setup module's `facts` export
([facts](../../docs/facts.md#facts-can-call-named-helpers-from-the-setup-module)). It
attests the exports the helper reaches, in its own code and in the setup module's functions
it calls, as a cases runner does. So `where` lists the fact under them, and `uncovered` no
longer lists them. <!-- test: adapters/typescript/test/unit/language/symbols.test.ts > "a fact that calls a named fact of the setup module" > "attests the exports its helper reaches, as `where` and `uncovered` show" -->
`impact` lists the fact when one of them changes. <!-- test: adapters/typescript/test/unit/language/symbols.test.ts > "a fact that calls a named fact of the setup module" > "is listed by `impact` when one of those exports changes" -->

### Cases tables: the setup module

A cases runner is a function of the setup module's `cases` export, called once per row with
the row's cells by column. It gets each cell's text as the page writes it, code spans with
their backticks ([cases](../../docs/cases.md#a-runner-receives-each-cell-as-written)).
`codeSpans`, which this package exports, takes the code out: it gives the content of every
code span in a cell, in order, as a fact over the cell's column compares it.

```ts check
import { codeSpans } from 'docattest';

assert.deepEqual(codeSpans('`FOO=1` `BAR=2`'), ['FOO=1', 'BAR=2']);
assert.deepEqual(codeSpans('``a`b`` or `` `c` ``'), ['a`b', '`c`']);
assert.deepEqual(codeSpans('no code'), []);
```

A span opens with a run of backticks and closes at the next run of as many, and the white
space around its content is dropped. As CommonMark has it, a backtick escaped with a
backslash opens no span, and neither does one inside an HTML tag or comment, an autolink, or
a link's destination or title. <!-- test: adapters/typescript/test/unit/cells/code-spans.test.ts > "codeSpans" > "$name" -->
Each cell of that table is also written into a page under a `fact column=` that lists what
`codeSpans` gives, and the core must agree. <!-- test: adapters/typescript/test/unit/cells/code-spans.test.ts > "codeSpans and the core" > "gives what a column fact compares" -->

A table's rows run one after another, in the page's order, in one worker thread, and the
setup module is loaded once for them: its module state is the rows' to share. <!-- test: adapters/typescript/test/unit/language/run/cases.test.ts > "the rows of a cases table" > "run one after another, in the page order, sharing the setup module and its state" -->
A row that throws or rejects fails, and the next row runs in the same worker. <!-- test: adapters/typescript/test/unit/language/run/cases.test.ts > "the rows of a cases table" > "go on after a row that throws or rejects, in the same worker" -->
A row that runs out of time, or exits, is the last of its worker: the next row starts on a
fresh one, which loads the setup module again. <!-- test: adapters/typescript/test/unit/language/run/cases.test.ts > "the rows of a cases table" > "start again on a fresh worker after a row that times out or exits" -->
Each table loads the setup module afresh, but a worker runs many tables, blocks and pages of
facts, one after another, and what a row leaves behind on `globalThis`, or a timer or a
server it does not stop, is still there for them: a runner stops what it starts before it
returns. <!-- test: adapters/typescript/test/unit/language/run/cases.test.ts > "cases tables" > "each load the setup module afresh, but share what a worker keeps between jobs" -->
Tables run side by side, with each other and with blocks: `runtime.workers` workers at
once in all. <!-- test: adapters/typescript/test/unit/language/run/cases.test.ts > "cases tables" > "run side by side, as many at once as there are workers" -->

### Citations of table-driven tests

`<!-- test: test/retry.test.ts > "describe title" > "test title" -->` ties a sentence to a
test: an `it` or `test` call, titled by its title, with the titles of the `describe` blocks
around it ([citations](../../docs/citations.md)). The file is read, never run. A title may
be computed, when the file's own literals write it: a constant, or a template of constants. <!-- test: adapters/typescript/test/unit/language/citations/tests-in-file.test.ts > "the tests of a table the file writes" > "titles the tests of a for-of loop and a forEach over a table the file writes" -->

So the tests of a table the file writes can be cited, each by the title its runner prints for
its row: `it.each(table)`, `test.each`, `describe.each` and `test.for` with a literal array,
or with a template-literal table, by their title's placeholders (`%s`, `%i`, `%#`, `$name`
and the rest); a `for (const c of table)` loop or a `table.forEach((c) => …)` over a literal
array (or `Object.entries` of a literal object), by the title its body computes. The table
is written in the call or held by a `const` above it that the file never changes. <!-- test: adapters/typescript/test/unit/language/citations/tests-in-file.test.ts > "the tests of a table the file writes" > "titles each row of an each table as the runner prints it" -->
Vitest and Jest print some values differently: Vitest quotes a string that `$name` names,
Jest does not. The runner is the one the file imports its test functions from (`vitest`, or
`@jest/globals`); when it imports them from neither, a title counts only where the two print
the same. <!-- test: adapters/typescript/test/unit/language/citations/tests-in-file.test.ts > "the tests of a table the file writes" > "titles a $name as each runner shows it, and as both do when the file names neither" -->
The titles Vitest prints are checked against Vitest itself, in a table-driven test whose
rows are cited so. <!-- test: adapters/typescript/test/unit/language/citations/vitest-titles.test.ts > "rows titled pair %i %s" > "pair 2 b" -->
What cannot be told stays uncitable: a table the file imports, builds with a call or changes
(`cases.push(…)`), and a value a title would show as an object or an array. <!-- test: adapters/typescript/test/unit/language/citations/tests-in-file.test.ts > "the tests of a table the file writes" > "cannot cite what the file does not write as a literal, or changes" -->
The title as written, `"adds %i"`, still cites the table's test as a whole.

```ts check
import { createProject } from 'docattest';

const cite = async (title: string) => {
  const report = await createProject({
    files: {
      'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
      'src/index.ts': 'export const N = 3;',
      'test/price.test.ts': [
        "import { describe, it } from 'vitest';",
        "const cases = [{ name: 'rounds down', cents: 1 }, { name: 'rounds up', cents: 5 }];",
        "describe('price', () => {",
        "  it.each([[1, 2], [3, 4]])('adds %i and %i', () => {});",
        '  for (const c of cases) it(c.name, () => {});',
        '});',
      ].join('\n'),
      'docs/p.md': `It holds. <!-- test: test/price.test.ts > "price" > "${title}" -->`,
    },
  }).check();
  return report.results[0];
};

assert.equal((await cite('adds 3 and 4')).status, 'pass');
assert.equal((await cite('rounds up')).status, 'pass');
const missing = await cite('rounds over');
assert.equal(missing.status, 'error');
assert.match(missing.message, /tests there: "price" > "adds %i and %i", "price" > "adds 1 and 2", "price" > "adds 3 and 4", "price" > "rounds down", "price" > "rounds up"/);
```

### Running cited tests

`docattest check --run-cited` also runs the test each citation names, with the project's own
runner, and the citation then passes only when that test ran and passed
([citations](../../docs/citations.md#running-the-cited-tests)). The runner is the one the
test file imports its test functions from (`vitest`, `@jest/globals` or `node:test`), else
the one the nearest `package.json` declares among its dependencies. Vitest and Jest run from
the directory of the nearest `package.json` that declares them, where their config is, as
installed in `node_modules` from there up to the project's root, on the Node that runs
docattest; so does node:test, from the nearest `package.json`. <!-- test: adapters/typescript/test/unit/language/test-runs/runner-choice.test.ts > "chooseRunner" > "runs a file with the runner it imports, from the package that declares it" --> <!-- test: adapters/typescript/test/unit/language/test-runs/runner-choice.test.ts > "chooseRunner" > "runs a file that imports nothing with the runner the nearest package declares" --> <!-- test: adapters/typescript/test/unit/language/test-runs/runner-choice.test.ts > "chooseRunner" > "runs node:test with the adapter's Node from the nearest package" -->
A file that names no runner, or whose runner is not installed, runs nothing, and its
citations are errors that say why. <!-- test: adapters/typescript/test/unit/language/test-runs/runner-choice.test.ts > "chooseRunner" > "has no runner for a file that names none, or whose runner is not installed" -->

The cited tests of one file run in one invocation, at most `runtime.workers` invocations at
once, each with a machine-readable report:

- Vitest: `vitest run <file> --testNamePattern=<pattern> --reporter=json --outputFile=<report>`. <!-- test: adapters/typescript/test/unit/language/test-runs/invocation.test.ts > "invocationOf" > "runs Vitest on the file alone, selecting by name, with a JSON report" -->
- Jest: `jest --json --outputFile=<report> --testNamePattern=<pattern> --runTestsByPath <file>`. <!-- test: adapters/typescript/test/unit/language/test-runs/invocation.test.ts > "invocationOf" > "runs Jest on exactly the file by its path, with its JSON report" -->
- node:test: `node --test --test-name-pattern=<pattern> --test-reporter=<docattest's reporter>
  --test-reporter-destination=<report> <file>`, with `--experimental-strip-types` on a Node
  older than 22.18, which does not strip TypeScript's types by itself. <!-- test: adapters/typescript/test/unit/language/test-runs/invocation.test.ts > "invocationOf" > "runs node:test with the adapter's reporter, stripping types where Node does not" -->

The pattern matches exactly each cited test's full name, its describe titles and its title
joined by single spaces, every character of a title taken literally. <!-- test: adapters/typescript/test/unit/language/test-runs/invocation.test.ts > "namePattern" > "matches exactly the full names, every character of a title taken literally" -->
Two tests whose titles join to the same name (`"price" > "rounds"` and `"price rounds"`)
both run, and Jest matches a name whatever its case; but how a test ran is read from the
runner's report, by its exact path, never from the exit code. <!-- test: adapters/typescript/test/unit/language/test-runs/outcome.test.ts > "outcomeOf" > "reads the test of exactly the cited path, not one whose titles join to the same name" --> <!-- test: adapters/typescript/test/protocol/run-tests.test.ts > "runTests" > "runs node:test tests by their exact path, a case of a table one alone" -->
A row of a table, cited by the title its runner prints, runs alone; the table cited by its
title as written (`"adds %i"`) runs every row, and passes only when every row does. <!-- test: adapters/typescript/test/unit/language/test-runs/outcome.test.ts > "outcomeOf" > "passes a table cited as a whole only when every row does, and names a failing row" --> <!-- test: adapters/typescript/test/protocol/run-tests.test.ts > "runTests" > "runs Vitest tests from the package that declares it, a table's rows together" -->
A table whose rows the file builds as it runs, so that it does not tell their titles, runs
by its describe titles: its rows are the tests reported directly under them, but for the
file's other tests there. It passes when one row at least ran and every row passed. <!-- test: adapters/typescript/test/unit/language/test-runs/invocation.test.ts > "namePattern" > "selects every test under describe titles, for a table whose rows are not known" --> <!-- test: adapters/typescript/test/protocol/run-tests.test.ts > "runTests" > "runs the rows of a table built as the file runs, by its describe titles, as one" -->
A node:test test that runs subtests passes only when they do. <!-- test: adapters/typescript/test/unit/language/test-runs/outcome.test.ts > "outcomeOf" > "fails a test whose subtest failed with the subtest's message, not the summary" -->

A test that fails says what its runner said of it, without the stack. <!-- test: adapters/typescript/test/unit/language/test-runs/outcome.test.ts > "outcomeOf" > "gives a failure's own lines, without its stack" -->
One the runner skips (`skip`, a `todo`, a skip at run time such as `t.skip('why')`) is
skipped, with the reason when the runner gives one. <!-- test: adapters/typescript/test/unit/language/test-runs/node-test-report.test.ts > "readNodeTestReport" > "reads each test by the path of the tests around it, and leaves describe blocks out" --> <!-- test: adapters/typescript/test/unit/language/test-runs/json-report.test.ts > "readJsonReport" > "reads the file's tests from Vitest's report by their titles, whatever else ran" -->
A test the run did not find by its path, such as one declared under an `if` that never
holds, and the tests of a file that cannot load, did not run. <!-- test: adapters/typescript/test/unit/language/test-runs/outcome.test.ts > "outcomeOf" > "runs nothing of a path the run selected no test by, or of a file that cannot run" -->
A project in memory (`createProject` with `files`) runs none of its tests. <!-- test: adapters/typescript/test/protocol/run-tests.test.ts > "runTests" > "runs nothing of a project in memory" -->
An invocation still running after its time limit, ten minutes, is killed with the processes
it started, and each of its tests fails. <!-- test: adapters/typescript/test/protocol/run-tests.test.ts > "runTests" > "fails every test of a run that does not finish in time" -->
The reports read are those of Vitest 3 and of node:test on Node 22.12 to 24; Jest's is read
from its documented format, the same as Vitest's. <!-- test: adapters/typescript/test/unit/language/test-runs/json-report.test.ts > "readJsonReport" > "reads Jest's report, and a file that could not run" -->

## Commands

Every command (`check`, `where`, `impact` and the rest) is described in the
[repository README](../../README.md#commands) and [the command line](../../docs/cli.md).

## From code

```ts compile
import { createProject } from 'docattest';

const report = await createProject({ root: '.' }).check();
console.log(report.exitCode, report.results.length);
```

`createProject` runs the same core the command runs and returns what it prints with
`--json`, typed. It also takes an in-memory map of files instead of a root.

## What is in this package

The package installs these commands: <!-- fact: Object.keys(manifest("adapters/typescript/package.json").bin) -->
- `docattest`: a launcher that finds the core (`DOCATTEST_CORE`, else the platform package,
  else a cache shared with the other language packages), tells it how to start the
  TypeScript adapter, and becomes it.
- `docattest-adapter-typescript`: the TypeScript adapter, which the core starts and talks to
  over [the adapter protocol](../../protocol/PROTOCOL.md). You never run it yourself.

Its module exports `createProject` and the types it returns: the programmatic API
([from code](#from-code)); and `codeSpans`, for the runners of cases tables
([the setup module](#cases-tables-the-setup-module)).

## Working on this package

The sources are grouped by what they serve. The TypeScript language itself (compiling and
running examples, evaluating facts, reading symbols, resolving surfaces, what a change
affects) is under `src/language/`, the only part that loads the compiler. Around it are the
protocol server that answers the core, the `docattest` command and the code that finds the
core it runs, `createProject`, which runs the core for code, and the public types. Tests in
`test/` mirror that layout under `unit/`; `protocol/` drives the built server over its pipes;
and the acceptance and corpus tests run one test per criterion of
[ACCEPTANCE.md](../../docs/design/ACCEPTANCE.md) and per event of
[CORPUS.md](../../docs/design/CORPUS.md) through the built package and the core.
`conformance.json` is this adapter's kit for the protocol's conformance suite.

From the repository root: `npm ci`, `npm run build:core`, `npm run build`, `npm test`.
[CONTRIBUTING.md](../../CONTRIBUTING.md) has the rest.

## License

[MIT](LICENSE)
