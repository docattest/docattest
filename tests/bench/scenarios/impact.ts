// Benchmark scenarios: impact. ../scenarios.ts joins the parts, in order.
import { lines } from '../../lib/lines.ts';
import type { ProjectFiles } from '../../lib/project.ts';
import { lineOf, project } from '../fixture.ts';
import { affects, affectsNothing, refers } from '../judges.ts';
import { adder, type Judge, type Outcome, type Polarity, type Scenario } from '../scenario.ts';

export const scenarios: Scenario[] = [];
const add = adder(scenarios);

// =====================================================================================
// impact: from a diff to the prose it touches. The base is committed; the change is in
// the working tree. `retry` reaches `jitter` through `backoff`; `parse` stands apart.
// =====================================================================================
const IMPACT = ['impact', '--json'];
const impactSrc = {
  'src/index.ts': lines(
    "export { retry, RETRY_DEFAULTS } from './retry.js';",
    "export { parse } from './parse.js';",
  ),
  'src/retry.ts': lines(
    "import { backoff } from './backoff.js';",
    'export const RETRY_DEFAULTS = { maxAttempts: 3 };',
    'export async function retry<T>(fn: () => Promise<T>): Promise<T> {',
    '  for (let attempt = 1; ; attempt++) {',
    '    try {',
    '      return await fn();',
    '    } catch (err) {',
    '      if (attempt >= RETRY_DEFAULTS.maxAttempts) throw err;',
    '      await new Promise((done) => setTimeout(done, backoff(attempt)));',
    '    }',
    '  }',
    '}',
  ),
  'src/backoff.ts': lines(
    "import { jitter } from './jitter.js';",
    'export function backoff(attempt: number): number {',
    '  return jitter(attempt * 2);',
    '}',
  ),
  'src/jitter.ts': lines('export function jitter(ms: number): number {', '  return ms;', '}'),
  'src/parse.ts': lines(
    'export function parse(text: string): number {',
    '  return Number(text);',
    '}',
  ),
  'src/unused.ts': lines('export function unused(): number {', '  return 1;', '}'),
};
const impactRetryPage = lines(
  '# Retry',
  '',
  'A call is tried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->',
  '',
  'A successful call returns its value unchanged.',
  '',
  '```ts check',
  "import { retry } from 'fixture';",
  "assert.equal(await retry(async () => 'ok'), 'ok');",
  '```',
);
const impactParsePage = lines(
  '# Parse',
  '',
  'Text becomes a number.',
  '',
  '```ts check',
  "import { parse } from 'fixture';",
  "assert.equal(parse('4'), 4);",
  '```',
);
const impactGuidePage = lines(
  '# Guide',
  '',
  'Read the input with `parse` before you retry anything.',
);
const impactBase = project({
  ...impactSrc,
  'docs/retry.md': impactRetryPage,
  'docs/parse.md': impactParsePage,
  'docs/guide.md': impactGuidePage,
});
const impact = (
  polarity: Polarity,
  id: string,
  title: string,
  head: ProjectFiles<string>,
  expect: Judge,
): void => {
  add('impact', polarity, id, title, { files: impactBase, head, run: IMPACT, expect });
};
const retryExampleLine = lineOf(impactRetryPage, '```ts check');
const silentOn = (out: Outcome, page: string): boolean =>
  !(out.json?.references ?? []).some((r) => r.page === page);

impact(
  'positive',
  'impact-01',
  'an exported constant changed',
  { 'src/retry.ts': impactSrc['src/retry.ts'].replace('maxAttempts: 3', 'maxAttempts: 5') },
  (out) =>
    affects(out, 'RETRY_DEFAULTS') &&
    refers(out, {
      symbol: 'RETRY_DEFAULTS',
      page: 'docs/retry.md',
      line: 3,
      kind: 'fact',
      proven: true,
    }) &&
    silentOn(out, 'docs/parse.md'),
);
impact(
  'positive',
  'impact-02',
  'an internal one call below an export changed',
  { 'src/backoff.ts': impactSrc['src/backoff.ts'].replace('attempt * 2', 'attempt * 200') },
  (out) =>
    affects(out, 'retry') &&
    refers(out, {
      symbol: 'retry',
      page: 'docs/retry.md',
      line: retryExampleLine,
      kind: 'example',
      proven: true,
    }) &&
    silentOn(out, 'docs/parse.md'),
);
impact(
  'positive',
  'impact-03',
  'an internal two calls below an export changed',
  {
    'src/jitter.ts': impactSrc['src/jitter.ts'].replace('return ms;', 'return ms + Math.random();'),
  },
  (out) =>
    affects(out, 'retry') &&
    refers(out, { symbol: 'retry', page: 'docs/retry.md', line: retryExampleLine, proven: true }) &&
    !affects(out, 'parse'),
);
impact(
  'positive',
  'impact-04',
  'a symbol that one page proves and another only mentions',
  {
    'src/parse.ts': impactSrc['src/parse.ts'].replace('Number(text)', 'Number.parseInt(text, 10)'),
  },
  (out) =>
    affects(out, 'parse') &&
    refers(out, { symbol: 'parse', page: 'docs/parse.md', proven: true }) &&
    refers(out, {
      symbol: 'parse',
      page: 'docs/guide.md',
      line: 3,
      kind: 'mention',
      proven: false,
    }) &&
    silentOn(out, 'docs/retry.md'),
);

impact('negative', 'impact-n1', 'nothing changed', {}, (out) => affectsNothing(out));
impact(
  'negative',
  'impact-n2',
  'only a comment changed',
  { 'src/retry.ts': `// Retries a call.\n${impactSrc['src/retry.ts']}` },
  (out) => affectsNothing(out),
);
impact(
  'negative',
  'impact-n3',
  'a module no export reaches changed',
  { 'src/unused.ts': impactSrc['src/unused.ts'].replace('return 1;', 'return 2;') },
  (out) => affectsNothing(out),
);
impact(
  'negative',
  'impact-n4',
  'only a page changed',
  { 'docs/guide.md': impactGuidePage + '\nMore soon.\n' },
  (out) => affectsNothing(out),
);
