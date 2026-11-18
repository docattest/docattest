---
title: Cases tables
---

# Cases tables

A Markdown table preceded by `<!-- cases: <runner> -->` runs one test per row. The rows
are the expectations and live in the page where a reader sees them. The runner is a
function exported from the setup module named in the config; it knows how to execute a
row and nothing about what any row expects.

The setup module used on this page:

```ts
import assert from 'node:assert/strict';
import { clamp } from 'docattest-fixture';
export const cases = {
  clamp: Object.assign(
    (row: Record<string, string>) => {
      const got = clamp(Number(row.value), Number(row.min), Number(row.max));
      assert.equal(String(got), row.result);
    },
    { columns: ['value', 'min', 'max', 'result'] },
  ),
};
```

## Each row is a result, named by the page, line and first cell

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': `export const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));`,
    'docs/docattest.setup.ts': [
      "import assert from 'node:assert/strict';",
      "import { clamp } from 'docattest-fixture';",
      'export const cases = { clamp: Object.assign((row: Record<string, string>) => {',
      '  assert.equal(String(clamp(Number(row.value), Number(row.min), Number(row.max))), row.result);',
      "}, { columns: ['value', 'min', 'max', 'result'] }) };",
    ].join('\n'),
    'docs/clamp.md': [
      '# clamp', '', 'Clamps into a range.', '',
      '<!-- cases: clamp -->',
      '| case          | value | min | max | result |',
      '|---------------|-------|-----|-----|--------|',
      '| below range   | -1    | 0   | 10  | 0      |',
      '| inside range  | 5     | 0   | 10  | 5      |',
      '| above range   | 11    | 0   | 10  | 10     |',
      '| wrong on purpose | 5  | 0   | 10  | 6      |',
    ].join('\n'),
  },
}).check();

assert.equal(report.results.length, 4);
assert.deepEqual(report.results.map((r) => r.kind), ['cases', 'cases', 'cases', 'cases']);
assert.deepEqual(report.results.map((r) => r.status), ['pass', 'pass', 'pass', 'fail']);
assert.deepEqual(report.results.map((r) => r.line), [8, 9, 10, 11]);
assert.match(report.results[3].message, /wrong on purpose/);
assert.match(report.results[3].message, /6/);
assert.deepEqual(report.results[0].symbols, ['clamp']);
```

## An unknown runner is an error before any row runs

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/docattest.setup.ts': 'export const cases = {};',
    'docs/t.md': '<!-- cases: nope -->\n| case | result |\n|---|---|\n| a | 1 |\n',
  },
}).check();

assert.equal(report.results.length, 1);
assert.equal(report.results[0].status, 'error');
assert.equal(report.results[0].line, 1);
assert.match(report.results[0].message, /nope/);
```

## A header missing a declared column is an error before any row runs

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/docattest.setup.ts': "export const cases = { t: Object.assign((row: Record<string, string>) => {}, { columns: ['input', 'result'] }) };",
    'docs/t.md': '<!-- cases: t -->\n| case | result |\n|---|---|\n| a | 1 |\n',
  },
}).check();

assert.equal(report.results.length, 1);
assert.equal(report.results[0].status, 'error');
assert.match(report.results[0].message, /input/);
```

## The directive binds the next table, even across blank lines

Like a `column=` fact, a `cases` directive can only mean a table, so blank lines between
the two are allowed. Anything else between them, or no table at all, is an error.

```ts check
import { createProject } from 'docattest';

const page = (body: string) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': `export const double = (n: number) => n * 2;`,
    'docs/docattest.setup.ts': [
      "import assert from 'node:assert/strict';",
      "import { double } from 'docattest-fixture';",
      'export const cases = { double: (row: Record<string, string>) => {',
      '  assert.equal(String(double(Number(row.input))), row.result);',
      '} };',
    ].join('\n'),
    'docs/t.md': body,
  },
});
const table = '| case | input | result |\n|---|---|---|\n| two | 2 | 4 |\n';

const spaced = await createProject(page(`<!-- cases: double -->\n\n${table}`)).check();
assert.deepEqual(spaced.results.map((r) => [r.line, r.status]), [[5, 'pass']]);

const interrupted = await createProject(page(`<!-- cases: double -->\n\nA paragraph first.\n\n${table}`)).check();
assert.equal(interrupted.results[0].status, 'error');
assert.match(interrupted.results[0].message, /cases at L1 has no table/);

const missing = await createProject(page('<!-- cases: double -->\n')).check();
assert.equal(missing.results[0].status, 'error');
assert.match(missing.results[0].message, /cases at L1 has no table/);
```

## A runner receives each cell as written

A runner gets each cell's text as the page writes it, code spans included: a cell
`` `3` `` reaches the runner as `` `3` ``, backticks and all, and only the `\|` escape is
undone. That is not what a fact compares: a fact over a table column takes the code spans
in a cell, `3`. A runner whose cells hold code takes the code out with its language's
helper, which gives the content of every code span in a cell, in order, as a fact over the
cell's column compares it: `codeSpans` from `docattest` in TypeScript,
`docattest.cells.code_spans` in Python, `cells.CodeSpans` from
`github.com/docattest/docattest/adapters/go/cells` in Go, and
`docattest_harness::cells::code_spans` in Rust. The core and the four helpers are tested
against one table of cells, `protocol/test-data/code-spans.json`.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/docattest.setup.ts': [
      "import assert from 'node:assert/strict';",
      'export const cases = { raw: (row: Record<string, string>) => {',
      "  assert.equal(row.value, '`3`');",
      "  assert.equal(row.text, 'a | b');",
      '} };',
    ].join('\n'),
    'docs/t.md': '<!-- cases: raw -->\n| case | value | text |\n|---|---|---|\n| as written | `3` | a \\| b |\n',
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['pass']);
```

```ts check
import { codeSpans } from 'docattest';

assert.deepEqual(codeSpans('`3`'), ['3']);
assert.deepEqual(codeSpans('`FOO=1` and `BAR=2`'), ['FOO=1', 'BAR=2']);
```

## A table's rows run one after another

A table's rows run in the page's order, one after another, sharing one process (in
TypeScript, one worker thread), so what a row leaves behind is still there for the rows
after it: a runner stops what it starts before it returns. A row that throws, raises or
panics fails, and the next row runs in the same process. A row that runs out of time or
ends the process is the last of its process, and the next row starts a fresh one. Tables run side by side, with each other and with
blocks, `runtime.workers` at once. Each guide says what a row shares in its language:

| language | a table's rows share | guide |
|---|---|---|
| TypeScript | a worker thread, and the setup module loaded once for the table; a worker runs other tables and blocks too | [cases tables](../adapters/typescript/README.md#cases-tables-the-setup-module) |
| Python | a process of its own, which runs the setup module once | [cases tables](../adapters/python/README.md#cases-tables-and-named-facts-the-setup-module) |
| Go | a process of its own, with the setup package's state | [cases tables](../adapters/go/docs/cases.md) |
| Rust | a process of its own, each row on a thread of its own | [cases tables](../adapters/rust/README.md#cases-tables-and-named-facts-the-setup-file) |

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/docattest.setup.ts': [
      "import assert from 'node:assert/strict';",
      'const seen: string[] = [];',
      'export const cases = { order: (row: Record<string, string>) => {',
      "  if (row.case === 'throws') throw new Error('on purpose');",
      '  seen.push(row.case);',
      "  assert.equal(seen.join(' '), row.seen);",
      '} };',
    ].join('\n'),
    'docs/t.md': '<!-- cases: order -->\n| case | seen |\n|---|---|\n| a | a |\n| throws | - |\n| b | a b |\n',
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['pass', 'fail', 'pass']);
```

## A comment in a cell is row data, never a directive

A cell of a cases table is data for the runner, and a comment in it is part of that data:
the runner receives it as written, and a directive there is not checked. That is what lets a
runner take a whole page as a cell, as the binding table in [facts](facts.md) does. To pin
a value that a table uses, put the fact in the prose around the table, not in a cell.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': 'export const RETRY = 3;',
    'docs/docattest.setup.ts': [
      "import assert from 'node:assert/strict';",
      'export const cases = { echo: (row: Record<string, string>) => {',
      "  assert.equal(row.tries, '`2` <!-- fact: RETRY -->');",
      '} };',
    ].join('\n'),
    'docs/t.md': '<!-- cases: echo -->\n| case | tries |\n|---|---|\n| default | `2` <!-- fact: RETRY --> |\n',
  },
}).check();

assert.deepEqual(report.results.map((r) => [r.kind, r.line, r.status]), [['cases', 4, 'pass']]);
```
