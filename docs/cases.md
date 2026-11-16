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
undone. That is not what a fact compares: a fact over a table column takes a cell's first
code span, `3`. A runner whose cells hold code strips the backticks itself.

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
