// Nothing changed, ordinary edits, and rewording.
import {
  change,
  CLAIM,
  EXAMPLE,
  FACT,
  LIST,
  page,
  PAGE,
  retry,
  type ChangeCase,
} from './fixtures.ts';

export const unchanged: ChangeCase[] = [
  // ----- nothing changed, and ordinary edits -----
  retry('unchanged', PAGE),
  retry('unchanged quiet', PAGE, [['check', '--base', 'HEAD', '--quiet']]),
  retry(
    'prose added above',
    page('Prose added.', '', 'More prose.', '', FACT, '', CLAIM, '', ...EXAMPLE),
  ),
  retry(
    'sections reordered',
    page('Prose added.', '', CLAIM, '', ...EXAMPLE, '', '## Limits', '', FACT),
  ),
  retry('claim deleted with its fact', page(CLAIM, '', ...EXAMPLE)),
  retry('claim deleted with its example', page(FACT)),
  retry(
    'proof replaced by another',
    page(FACT, '', CLAIM, '', '```ts check', 'assert.ok(true);', '```'),
  ),
  retry(
    'attestations added',
    page(FACT, '', CLAIM, '', ...EXAMPLE, '', ...LIST, '', 'Another `3`. <!-- fact: LIMIT -->'),
  ),
  change(
    'page renamed',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': null, 'docs/retrying.md': PAGE },
  ),
  change('page deleted', { 'docs/retry.md': PAGE }, { 'docs/retry.md': null }),
  change(
    'page split in two',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': page(FACT), 'docs/values.md': page(CLAIM, '', ...EXAMPLE) },
  ),
  change(
    'fact moved to another living page',
    { 'docs/retry.md': PAGE, 'docs/b.md': '# B\n' },
    { 'docs/retry.md': page(CLAIM, '', ...EXAMPLE), 'docs/b.md': `# B\n\n${FACT}\n` },
  ),
  change(
    'example turned into a fact',
    { 'docs/l.md': page('The limit is `3` calls.', '', ...EXAMPLE) },
    { 'docs/l.md': page('The limit is `3` calls. <!-- fact: LIMIT -->') },
  ),
];

export const rewording: ChangeCase[] = [
  // ----- rewording -----
  retry(
    'reworded lightly, proof kept',
    page('A call is attempted at most `3` times. <!-- fact: LIMIT -->', '', CLAIM, '', ...EXAMPLE),
  ),
  retry(
    'reworded lightly, proof deleted',
    page('A call is attempted at most `3` times.', '', CLAIM, '', ...EXAMPLE),
  ),
  retry(
    'reworded heavily, proof deleted',
    page('Calls are retried up to `3` times in total.', '', CLAIM, '', ...EXAMPLE),
  ),
  retry(
    'reworded with another literal',
    page('A call is tried at most `4` times.', '', CLAIM, '', ...EXAMPLE),
  ),
  retry(
    'reworded lightly twice over',
    page(
      'A call is attempted at most `3` times.',
      '',
      'A call is retried at most `3` times.',
      '',
      CLAIM,
      '',
      ...EXAMPLE,
    ),
  ),
  retry(
    'claim reworded lightly and expression edited',
    page(
      'A call is attempted at most `3` times. <!-- fact: Math.min(LIMIT, 3) -->',
      '',
      CLAIM,
      '',
      ...EXAMPLE,
    ),
  ),
  retry(
    'claim reworded heavily and expression edited',
    page(
      'Calls are retried up to `3` times in total. <!-- fact: Math.min(LIMIT, 3) -->',
      '',
      CLAIM,
      '',
      ...EXAMPLE,
    ),
  ),
  retry(
    'example claim reworded lightly, example edited',
    page(
      FACT,
      '',
      'A successful call returns the value unchanged.',
      '',
      ...EXAMPLE.slice(0, 2),
      'console.log(LIMIT);',
      '```',
    ),
  ),
  retry(
    'case and punctuation only',
    page('a call is tried, at most, `3` times', '', CLAIM, '', ...EXAMPLE),
  ),
];
