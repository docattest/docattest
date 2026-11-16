// Benchmark scenarios: guard. ../scenarios.ts joins the parts, in order.
import { lines } from '../../lib/lines.ts';
import type { ProjectFiles } from '../../lib/project.ts';
import { CONFIG, lineOf, project } from '../fixture.ts';
import { reportsChange, reportsNoChange } from '../judges.ts';
import { adder, type Judge, type Polarity, type Scenario } from '../scenario.ts';

export const scenarios: Scenario[] = [];
const add = adder(scenarios);

// =====================================================================================
// guard: `check --base <ref>` judges the change. The base is committed; the head is the
// working tree. Three ways to go green while the docs go wrong: delete, freeze, weaken.
// =====================================================================================
const GUARD = ['check', '--base', 'HEAD', '--json'];
export const RETRY_SRC = lines(
  'export const RETRY_DEFAULTS = { maxAttempts: 3 };',
  'export async function retry<T>(fn: () => Promise<T>): Promise<T> {',
  '  return fn();',
  '}',
);
const FACT_LINE = 'A call is tried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->';
const CLAIM_LINE = 'A successful call returns its value unchanged.';
const EXAMPLE = [
  '```ts check',
  "import { retry } from 'fixture';",
  "assert.equal(await retry(async () => 'ok'), 'ok');",
  'assert.equal(await retry(async () => 7), 7);',
  '```',
];
/** The retry page; a `null` fact or claim leaves that line out. */
interface RetryPage {
  fact?: string | null;
  claim?: string | null;
  example?: string[];
  top?: string[];
}
const retryPage = ({
  fact = FACT_LINE,
  claim = CLAIM_LINE,
  example = EXAMPLE,
  top = [],
}: RetryPage = {}): string =>
  lines(
    '# Retry',
    '',
    ...top,
    ...(fact ? [fact, ''] : []),
    ...(claim ? [claim, ''] : []),
    ...example,
  );
const guardBase = project({ 'src/index.ts': RETRY_SRC, 'docs/retry.md': retryPage() });
const guard = (
  polarity: Polarity,
  id: string,
  title: string,
  head: ProjectFiles<string>,
  expect: Judge,
): void => {
  add('guard', polarity, id, title, { files: guardBase, head, run: GUARD, expect });
};

{
  const page = retryPage({ fact: 'A call is tried at most `3` times.' });
  guard(
    'positive',
    'guard-01',
    'fact directive deleted, its sentence kept',
    { 'docs/retry.md': page },
    (out) =>
      out.code === 1 &&
      reportsChange(out, {
        type: 'orphaned-claim',
        page: 'docs/retry.md',
        line: lineOf(page, 'A call is tried'),
        severity: 'fail',
      }),
  );
}
{
  const page = retryPage({ example: [] });
  guard(
    'positive',
    'guard-02',
    'example deleted, its sentence kept',
    { 'docs/retry.md': page },
    (out) =>
      out.code === 1 &&
      reportsChange(out, {
        type: 'orphaned-claim',
        page: 'docs/retry.md',
        line: lineOf(page, CLAIM_LINE),
        severity: 'fail',
      }),
  );
}
{
  // The realistic cheat: the code changed, the page is now false, the proof is removed.
  const page = retryPage({ fact: 'A call is tried at most `3` times.' });
  guard(
    'positive',
    'guard-03',
    'code changed and the contradicting fact removed',
    {
      'src/index.ts': RETRY_SRC.replace('maxAttempts: 3', 'maxAttempts: 5'),
      'docs/retry.md': page,
    },
    (out) =>
      out.code === 1 && reportsChange(out, { type: 'orphaned-claim', page: 'docs/retry.md' }),
  );
}
guard(
  'positive',
  'guard-04',
  'attested page frozen in the config',
  { 'docattest.config.json': JSON.stringify({ ...CONFIG, frozen: ['docs/retry.md'] }) },
  (out) => reportsChange(out, { type: 'frozen-page', page: 'docs/retry.md' }),
);
guard(
  'positive',
  'guard-05',
  'attested page frozen by frontmatter',
  { 'docs/retry.md': `---\ndocattest: false\n---\n\n${retryPage()}` },
  (out) => reportsChange(out, { type: 'frozen-page', page: 'docs/retry.md' }),
);
{
  const page = retryPage({ example: ['```ts compile', ...EXAMPLE.slice(1)] });
  guard(
    'positive',
    'guard-06',
    'check block downgraded to compile',
    { 'docs/retry.md': page },
    (out) =>
      reportsChange(out, {
        type: 'downgrade',
        page: 'docs/retry.md',
        line: lineOf(page, '```ts compile'),
      }),
  );
}
{
  const page = retryPage({
    example: [...EXAMPLE.slice(0, 2), "await retry(async () => 'ok');", '```'],
  });
  guard(
    'positive',
    'guard-07',
    'every assertion removed from an example',
    { 'docs/retry.md': page },
    (out) =>
      reportsChange(out, {
        type: 'downgrade',
        page: 'docs/retry.md',
        line: lineOf(page, '```ts check'),
      }),
  );
}
{
  const page = retryPage({
    fact: 'A call is tried at most `3` times. <!-- fact: Math.min(RETRY_DEFAULTS.maxAttempts, 3) -->',
  });
  guard(
    'positive',
    'guard-08',
    'fact expression changed, literal unchanged',
    { 'docs/retry.md': page },
    (out) =>
      reportsChange(out, {
        type: 'weakened',
        page: 'docs/retry.md',
        line: lineOf(page, 'A call is tried'),
      }),
  );
}

guard('negative', 'guard-n1', 'nothing changed', {}, (out) => reportsNoChange(out));
guard(
  'negative',
  'guard-n2',
  'sentence and its proof deleted together',
  { 'docs/retry.md': retryPage({ fact: null }) },
  (out) => reportsNoChange(out),
);
guard(
  'negative',
  'guard-n3',
  'prose added above, every attestation shifted down',
  {
    'docs/retry.md': retryPage({
      top: ['Retrying is on by default.', '', 'It can be turned off per call.', ''],
    }),
  },
  (out) => reportsNoChange(out),
);
guard(
  'negative',
  'guard-n4',
  'a new attestation added',
  {
    'docs/retry.md':
      retryPage() +
      lines(
        '',
        'The defaults are an object.',
        '',
        '```ts check',
        "import { RETRY_DEFAULTS } from 'fixture';",
        "assert.equal(typeof RETRY_DEFAULTS, 'object');",
        '```',
      ),
  },
  (out) => reportsNoChange(out),
);
guard(
  'negative',
  'guard-n5',
  'the honest fix: code and literal changed together',
  {
    'src/index.ts': RETRY_SRC.replace('maxAttempts: 3', 'maxAttempts: 5'),
    'docs/retry.md': retryPage({ fact: FACT_LINE.replace('`3`', '`5`') }),
  },
  (out) => reportsNoChange(out),
);
guard(
  'negative',
  'guard-n6',
  'an attested sentence reworded, proof kept',
  {
    'docs/retry.md': retryPage({
      claim: 'When the call succeeds, its value comes back unchanged.',
    }),
  },
  (out) => reportsNoChange(out),
);
