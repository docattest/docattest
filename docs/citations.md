---
title: Citations
---

# Citations

A citation ties a sentence to a test the project already has. It is the cheap way to
defend a sentence when the behaviour needs too much setup for an example on the page and
a test elsewhere already covers it. Put it after the sentence it defends, on the same line:

```md
A call that keeps failing throws the last error. <!-- test: test/retry.test.ts > "throws the last error" -->
```

The file is relative to the project root. The title is the test's own title, in double or
single quotes. Titles of enclosing `describe` blocks may come first, outermost first, as
further `> "…"` segments; when they are given they must match the test's describe blocks
exactly, and when they are left out any test with that title in the file matches. No
configuration is needed. Deleting or renaming the cited test fails the build at the
sentence, and the sentence shows up in `where` and the index.

## What a citation does not prove

A citation is weaker than an example, and the report keeps it apart as its own kind,
`test`, never counted as an example. docattest reads the test file's syntax; it does not
run the test. That is the project's own test job, which docattest neither replaces nor
watches. So a citation cannot tell that the test fails, is skipped, has been weakened
until it asserts nothing, or never tested what the sentence says. It proves only that a
test with that name still exists where the page says it does. When a sentence matters
enough for more, write an example.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const add = (a: number, b: number) => a + b;`,
    'test/add.test.ts': "it.skip('adds', () => { expect(1).toBe(2); });",
    'docs/add.md': 'Adding works. <!-- test: test/add.test.ts > "adds" -->',
  },
}).check();

// A skipped test that would fail still satisfies its citation: nothing ran it.
assert.equal(report.results[0].kind, 'test');
assert.equal(report.results[0].status, 'pass');
assert.match(report.results[0].message, /not run by docattest/);
```

## A citation of a test that exists passes, and covers its page

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const retry = async <T>(fn: () => Promise<T>) => fn();\nexport const unused = 1;`,
    'test/retry.test.ts': [
      "import { describe, it, expect } from 'vitest';",
      "import { retry } from '../src/index.js';",
      "describe('retry', () => {",
      "  it('throws the last error', async () => {",
      "    await expect(retry(async () => { throw new Error('x'); })).rejects.toThrow('x');",
      '  });',
      '});',
    ].join('\n'),
    'docs/retry.md': '# Retry\n\nA call that keeps failing throws the last error. <!-- test: test/retry.test.ts > "throws the last error" -->',
  },
}).check();

assert.equal(report.results.length, 1);
assert.equal(report.results[0].kind, 'test');
assert.equal(report.results[0].status, 'pass');
assert.equal(report.results[0].line, 3);
assert.match(report.results[0].message, /test\/retry\.test\.ts:4 declares "retry" > "throws the last error"/);
assert.deepEqual(report.coverage.uncovered, []);
assert.equal(report.coverage.cited, 1);
assert.equal(report.exitCode, 0);
```

## Renaming or deleting the test fails the build at the sentence

The message names the file and the title. When the title is missing it lists the titles
the file does declare, so the fix is plain at the failure site.

```ts check
import { createProject } from 'docattest';

const check = (tests: Record<string, string>) =>
  createProject({
    files: {
      'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
      'src/index.ts': `export const N = 3;`,
      'docs/a.md': 'Intro.\n\nIt throws. <!-- test: test/a.test.ts > "throws the last error" -->',
      ...tests,
    },
  }).check();

const renamed = await check({ 'test/a.test.ts': "it('rethrows', () => {});\nit('waits', () => {});" });
assert.equal(renamed.exitCode, 1);
assert.deepEqual(renamed.results.map((r) => [r.line, r.kind, r.status]), [[3, 'test', 'error']]);
assert.equal(renamed.results[0].message, 'test/a.test.ts declares no test "throws the last error"; tests there: "rethrows", "waits"');

const deleted = await check({});
assert.equal(deleted.results[0].status, 'error');
assert.equal(deleted.results[0].message, 'cannot cite "throws the last error": test/a.test.ts does not exist');
```

## Describe titles, when given, must match the test's describe blocks

```ts check
import { createProject } from 'docattest';

const cite = async (directive: string) => {
  const report = await createProject({
    files: {
      'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
      'src/index.ts': `export const N = 3;`,
      'test/a.test.ts': "describe('retry', () => { describe('with backoff', () => { it('waits', () => {}); }); });",
      'docs/a.md': `It waits. ${directive}`,
    },
  }).check();
  return report.results[0];
};

assert.equal((await cite('<!-- test: test/a.test.ts > "waits" -->')).status, 'pass');
assert.equal((await cite('<!-- test: test/a.test.ts > "retry" > "with backoff" > "waits" -->')).status, 'pass');

const partial = await cite('<!-- test: test/a.test.ts > "with backoff" > "waits" -->');
assert.equal(partial.status, 'error');
assert.match(partial.message, /tests there: "retry" > "with backoff" > "waits"/);

const suite = await cite('<!-- test: test/a.test.ts > "retry" -->');
assert.equal(suite.status, 'error');
assert.match(suite.message, /"retry" in test\/a\.test\.ts is a describe block, not a test/);
```

## Only a declared test counts

`it` and `test` calls count, with their `.only`, `.skip`, `.concurrent` and `.each` forms,
when the title is a string literal. The same words in a comment or a string do not.

```ts check
import { createProject } from 'docattest';

const statuses = async (titles: string[]) => {
  const page = titles.map((t) => `Claim. <!-- test: test/a.test.ts > "${t}" -->`).join('\n\n');
  const report = await createProject({
    files: {
      'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
      'src/index.ts': `export const N = 3;`,
      'test/a.test.ts': [
        "// it('in a comment', () => {});",
        "const note = \"it('in a string', () => {})\";",
        "it.only('only', () => {});",
        'test.concurrent(`concurrent`, async () => {});',
        "it.each([1, 2])('each %i', (n) => {});",
      ].join('\n'),
      'docs/a.md': page,
    },
  }).check();
  return report.results.map((r) => r.status);
};

assert.deepEqual(await statuses(['only', 'concurrent', 'each %i']), ['pass', 'pass', 'pass']);
assert.deepEqual(await statuses(['in a comment', 'in a string']), ['error', 'error']);
```

## A malformed citation fails closed; an ordinary comment is left alone

A citation with no title, no file, a missing quote or no closing `-->` is an error at its
line. So is one with no sentence before it to defend. A comment that merely starts with a
word like "testing" is not a citation.

```ts check
import { createProject } from 'docattest';

const check = (page: string) =>
  createProject({
    files: {
      'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
      'src/index.ts': `export const N = 3;`,
      'test/a.test.ts': "it('works', () => {});",
      'docs/a.md': page,
    },
  }).check();

const untitled = await check('It works. <!-- test: test/a.test.ts -->');
assert.equal(untitled.results[0].status, 'error');
assert.match(untitled.results[0].message, /test at L1 names no test title after test\/a\.test\.ts/);

const unclosed = await check('It works. <!-- test: test/a.test.ts > "works"\n\nMore prose.');
assert.equal(unclosed.results[0].status, 'error');
assert.match(unclosed.results[0].message, /not closed/);

const alone = await check('<!-- test: test/a.test.ts > "works" -->');
assert.match(alone.results[0].message, /defends no sentence/);

const ordinary = await check('It works. <!-- testing notes: revisit this wording -->');
assert.deepEqual(ordinary.results, []);
assert.equal(ordinary.exitCode, 0);
```

## A citation on its own line defends the paragraph just before it

On a line of its own, a citation defends the paragraph immediately before it in the same
container: in a list item, the item's own sentence, not the prose before the list.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const retry = (n: number) => n;`,
    'test/retry.test.ts': "import { retry } from '../src/index.js';\nit('retries', () => { retry(3); });",
    'docs/retry.md': [
      'Retrying:', '',
      '- A call is retried.',
      '  <!-- test: test/retry.test.ts > "retries" -->',
    ].join('\n'),
  },
});

const report = await project.check();
assert.deepEqual(report.results.map((r) => [r.line, r.status]), [[4, 'pass']]);
assert.deepEqual(await project.where('retry'), [{ page: 'docs/retry.md', line: 4, kind: 'test', claim: 'A call is retried.' }]);
```

## The index lists citations, with the symbols the cited test uses

`where` and the index carry a citation under each surface symbol that the cited test's own
body uses, imported from a surface by name or by a relative path to the surface's file.
Symbols used only in hooks or helpers outside the test are not followed, so such a
citation names no symbol; it still covers its page.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const retry = (n: number) => n;\nexport const wait = () => 0;`,
    'test/retry.test.ts': [
      "import { retry, wait } from '../src/index.js';",
      "it('retries', () => { retry(3); });",
      "it('waits', () => { wait(); });",
    ].join('\n'),
    'docs/retry.md': '# Retry\n\nA call is retried. <!-- test: test/retry.test.ts > "retries" -->',
  },
});

const refs = await project.where('retry');
assert.deepEqual(refs, [{ page: 'docs/retry.md', line: 3, kind: 'test', claim: 'A call is retried.' }]);
assert.deepEqual(await project.where('wait'), []);

const report = await project.check();
assert.deepEqual(report.results[0].symbols, ['retry']);
assert.deepEqual(report.surfaces.default.unattested, ['wait']);
```

## The text report counts citations on the coverage line

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-cited-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', `export const N = 3;`);
put('test/a.test.ts', "it('works', () => {});");
put('docs/a.md', 'N is `3`. <!-- fact: N -->\n\nIt works. <!-- test: test/a.test.ts > "works" -->');
// ---cut---
const lines = spawnSync(process.execPath, [cli, 'check'], { cwd: root, encoding: 'utf8' }).stdout.split('\n');
assert.ok(lines.includes('coverage: 1 living page, 1 covered (2 attestations, 0 weak, 1 cited), 0 uncovered, 0 frozen'));
```

This page cites one of docattest's own tests: the npm launcher names platforms the way
every docattest package does. <!-- test: adapters/typescript/test/unit/core-binary/locate.test.ts > "the core lookup" > "names platforms the same in every package" -->
