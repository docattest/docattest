// `runTests` over the built server: cited tests of a tiny project on disk, run by node:test
// and by Vitest, and of a project in memory, where nothing runs.
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { byId, exchange, HELLO, open } from './exchange.ts';

/** The repository's own installed packages, Vitest among them, which the project links. */
const NODE_MODULES = fileURLToPath(new URL('../../../../node_modules', import.meta.url));

const MATH = [
  "import { describe, test } from 'node:test';",
  "import assert from 'node:assert/strict';",
  "test('adds', () => { assert.equal(1 + 1, 2); });",
  "test('subtracts', () => { assert.equal(2 - 1, 0, 'two minus one is not zero'); });",
  "test.skip('multiplies', () => {});",
  "test('divides', (t) => { t.skip('no divisor today'); });",
  "if (process.env.NEVER) test('ghost', () => {});",
  "const cases = [{ name: 'rounds down', cents: 1 }, { name: 'rounds up', cents: 5 }];",
  "describe('rounding', () => {",
  '  for (const c of cases) {',
  '    test(c.name, () => { assert.ok(c.cents < 5, `${c.cents} cents is too many`); });',
  '  }',
  '});',
  "test('rounding rounds up', () => {});",
  "test('has cases', async (t) => {",
  "  await t.test('first', () => {});",
  "  await t.test('second', () => { assert.fail('the second case broke'); });",
  '});',
];

const PRICE = [
  "import { describe, expect, it, test } from 'vitest';",
  "describe('price', () => {",
  "  it.each([[1, 2], [2, 4]])('doubles %i to %i', (a, b) => { expect(a * 2).toBe(b); });",
  "  it.each([[1, 2], [2, 5]])('adds one to %i giving %i', (a, b) => { expect(a + 1).toBe(b); });",
  "  it('rounds', () => { expect(1, 'one is not two').toBe(2); });",
  "  it.skip('discounts', () => {});",
  "  if (process.env.NEVER) it('ghost', () => {});",
  '});',
  "test('price rounds', () => {});",
];

// Tables whose rows are built as the file runs, so the file does not tell their titles.
const BUILT = [
  "import { describe, expect, it } from 'vitest';",
  'const rows = JSON.parse(\'[{"name":"one","n":1},{"name":"two","n":2}]\');',
  "describe('built', () => {",
  "  it.each(rows)('$name', ({ n }) => { expect(n).toBeGreaterThan(0); });",
  "  it('alone', () => { expect(1, 'alone is not a row').toBe(2); });",
  '});',
  "describe('built badly', () => {",
  "  it.each(rows)('$name', ({ n }) => { expect(n, 'too small').toBeGreaterThan(1); });",
  '});',
];

const SLOW = [
  "import { test } from 'node:test';",
  "test('waits', async () => { await new Promise((resolve) => setTimeout(resolve, 20_000)); });",
];

let root = '';

beforeAll(() => {
  root = mkdtempSync(join(tmpdir(), 'docattest-run-tests-'));
  const write = (file: string, lines: string[]) => {
    mkdirSync(dirname(join(root, file)), { recursive: true });
    writeFileSync(join(root, file), lines.join('\n'));
  };
  write('package.json', [JSON.stringify({ name: 'tiny', type: 'module' })]);
  write('src/index.ts', ['export const N = 3;']);
  write('test/math.test.ts', MATH);
  write('test/slow.test.ts', SLOW);
  write('shop/package.json', [JSON.stringify({ name: 'shop', devDependencies: { vitest: '*' } })]);
  write('shop/test/price.test.ts', PRICE);
  write('shop/test/built.test.ts', BUILT);
  symlinkSync(NODE_MODULES, join(root, 'node_modules'), 'dir');
});

afterAll(() => {
  rmSync(root, { recursive: true, force: true });
});

/** Run the cited tests of the project on disk, each `[file, ...describes, title]`. */
function runTests(cited: string[][], timeoutMs = 60_000) {
  const tests = cited.map(([file = '', ...titles]) => ({
    file,
    describes: titles.slice(0, -1),
    title: titles.at(-1),
  }));
  // Without `files`, the project is the one on disk at `root`.
  const params = { ...open(1, {}).params, files: undefined, root };
  const x = exchange([
    HELLO,
    { id: 1, method: 'open', params },
    { id: 2, method: 'runTests', params: { tests, timeoutMs } },
  ]);
  return byId(x, 2)?.result as { status: string; message?: string }[];
}

describe('runTests', () => {
  it(
    'runs node:test tests by their exact path, a case of a table one alone',
    { timeout: 60_000 },
    () => {
      const file = 'test/math.test.ts';
      const runs = runTests([
        [file, 'adds'],
        [file, 'subtracts'],
        [file, 'multiplies'],
        [file, 'divides'],
        [file, 'ghost'],
        [file, 'rounding', 'rounds down'],
        [file, 'rounding', 'rounds up'],
        [file, 'rounding rounds up'],
        [file, 'has cases'],
        [file, 'never declared'],
      ]);
      expect(runs).toEqual([
        { status: 'passed' },
        { status: 'failed', message: expect.stringMatching(/^two minus one is not zero/) },
        { status: 'skipped', message: 'node:test skipped it' },
        { status: 'skipped', message: 'no divisor today' },
        { status: 'notRun', message: 'node:test selected no test named "ghost"' },
        { status: 'passed' },
        { status: 'failed', message: '5 cents is too many' },
        { status: 'passed' },
        { status: 'failed', message: 'second: the second case broke' },
        { status: 'notRun', message: 'test/math.test.ts declares no test named "never declared"' },
      ]);
    },
  );

  it(
    "runs Vitest tests from the package that declares it, a table's rows together",
    { timeout: 60_000 },
    () => {
      const file = 'shop/test/price.test.ts';
      const runs = runTests([
        [file, 'price', 'doubles %i to %i'],
        [file, 'price', 'adds one to %i giving %i'],
        [file, 'price', 'adds one to 1 giving 2'],
        [file, 'price', 'rounds'],
        [file, 'price rounds'],
        [file, 'price', 'discounts'],
        [file, 'price', 'ghost'],
      ]);
      expect(runs).toEqual([
        { status: 'passed' },
        { status: 'failed', message: expect.stringMatching(/^adds one to 2 giving 5: /) },
        { status: 'passed' },
        { status: 'failed', message: expect.stringContaining('one is not two') },
        { status: 'passed' },
        { status: 'skipped', message: 'Vitest skipped it' },
        { status: 'notRun', message: 'Vitest selected no test named "price" > "ghost"' },
      ]);
    },
  );

  it(
    'runs the rows of a table built as the file runs, by its describe titles, as one',
    { timeout: 60_000 },
    () => {
      const file = 'shop/test/built.test.ts';
      const runs = runTests([
        [file, 'built', '$name'],
        [file, 'built badly', '$name'],
      ]);
      expect(runs).toEqual([
        { status: 'passed' },
        { status: 'failed', message: expect.stringMatching(/^'one': .*too small/) },
      ]);
    },
  );

  it('fails every test of a run that does not finish in time', { timeout: 30_000 }, () => {
    const started = Date.now();
    const runs = runTests([['test/slow.test.ts', 'waits']], 1_000);
    expect(runs).toEqual([{ status: 'failed', message: 'node:test did not finish within 1 s' }]);
    // The test waits 20 s: the run was stopped, not waited for.
    expect(Date.now() - started).toBeLessThan(10_000);
  });

  it('runs nothing of a project in memory', () => {
    const x = exchange([
      HELLO,
      open(1, { 'test/a.test.ts': "import { test } from 'node:test';\ntest('a', () => {});" }),
      {
        id: 2,
        method: 'runTests',
        params: { tests: [{ file: 'test/a.test.ts', describes: [], title: 'a' }], timeoutMs: 1 },
      },
      { id: 3, method: 'runTests', params: { tests: [{ file: 'test/a.test.ts' }], timeoutMs: 1 } },
    ]);
    expect(byId(x, 2)?.result).toEqual([
      {
        status: 'notRun',
        message: 'the project is in memory (open.files), so none of its tests can run',
      },
    ]);
    expect(byId(x, 3)?.error?.message).toBe(
      'runTests: each of "tests" must have a "file", "describes" and a "title"',
    );
  });
});
