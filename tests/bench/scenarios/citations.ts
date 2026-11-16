// Benchmark scenarios: citations. ../scenarios.ts joins the parts, in order.
import { lines } from '../../lib/lines.ts';
import { project } from '../fixture.ts';
import { answered, cites } from '../judges.ts';
import { adder, type Judge, type Polarity, type Scenario } from '../scenario.ts';
import { RETRY_SRC } from './guard.ts';

export const scenarios: Scenario[] = [];
const add = adder(scenarios);

// =====================================================================================
// citations: `<!-- test: <file> > "<title>" -->` binds a sentence to an existing test.
// =====================================================================================
const CHECK = ['check', '--json'];
const TEST_FILE = lines(
  "import { describe, it, expect } from 'vitest';",
  "import { retry } from '../src/index.js';",
  "describe('retry', () => {",
  "  it('throws the last error', async () => {",
  "    await expect(retry(async () => { throw new Error('x'); })).rejects.toThrow('x');",
  '  });',
  '});',
);
const citePage = (directive: string): string =>
  lines('# Retry', '', `A call that keeps failing throws the last error. ${directive}`);
// `directive` is the citation the page carries; `files`, what the project has besides.
const cite = (
  polarity: Polarity,
  id: string,
  title: string,
  { directive, files }: { directive: string; files: Record<string, string> },
  expect: Judge,
): void => {
  add('citations', polarity, id, title, {
    files: project({ 'src/index.ts': RETRY_SRC, 'docs/retry.md': citePage(directive), ...files }),
    run: CHECK,
    expect,
  });
};
const CITATION = '<!-- test: test/retry.test.ts > "throws the last error" -->';

cite(
  'positive',
  'cite-01',
  'a cited test that exists',
  { directive: CITATION, files: { 'test/retry.test.ts': TEST_FILE } },
  (out) =>
    out.code === 0 &&
    cites(out, { page: 'docs/retry.md', line: 3, status: 'pass' }) &&
    !(out.json?.coverage?.uncovered ?? ['docs/retry.md']).includes('docs/retry.md'),
);
cite(
  'positive',
  'cite-02',
  'the cited test was renamed',
  {
    directive: CITATION,
    files: { 'test/retry.test.ts': TEST_FILE.replace('throws the last error', 'rethrows') },
  },
  (out) => out.code === 1 && cites(out, { page: 'docs/retry.md', line: 3, status: 'error' }),
);
cite(
  'positive',
  'cite-03',
  'the cited file is gone',
  { directive: CITATION, files: {} },
  (out) => out.code === 1 && cites(out, { page: 'docs/retry.md', line: 3, status: 'error' }),
);
cite(
  'positive',
  'cite-04',
  'a citation with no test title fails closed',
  { directive: '<!-- test: test/retry.test.ts -->', files: { 'test/retry.test.ts': TEST_FILE } },
  (out) =>
    out.code === 1 &&
    (out.json?.results ?? []).some((r) => r.page === 'docs/retry.md' && r.status === 'error'),
);

cite(
  'negative',
  'cite-n1',
  'an ordinary comment that starts with the word testing',
  {
    directive: '<!-- testing notes: revisit this wording -->',
    files: { 'test/retry.test.ts': TEST_FILE },
  },
  (out) => answered(out, 0) && out.json?.results?.length === 0,
);
