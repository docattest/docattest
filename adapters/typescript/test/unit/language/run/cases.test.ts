import { mkdirSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { createProject } from 'docattest';

/** A project whose setup module is `setup`, and whose page holds `tables`. */
function check(setup: string[], tables: string[], runtime: Record<string, number> = {}) {
  return createProject({
    files: {
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/index.ts' },
        setup: 'docs/setup.ts',
        adapter: 'typescript',
        runtime,
      }),
      'src/index.ts': 'export const x = 1;',
      'docs/setup.ts': setup.join('\n'),
      'docs/t.md': tables.join('\n\n'),
    },
  }).check();
}

const table = (runner: string, rows: string[]) =>
  [`<!-- cases: ${runner} -->`, '| case | value |', '|---|---|', ...rows].join('\n');

// A runner that counts its rows in the setup module's own state, and fails a row whose
// value is not the count so far.
const COUNTER = [
  "import assert from 'node:assert/strict';",
  'let count = 0;',
  'export const cases = {',
  '  count: (row: Record<string, string>) => {',
  '    count += 1;',
  "    if (row.value === 'throw') throw new Error('thrown');",
  "    if (row.value === 'reject') return Promise.reject(new Error('rejected'));",
  "    if (row.value === 'spin') for (;;);",
  "    if (row.value === 'exit') process.exit(3);",
  '    assert.equal(String(count), row.value);',
  '  },',
  '};',
];

describe('the rows of a cases table', () => {
  it('run one after another, in the page order, sharing the setup module and its state', async () => {
    const report = await check(COUNTER, [table('count', ['| a | 1 |', '| b | 2 |', '| c | 3 |'])]);
    expect(report.results.map((r) => r.status)).toEqual(['pass', 'pass', 'pass']);
  });

  it('go on after a row that throws or rejects, in the same worker', async () => {
    const rows = ['| a | 1 |', '| b | throw |', '| c | reject |', '| d | 4 |'];
    const report = await check(COUNTER, [table('count', rows)]);
    expect(report.results.map((r) => [r.status, r.message])).toEqual([
      ['pass', 'docs/t.md:4 a'],
      ['fail', 'docs/t.md:5 b: Error: thrown'],
      ['fail', 'docs/t.md:6 c: Error: rejected'],
      ['pass', 'docs/t.md:7 d'],
    ]);
  });

  it('start again on a fresh worker after a row that times out or exits', async () => {
    const rows = ['| a | 1 |', '| b | spin |', '| c | 1 |', '| d | exit |', '| e | 1 |'];
    const report = await check(COUNTER, [table('count', rows)], { timeoutMs: 1000 });
    expect(report.results.map((r) => [r.status, r.message])).toEqual([
      ['pass', 'docs/t.md:4 a'],
      ['fail', 'docs/t.md:5 b: timed out after 1000 ms'],
      ['pass', 'docs/t.md:6 c'],
      ['fail', 'docs/t.md:7 d: the worker exited with code 3'],
      ['pass', 'docs/t.md:8 e'],
    ]);
  });
});

describe('cases tables', () => {
  it('each load the setup module afresh, but share what a worker keeps between jobs', async () => {
    // One worker runs both tables, one after the other: the module's count starts again,
    // and the interval the first table left running still ticks on `globalThis`.
    const setup = [
      ...COUNTER.slice(0, -1),
      '  tick: () => {',
      '    const shared = globalThis as { ticks?: number };',
      '    shared.ticks ??= 0;',
      '    setInterval(() => { shared.ticks = (shared.ticks ?? 0) + 1; }, 10).unref();',
      '  },',
      '  ticked: async () => {',
      '    await new Promise((resolve) => setTimeout(resolve, 50));',
      "    assert.ok(((globalThis as { ticks?: number }).ticks ?? 0) > 0, 'no ticks');",
      '  },',
      '};',
    ];
    const tables = [
      table('count', ['| a | 1 |']),
      table('tick', ['| start | x |']),
      table('ticked', ['| still | x |']),
      table('count', ['| again | 1 |']),
    ];
    const report = await check(setup, tables, { workers: 1 });
    expect(report.results.map((r) => r.status)).toEqual(['pass', 'pass', 'pass', 'pass']);
  });

  it('run side by side, as many at once as there are workers', async () => {
    // Each table's one row waits for the other table's row to have started.
    const dir = mkdtempSync(join(tmpdir(), 'docattest-cases-'));
    const setup = [
      "import { existsSync, writeFileSync } from 'node:fs';",
      "import { join } from 'node:path';",
      'const meet = async (dir: string, me: string, other: string) => {',
      "  writeFileSync(join(dir, me), '');",
      '  for (let k = 0; k < 100 && !existsSync(join(dir, other)); k++) {',
      '    await new Promise((resolve) => setTimeout(resolve, 20));',
      '  }',
      '  if (!existsSync(join(dir, other))) throw new Error(`${other} never started`);',
      '};',
      'export const cases = {',
      "  a: (row: Record<string, string>) => meet(row.value ?? '', 'a', 'b'),",
      "  b: (row: Record<string, string>) => meet(row.value ?? '', 'b', 'a'),",
      '};',
    ];
    const tables = (sub: string) => [
      table('a', [`| a | ${sub} |`]),
      table('b', [`| b | ${sub} |`]),
    ];
    for (const sub of ['one', 'two']) mkdirSync(join(dir, sub));
    const together = await check(setup, tables(join(dir, 'one')), { workers: 2 });
    expect(together.results.map((r) => r.status)).toEqual(['pass', 'pass']);
    const alone = await check(setup, tables(join(dir, 'two')), { workers: 1 });
    expect(alone.results.map((r) => r.status)).toContain('fail');
  });
});
