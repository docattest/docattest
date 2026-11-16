// What makes a page prove less: freezing it, downgrading an attestation, weakening a fact.
import {
  BARE,
  BLOCK,
  CELL,
  change,
  CLAIM,
  COLUMN,
  config,
  doc,
  DOC_EXAMPLE,
  EXAMPLE,
  FACT,
  LIST,
  ORDERED,
  page,
  PAGE,
  retry,
  SRC,
  type ChangeCase,
} from './fixtures.ts';

export const frozen: ChangeCase[] = [
  // ----- frozen pages -----
  change(
    'frozen by the config',
    { 'docs/retry.md': PAGE },
    { 'docattest.config.json': config({ frozen: ['docs/retry.md'] }) },
  ),
  retry('frozen by frontmatter', `---\ndocattest: false\n---\n${PAGE}`),
  retry('frontmatter that does not freeze', `---\ntitle: Retry\n---\n${PAGE}`),
  change(
    'dropped from pages',
    { 'docs/retry.md': PAGE },
    { 'docattest.config.json': config({ pages: ['docs/other/**'] }) },
  ),
  change(
    'moved into a frozen directory',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': null, 'docs/old/retry.md': PAGE },
  ),
  change(
    'moved into a frozen directory with frontmatter',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': null, 'docs/old/retry.md': `---\r\ndocattest: false\r\n---\r\n${PAGE}` },
  ),
  change(
    'moved out of pages',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': null, 'notes/Retry.MD': PAGE },
  ),
  change(
    'moved out of pages and edited',
    { 'docs/retry.md': PAGE },
    {
      'docs/retry.md': null,
      'notes/retry.md': page('Other prose.', '', ...EXAMPLE),
      'notes/z.md': page(FACT),
    },
  ),
  change(
    'two pages moved into one unchecked file',
    { 'docs/a.md': page(FACT), 'docs/b.md': page(CLAIM, '', ...EXAMPLE) },
    { 'docs/a.md': null, 'docs/b.md': null, 'archive/all.markdown': PAGE },
  ),
  change(
    'source dropped from sources',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'docattest.config.json': config({ sources: ['lib/**/*.ts'] }) },
  ),
  change(
    'sources dropped altogether',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'docattest.config.json': config({ sources: undefined }) },
  ),
  change(
    'frozen at the base, living now',
    { 'docs/retry.md': `---\ndocattest: false\n---\n${PAGE}` },
    { 'docs/retry.md': PAGE },
  ),
  change(
    'frozen page and an orphan elsewhere',
    { 'docs/retry.md': PAGE, 'docs/b.md': page(...LIST) },
    {
      'docs/retry.md': `---\ndocattest: false\n---\n${PAGE}`,
      'docs/b.md': page('The keys are:', '- `A`', '- `B`'),
    },
  ),
  change(
    'pages glob renamed with its directory',
    { 'docattest.config.json': config({ pages: ['doc/**/*.md'] }), 'doc/retry.md': PAGE },
    {
      'docattest.config.json': config(),
      'doc/retry.md': null,
      'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE),
    },
  ),
];

export const downgrades: ChangeCase[] = [
  // ----- downgrades -----
  retry('check became compile', page(FACT, '', CLAIM, '', '```ts compile', ...EXAMPLE.slice(1))),
  retry(
    'lost every assertion',
    page(FACT, '', CLAIM, '', ...EXAMPLE.slice(0, 2), 'console.log(LIMIT);', '```'),
  ),
  retry(
    'assertion refactored',
    page(FACT, '', CLAIM, '', ...EXAMPLE.slice(0, 2), 'assert.ok(LIMIT === 3);', '```'),
  ),
  change(
    'untagged with only a heading above',
    { 'docs/u.md': ['# Retry', '', ...EXAMPLE, ''].join('\n') },
    {
      'docs/u.md': ['# Retry', '', 'New prose.', '', '```ts [title]', ...EXAMPLE.slice(1), ''].join(
        '\n',
      ),
    },
  ),
  change(
    'compile block untagged',
    { 'docs/u.md': ['# Retry', '', '```ts compile', 'const n: number = 1;', '```', ''].join('\n') },
    { 'docs/u.md': ['# Retry', '', '```ts', 'const   n: number = 1;', '```', ''].join('\n') },
  ),
  change(
    'untagged and its language changed',
    { 'docs/u.md': ['# Retry', '', ...EXAMPLE, ''].join('\n') },
    { 'docs/u.md': ['# Retry', '', '```js', ...EXAMPLE.slice(1), ''].join('\n') },
  ),
  change(
    'compile became check',
    { 'docs/u.md': page(CLAIM, '', '```ts compile', ...EXAMPLE.slice(1)) },
    { 'docs/u.md': page(CLAIM, '', ...EXAMPLE) },
  ),
  change(
    'doc-comment example became compile',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'src/double.ts': doc(DOC_EXAMPLE.replace('// check', '// compile')) },
  ),
  change(
    'doc-comment example lost its assertion',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'src/double.ts': doc(DOC_EXAMPLE.replace('assert.equal(double(2), 4);', 'double(2);')) },
  ),
  change(
    'renamed page and downgraded',
    { 'docs/retry.md': PAGE },
    {
      'docs/retry.md': null,
      'docs/new.md': page(FACT, '', CLAIM, '', '```ts compile', ...EXAMPLE.slice(1)),
    },
  ),
];

export const weakened: ChangeCase[] = [
  // ----- weakened facts -----
  retry(
    'expression changed, literal kept',
    page(FACT.replace('LIMIT', 'Math.min(LIMIT, 3)'), '', CLAIM, '', ...EXAMPLE),
  ),
  change(
    'expression and literal changed',
    { 'docs/retry.md': PAGE },
    {
      'docs/retry.md': page(
        FACT.replaceAll('3', '5').replace('LIMIT', 'LIMIT + 2'),
        '',
        CLAIM,
        '',
        ...EXAMPLE,
      ),
    },
  ),
  change(
    'symbol renamed in code and expression',
    { 'docs/retry.md': PAGE },
    {
      'src/index.ts': SRC.replace('LIMIT', 'MAX_TRIES'),
      'src/extra.ts': 'export const OTHER = 3;\n',
      'docs/retry.md': page(
        FACT.replace('LIMIT', 'MAX_TRIES'),
        '',
        CLAIM,
        '',
        ...EXAMPLE.map((l) => l.replaceAll('LIMIT', 'MAX_TRIES')),
      ),
    },
  ),
  change(
    'expression renamed while the old name stays',
    { 'docs/retry.md': PAGE },
    {
      'src/index.ts': `${SRC}export const MAX_TRIES = 3;\n`,
      'docs/retry.md': page(FACT.replace('LIMIT', 'MAX_TRIES'), '', CLAIM, '', ...EXAMPLE),
    },
  ),
  change(
    'expression whitespace only',
    { 'docs/k.md': page('First key `A`. <!-- fact: KEYS[0] -->') },
    { 'docs/k.md': page('First key `A`. <!-- fact:   KEYS[ 0 ]   -->') },
  ),
  change(
    'expression quotes and shape',
    { 'docs/k.md': page('The joined keys are `A-B`. <!-- fact: KEYS.join("-") -->') },
    { 'docs/k.md': page("The joined keys are `A-B`. <!-- fact: KEYS.join('-') -->") },
  ),
  change(
    'surface changed',
    { 'docs/k.md': page('The limit is `3` tries. <!-- fact: LIMIT -->') },
    { 'docs/k.md': page('The limit is `3` tries. <!-- fact @extra: LIMIT -->') },
  ),
  change(
    'surface and expression changed',
    { 'docs/k.md': page('The limit is `3` tries. <!-- fact: LIMIT -->') },
    { 'docs/k.md': page('The limit is `3` tries. <!-- fact @extra: OTHER -->') },
  ),
  change(
    'order no longer checked',
    { 'docs/k.md': page(...ORDERED) },
    { 'docs/k.md': page(ORDERED[0].replace('fact ordered', 'fact'), ...ORDERED.slice(1)) },
  ),
  change(
    'order no longer checked, items not sorted',
    {
      'docs/k.md': page(
        'The keys, in order: <!-- fact ordered: KEYS.toReversed() -->',
        '- `B`',
        '- `A`',
      ),
    },
    { 'docs/k.md': page('The keys, in order: <!-- fact: KEYS.toReversed() -->', '- `B`', '- `A`') },
  ),
  change(
    'list reordered, expression edited',
    { 'docs/k.md': page(...LIST) },
    { 'docs/k.md': page('The keys are: <!-- fact: [...KEYS] -->', '- `B`', '- `A`') },
  ),
  change(
    'column fact expression edited',
    { 'docs/k.md': page(...COLUMN) },
    { 'docs/k.md': page(COLUMN[0].replace('KEYS', 'KEYS.slice(0, 2)'), ...COLUMN.slice(1)) },
  ),
  change(
    'block fact expression edited',
    { 'docs/k.md': page(...BLOCK) },
    { 'docs/k.md': page(...BLOCK.map((l) => l.replace('TEXT', 'TEXT.trim()'))) },
  ),
  change(
    'cell fact expression edited',
    { 'docs/t.md': page(...CELL('`3` <!-- fact: LIMIT -->')) },
    { 'docs/t.md': page(...CELL('`3` <!-- fact: LIMIT + 0 -->')) },
  ),
  change(
    'weakened fact that now fails too',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': page(FACT.replace('LIMIT', 'LIMIT + 1'), '', CLAIM, '', ...EXAMPLE) },
  ),
];
