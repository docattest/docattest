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

/**
 * `result` is `pass`, `fail: <substring of message>` or `error: <substring of message>`, of
 * the page's one fact: a citation beside it is there for where it sits, and cites no test.
 */
function expectResult(report: Report, result: string): void {
  const [status = '', ...rest] = result.split(':');
  const needle = rest.join(':').trim();
  const facts = report.results.filter((r) => r.kind === 'fact');
  assert.equal(facts.length, 1, `expected exactly one fact, got ${facts.length}`);
  const [only] = facts;
  assert.ok(only);
  assert.equal(only.status, status.trim());
  if (needle) {
    assert.ok(
      only.message.includes(needle),
      `message "${only.message}" should include "${needle}"`,
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
};
