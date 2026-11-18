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

1. Add `docattest.config.json` at the project root:

   ```json
   {
     "pages": ["docs/**/*.md"],
     "surfaces": { "default": "./src/index.ts" },
     "adapter": "typescript"
   }
   ```

   The *surface* is your public API: what pages may import (under your package's name) and
   what facts may read.

2. Add proofs to a page, for example `docs/retries.md`:

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
| **Example** | a block tagged `ts check` (runs) or `ts compile` (type-checks only); `js`, `tsx` and friends too | it throws, an `assert` fails, or it no longer type-checks |
| **Fact** | `<!-- fact: expression -->` after a value, or above a list, table column or code block | the value on the page differs from `String(value)` of the expression |
| **Cases** | `<!-- cases: runner -->` above a table, with the runner in your setup module | a row's expected result is not what the runner produces |
| **Citation** | `<!-- test: test/retry.test.ts > "describe title" > "test title" -->` | the file no longer declares an `it` or `test` with that title |
| **Doc comment** | an `@example` whose first line is `// check` or `// compile`, in a file under `sources` | as for an example |

Every example is its own module with a fresh import of your package, runs in a worker with a
time limit, and has the network off: a `check network` block runs only when
`DOCATTEST_NETWORK=1` is set, and is skipped otherwise. Lines above `// ---cut---` run but
are setup that renderers can hide. A surface is an entry file (`./src/index.ts`), a package
name resolved as Node would, or both (`{ "import": "@acme/client", "path":
"packages/client/dist/index.js" }`), which is how a monorepo points a page at one package's
build.

A fact expression reads the surface's exports by their names, `snapshot("name")`,
`manifest("path")` and `file("path")`. `manifest` gives the parsed data of a project file
such as `package.json`, so a claim about what the package declares needs no setup code. The
versions at the top of this page are read so ([manifests](../../docs/manifests.md)). `file`
gives any other project file's text, a string, for its own methods and regular
expressions: this package is released under the `MIT` license. <!-- fact: file("adapters/typescript/LICENSE").match(/^(\w+) License/)[1] -->

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
