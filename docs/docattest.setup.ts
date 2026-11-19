// Case runners for docattest's own docs. Runners know how to execute a row; the rows in the
// pages hold the expectations.
import assert from 'node:assert/strict';
import { createProject, type Report } from 'docattest';

const SURFACE = [
  'export const N = 3;',
  "export const KEYS = ['A', 'B', 'C'];",
  "export const ROUTES = [{ method: 'GET', path: '/a' }, { method: 'POST', path: '/b' }];",
  'export const SHAPE = `export interface Point { x: number; y: number }`;',
  "export const VERSION = '1.2.3-rc.1';",
  'export const STRICT = true;',
  'export const RATIO = 1.0;',
].join('\n');

const SHIPPING_SURFACE = [
  'export const FREE_FROM = 50;',
  "export const ZONES = ['EU', 'UK', 'US'];",
].join('\n');

const CONFIG = JSON.stringify({
  pages: ['docs/**/*.md'],
  surfaces: { default: './src/index.ts', shipping: './src/shipping.ts' },
  adapter: 'typescript',
});

/** A cell of a row; the runner's declared columns guarantee it is there. */
function cell(row: Record<string, string>, column: string): string {
  const value = row[column];
  assert.ok(value !== undefined, `the row has no "${column}" cell`);
  return value;
}

/** The tests docs/citation-shorthands.md cites, three cases of one table-driven test. */
const LAUNCH_TESTS = [
  "describe('run', () => {",
  "  for (const script of ['python', 'node', 'ruby']) it(script, () => {});",
  '});',
].join('\n');

/**
 * `result` is `pass`, `fail: <substring of message>` or `error: <substring of message>`, of
 * the page's one fact: a citation beside it is there for where it sits, and cites no test.
 */
function expectResult(report: Report, result: string): void {
  const facts = report.results.filter((r) => r.kind === 'fact');
  assert.equal(facts.length, 1, `expected exactly one fact, got ${facts.length}`);
  const [only] = facts;
  assert.ok(only);
  expectOutcome(only, result);
}

/**
 * `results` is one `result`, as {@link expectResult} reads it, for each result, in order,
 * parted by `;`, or `none` for no result.
 */
function expectResults(report: Report, results: string): void {
  const expected = results === 'none' ? [] : results.split(';');
  assert.equal(report.results.length, expected.length, `expected ${expected.length} results`);
  report.results.forEach((r, k) => {
    expectOutcome(r, expected[k] ?? '');
  });
}

function expectOutcome(outcome: Report['results'][number], result: string): void {
  const [status = '', ...rest] = result.split(':');
  const needle = rest.join(':').trim();
  assert.equal(outcome.status, status.trim(), outcome.message);
  if (needle) {
    assert.ok(
      outcome.message.includes(needle),
      `message "${outcome.message}" should include "${needle}"`,
    );
  }
}

export const cases = {
  /** docs/facts.md: how a fact finds and compares its target. */
  factBinding: Object.assign(
    async (row: Record<string, string>) => {
      const page = cell(row, 'page').replaceAll('¶', '\n').replaceAll('\\|', '|');
      const report = await createProject({
        files: {
          'docattest.config.json': CONFIG,
          'src/index.ts': SURFACE,
          'src/shipping.ts': SHIPPING_SURFACE,
          'docs/page.md': page + '\n',
        },
      }).check();
      expectResult(report, cell(row, 'result'));
    },
    { columns: ['page', 'result'] },
  ),
  /** docs/explain.md: the rule that bound the page's one fact, and where it took effect. */
  explainedRule: Object.assign(
    async (row: Record<string, string>) => {
      const page = cell(row, 'page').replaceAll('¶', '\n').replaceAll('\\|', '|');
      const explained = await createProject({
        files: {
          'docattest.config.json': CONFIG,
          'src/index.ts': SURFACE,
          'src/shipping.ts': SHIPPING_SURFACE,
          'docs/page.md': page + '\n',
        },
      }).explain('docs/page.md');
      const [fact] = explained.directives;
      assert.ok(fact?.kind === 'fact', 'the page has one fact');
      assert.equal(fact.rule, cell(row, 'rule'));
      assert.equal(fact.placed.where, cell(row, 'placed'));
    },
    { columns: ['page', 'rule', 'placed'] },
  ),
  /** docs/citation-shorthands.md: what a citation cites, beside a `Makefile` and the tests. */
  citations: Object.assign(
    async (row: Record<string, string>) => {
      const page = cell(row, 'page').replaceAll('¶', '\n');
      const report = await createProject({
        files: {
          'docattest.config.json': CONFIG,
          'src/index.ts': SURFACE,
          'src/shipping.ts': SHIPPING_SURFACE,
          'test/launch.test.ts': LAUNCH_TESTS,
          Makefile: 'test:\n\tnpm test\n',
          'docs/page.md': page + '\n',
        },
      }).check();
      expectResults(report, cell(row, 'results'));
    },
    { columns: ['page', 'results'] },
  ),
};
