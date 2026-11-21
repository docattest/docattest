---
title: Citations
---

# Citations

A citation ties a sentence to a test the project already has. It is the cheap way to
defend a sentence when the behaviour needs too much setup for an example on the page and
a test elsewhere already covers it. Put it after the sentence it defends, on the same line,
or alone on the line directly above that sentence:

```md
A call that keeps failing throws the last error. <!-- test: test/retry.test.ts > "throws the last error" -->

<!-- test: test/retry.test.ts > "gives up after the last attempt" -->
A call is given up after its last attempt.
```

The file is relative to the project root. The title is the test's own title, in double or
single quotes. Titles of enclosing `describe` blocks may come first, outermost first, as
further `> "…"` segments; when they are given they must match the test's describe blocks
exactly, and when they are left out any test with that title in the file matches. No
configuration is needed. Deleting or renaming the cited test fails the build at the
sentence, and the sentence shows up in `where` and the index.

Two shorthands save writing the same file and titles again: a set of titles cites several
tests of one place in one directive, and an alias names a test file once for a whole page
([shorthands](citation-shorthands.md)).

## What a citation does not prove

A citation is weaker than an example, and the report keeps it apart as its own kind,
`test`, never counted as an example. By default docattest reads the test file's syntax; it
does not run the test. That is the project's own test job. So a citation cannot tell that
the test fails, is skipped, has been weakened until it asserts nothing, or never tested
what the sentence says. It proves only that a test with that name still exists where the
page says it does. [`--run-cited`](#running-the-cited-tests) runs the tests too, which
answers the first two; no run answers the last two. When a sentence matters enough for
more, write an example.

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

## Running the cited tests

`docattest check --run-cited` also runs the test each citation names, once the check has
found it declared, and a citation then holds only when its test ran and passed. A test
that fails fails its citation, with what the runner said about the failure; one the runner
skips, or does not run at all, is an error that says why. The language's adapter runs the
project's own test runner with exactly the cited tests selected, so no other test runs,
and reads the runner's own report of each. Each guide says what runs in its language:
[TypeScript](../adapters/typescript/README.md#running-cited-tests),
[Python](../adapters/python/README.md#running-cited-tests),
[Go](../adapters/go/docs/citations.md#running-cited-tests),
[Rust](../adapters/rust/README.md#running-cited-tests).

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-run-cited-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', 'export const N = 3;');
put('test/math.test.ts', [
  "import { test } from 'node:test';",
  "import assert from 'node:assert/strict';",
  "test('adds', () => { assert.equal(1 + 1, 2); });",
  "test('subtracts', () => { assert.equal(2 - 1, 0, 'two minus one is not zero'); });",
  "test.skip('multiplies', () => { assert.equal(2 * 2, 4); });",
].join('\n'));
put('docs/math.md', [
  'Adding works. <!-- test: test/math.test.ts > "adds" -->', '',
  'Subtracting works. <!-- test: test/math.test.ts > "subtracts" -->', '',
  'Multiplying works. <!-- test: test/math.test.ts > "multiplies" -->',
].join('\n'));
// ---cut---
const check = (...args: string[]) => spawnSync(process.execPath, [cli, 'check', ...args], { cwd: root, encoding: 'utf8' });

// Without --run-cited, three tests that are declared hold their citations.
assert.equal(check().status, 0);

const run = check('--run-cited');
assert.equal(run.status, 1);
assert.match(run.stdout, /✓ L1 test: test\/math\.test\.ts:3 declares "adds", which ran and passed\n/);
assert.match(run.stdout, /✗ L3 test: test\/math\.test\.ts:4 declares "subtracts", which failed: [^]*two minus one is not zero/);
assert.match(run.stdout, /! L5 test: test\/math\.test\.ts:5 declares "multiplies", which was skipped/);
```

Running costs what running those tests costs, so it is meant for CI, where the project's
tests run anyway, and not for the check a coding agent runs after every edit. That is why
it is a flag of `check` and not a key of the config: the config holds for every check,
and the cost belongs to the one that asks for it. A test suite is not a block, so
`runtime.timeoutMs` does not bound a run of one: a run of the runner that has not finished
after `600_000` <!-- fact: matched("core/src/project/check/cited_runs.rs", `RUN_TIMEOUT_MS: u64 = ([0-9_]+);`) -->
milliseconds, ten minutes, is stopped, and its tests fail.

A citation whose test ran is still a citation, not a proof: the run shows that the test
passes, not that it tests the sentence. So the coverage line counts it among the
citations, and says how many of them ran; the report object has the count as
`citedRun`, only when the check ran cited tests.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-run-cited-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', 'export const N = 3;');
put('test/math.test.ts', "import { test } from 'node:test';\ntest('adds', () => {});\ntest.skip('multiplies', () => {});");
put('docs/math.md', 'N is `3`. <!-- fact: N -->\n\nIt adds. <!-- test: test/math.test.ts > "adds" -->\n\nIt multiplies. <!-- test: test/math.test.ts > "multiplies" -->');
// ---cut---
const check = (...args: string[]) => spawnSync(process.execPath, [cli, 'check', ...args], { cwd: root, encoding: 'utf8' }).stdout;
assert.ok(check('--run-cited').split('\n').includes('coverage: 1 living page, 1 covered (3 attestations: 1 proof, 0 of them weak, and 2 citations, 1 of them run), 0 uncovered, 0 frozen'));
assert.equal(JSON.parse(check('--json', '--run-cited')).coverage.citedRun, 1);
assert.equal(JSON.parse(check('--json')).coverage.citedRun, undefined);
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
assert.equal(report.coverage.proofs, 0);
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
when the file's own literals write the title. The same words in a comment or a string do
not.

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

assert.deepEqual(await statuses(['only', 'concurrent', 'each %i', 'each 1']), ['pass', 'pass', 'pass', 'pass']);
assert.deepEqual(await statuses(['in a comment', 'in a string']), ['error', 'error']);
```

## The cases of a table-driven test are cited by the names their runner prints

A table-driven test runs one case for each row of a table, and its runner prints a name for
each. When the test file writes the table in literals, each case can be cited by that name,
as a test inside its test. A citation never runs the test, so the name is read from the
file as the runner would print it:

- TypeScript: the rows of `it.each`, `describe.each` and `test.for`, and the tests of a
  `for … of` loop or a `.forEach` over a table the file writes, by the title the runner
  prints for each row; the title as written, `"each %i"`, still cites the test as a whole
  ([TypeScript guide](../adapters/typescript/README.md#citations-of-table-driven-tests)).
- Python: the cases of `@pytest.mark.parametrize`, by the id pytest prints, so
  `test_total[big discount]` is `"test_total" > "big discount"`, and the subtests of
  `with self.subTest(...)`, by what unittest prints, so `subTest(region="NL")` is
  `"region='NL'"` ([Python guide](../adapters/python/README.md#citations)).
- Go: the subtests that `t.Run` starts in a loop over a table written as a composite
  literal in the test function, by the names the table writes
  ([Go guide](../adapters/go/docs/citations.md)).
- Rust: the named cases of rstest (`#[case::quarter(…)]`) and of test-case
  (`#[test_case(… ; "a quarter off")]`), with the function named around them:
  `"tests" > "discount" > "quarter"` ([Rust guide](../adapters/rust/README.md#citations)).

A name the file does not write, such as one a function makes, or a row of a table built by
a call or changed after it is written, cannot be cited; the test itself still can. A
citation of a case that is not there lists the cases that are.

```ts check
import { createProject } from 'docattest';

const cite = async (title: string) => {
  const report = await createProject({
    files: {
      'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
      'src/index.ts': `export const N = 3;`,
      'test/a.test.ts': "it.each([[1, 2], [3, 4]])('adds %i and %i', () => {});",
      'docs/a.md': `It adds. <!-- test: test/a.test.ts > "${title}" -->`,
    },
  }).check();
  return report.results[0];
};

assert.equal((await cite('adds 3 and 4')).status, 'pass');
assert.equal((await cite('adds %i and %i')).status, 'pass');
const missing = await cite('adds 5 and 6');
assert.equal(missing.status, 'error');
assert.equal(missing.message, 'test/a.test.ts declares no test "adds 5 and 6"; tests there: "adds %i and %i", "adds 1 and 2", "adds 3 and 4"');
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

## A citation on the line above a sentence defends that sentence

A line that holds only directives applies each of them to the line directly below it, as
`// @ts-expect-error` does: citations there defend the sentence that starts on that line,
and `where` shows that sentence as their claim. With a blank line between such a line and
the prose below, a citation is an error at its line that says to put it directly above
the sentence or at its end.

```ts check
import { createProject } from 'docattest';

const page = (gap: string) => [
  '# Launching', '',
  '<!-- test: test/launch.test.ts > "runs a Python script" --> <!-- test: test/launch.test.ts > "runs a Node.js script" -->',
  `${gap}Python and Node.js scripts run the same way. Other scripts do not.`,
].join('\n');
const project = (gap: string) => createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const launch = (script: string) => script;`,
    'test/launch.test.ts': [
      "import { launch } from '../src/index.js';",
      "it('runs a Python script', () => { launch('a.py'); });",
      "it('runs a Node.js script', () => { launch('a.js'); });",
    ].join('\n'),
    'docs/launch.md': page(gap),
  },
});

const above = project('');
const report = await above.check();
assert.deepEqual(report.results.map((r) => [r.line, r.status]), [[3, 'pass'], [3, 'pass']]);
const claim = 'Python and Node.js scripts run the same way.';
assert.deepEqual((await above.where('launch')).map((r) => [r.line, r.claim]), [[3, claim], [3, claim]]);

const apart = await project('\n').check();
assert.deepEqual(apart.results.map((r) => r.status), ['error', 'error']);
assert.match(apart.results[0].message, /test at L3 must sit directly above the sentence it attests/);
```

## A citation on its own line defends the paragraph just before it

On a line of its own with no prose directly below it, a citation defends the paragraph
immediately before it in the same container: in a list item, the item's own sentence, not
the prose before the list.

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

`where` and the index carry a citation under each surface symbol that the cited test's body
uses, imported from a surface by name or by a relative path to the surface's file, including
local helper functions and variables in the same test file (or test package) that the test
references. When a passing citation defends a sentence whose code spans name at least one
exported surface symbol and the cited test uses none of those symbols, `check` emits a
warning naming the page, line, symbol(s) and test title without failing the check.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const retry = (n: number) => n;\nexport const wait = () => 0;`,
    'test/retry.test.ts': [
      "import { retry, wait } from '../src/index.js';",
      'function runRetry() { return retry(3); }',
      "it('retries', () => { runRetry(); });",
      "it('waits', () => { wait(); });",
    ].join('\n'),
    'docs/retry.md': [
      '# Retry',
      '',
      'A call to `retry` is retried. <!-- test: test/retry.test.ts > "retries" -->',
      '',
      'Calling `retry` never blocks forever. <!-- test: test/retry.test.ts > "waits" -->',
    ].join('\n'),
  },
});

const refs = await project.where('retry');
assert.deepEqual(refs, [{ page: 'docs/retry.md', line: 3, kind: 'test', claim: 'A call to `retry` is retried.' }]);

const report = await project.check();
assert.equal(report.exitCode, 0);
assert.deepEqual(report.results[0].symbols, ['retry']);
assert.deepEqual(report.warnings, ['docs/retry.md:5 citation names `retry`, which `waits` does not use']);
```

## The coverage line counts proofs and citations apart

A citation says where to look; it is not a proof. So the coverage line never gives a total
alone: it gives the attestations, then how many of them are proofs, which docattest runs
or evaluates itself (examples, facts and the rows of cases tables, weak examples among
them), and how many are citations. A broken citation, one that names no file, still counts
as a citation, never as a proof. The report object has the same counts: `proofs` and
`cited`, which add up to `attestations`.

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
put('docs/a.md', 'N is `3`. <!-- fact: N -->\n\nIt works. <!-- test: test/a.test.ts > "works" -->\n\nIt is tested. <!-- test: -->');
// ---cut---
const check = (...args: string[]) => spawnSync(process.execPath, [cli, 'check', ...args], { cwd: root, encoding: 'utf8' }).stdout;
assert.ok(check().split('\n').includes('coverage: 1 living page, 1 covered (3 attestations: 1 proof, 0 of them weak, and 2 citations), 0 uncovered, 0 frozen'));
const { attestations, proofs, weak, cited } = JSON.parse(check('--json')).coverage;
assert.deepEqual({ attestations, proofs, weak, cited }, { attestations: 3, proofs: 1, weak: 0, cited: 2 });
```

This page cites one of docattest's own tests: the npm launcher names platforms the way
every docattest package does. <!-- test: adapters/typescript/test/unit/core-binary/locate.test.ts > "the core lookup" > "names platforms the same in every package" -->
