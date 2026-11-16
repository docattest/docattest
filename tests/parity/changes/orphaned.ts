// Orphaned claims, by attestation kind.
import {
  ASTRAL,
  BARE,
  BLOCK,
  CASES,
  CELL,
  change,
  CITED,
  CLAIM,
  COLUMN,
  doc,
  DOC_EXAMPLE,
  EXAMPLE,
  FACT,
  HEADERS,
  LIST,
  LONG,
  page,
  PAGE,
  retry,
  type ChangeCase,
} from './fixtures.ts';

export const orphaned: ChangeCase[] = [
  // ----- orphaned claims, by attestation kind -----
  retry('scalar fact deleted', page('Intro.', '', BARE, '', CLAIM, '', ...EXAMPLE)),
  retry('example deleted', page(FACT, '', CLAIM)),
  retry(
    'example untagged under its claim',
    page(FACT, '', CLAIM, '', '```ts', ...EXAMPLE.slice(1)),
  ),
  retry('fact and example both deleted', page(BARE, '', CLAIM)),
  change(
    'list fact deleted',
    { 'docs/k.md': page(...LIST) },
    { 'docs/k.md': page('The keys are:', '- `A`', '- `B`') },
  ),
  change(
    'column fact deleted',
    { 'docs/k.md': page(...COLUMN) },
    { 'docs/k.md': page('The keys by name.', '', ...COLUMN.slice(2)) },
  ),
  change(
    'block fact deleted',
    { 'docs/k.md': page(...BLOCK) },
    { 'docs/k.md': page('Intro.', '', 'The text is exactly this.', '', ...BLOCK.slice(3)) },
  ),
  change(
    'table cell fact deleted',
    { 'docs/t.md': page(...CELL('`3` <!-- fact: LIMIT -->')) },
    { 'docs/t.md': page(...CELL('`3`')) },
  ),
  change(
    'cases directive deleted',
    {
      'docs/c.md': page('Adding small and large numbers works. <!-- cases: adds -->', '', ...CASES),
    },
    { 'docs/c.md': page('Adding small and large numbers works.', '', ...CASES) },
  ),
  change(
    'cases directive above its table deleted',
    {
      'docs/c.md': page(
        'Adding small and large numbers works.',
        '',
        '<!-- cases: adds -->',
        ...CASES,
      ),
    },
    { 'docs/c.md': page('Adding small and large numbers works.', '', ...CASES) },
  ),
  change(
    'cases row edited',
    {
      'docs/c.md': page('Adding small and large numbers works. <!-- cases: adds -->', '', ...CASES),
    },
    {
      'docs/c.md': page(
        'Adding small and large numbers works. <!-- cases: adds -->',
        '',
        ...CASES.slice(0, 3),
      ),
    },
  ),
  change(
    'test citation deleted',
    { 'docs/a.md': page(CITED) },
    { 'docs/a.md': page('Adding two numbers gives their sum.') },
  ),
  change(
    'test citation retitled',
    { 'docs/a.md': page(CITED) },
    { 'docs/a.md': page(CITED.replace('"adds"', '"sums"')) },
  ),
  change(
    'doc-comment example deleted',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'src/double.ts': doc('') },
  ),
  change(
    'doc-comment example deleted, description reworded lightly',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'src/double.ts': doc('', 'Doubles a number, whatever the sign.') },
  ),
  change(
    'doc-comment example deleted with its description',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'src/double.ts': 'export const double = (n: number) => n * 2;\n' },
  ),
  change(
    'doc-comment source deleted',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'src/double.ts': null },
  ),
  change(
    'doc-comment example moved down the file',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'src/double.ts': `// a new header\n\nexport const one = 1;\n\n${doc(DOC_EXAMPLE)}` },
  ),
  change(
    'doc-comment source renamed',
    { 'src/double.ts': doc(DOC_EXAMPLE) },
    { 'src/double.ts': null, 'src/twice.ts': doc(DOC_EXAMPLE).replace('./double', './twice') },
  ),
  change(
    'doc-comment source renamed, example deleted',
    { 'src/double.ts': doc(DOC_EXAMPLE.replace(' * import { double } from "./double";\n', '')) },
    { 'src/double.ts': null, 'src/twice.ts': doc('') },
  ),
  change(
    'page renamed and a proof lost',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': null, 'docs/retrying.md': page(BARE, '', CLAIM, '', ...EXAMPLE) },
  ),
  change(
    'an unproven copy was already there',
    { 'docs/retry.md': page(FACT, '', BARE, '', CLAIM, '', ...EXAMPLE) },
    { 'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE) },
  ),
  change(
    'an unproven copy was already there, and both stay',
    { 'docs/retry.md': page(FACT, '', BARE) },
    { 'docs/retry.md': page(BARE, '', BARE) },
  ),
  change(
    'two facts in one paragraph, one deleted',
    {
      'docs/p.md': page(
        'At most `3` tries <!-- fact: LIMIT --> and',
        'the first key is `A`. <!-- fact: KEYS[0] -->',
      ),
    },
    { 'docs/p.md': page('At most `3` tries <!-- fact: LIMIT --> and', 'the first key is `A`.') },
  ),
  change(
    'two facts in one paragraph, both deleted',
    {
      'docs/p.md': page(
        'At most `3` tries <!-- fact: LIMIT --> and',
        'the first key is `A`. <!-- fact: KEYS[0] -->',
      ),
    },
    { 'docs/p.md': page('At most `3` tries and', 'the first key is `A`.') },
  ),
  change(
    'two facts on one line, both deleted',
    {
      'docs/p.md': page(
        'At most `3` tries <!-- fact: LIMIT --> and the first key is `A`. <!-- fact: KEYS[0] -->',
      ),
    },
    { 'docs/p.md': page('Intro.', '', 'At most `3` tries and the first key is `A`.') },
  ),
  change(
    'the same example twice, one deleted',
    {
      'docs/d.md': page(
        'First it holds here.',
        '',
        ...EXAMPLE,
        '',
        'Then it holds there.',
        '',
        ...EXAMPLE,
      ),
    },
    { 'docs/d.md': page('First it holds here.', '', 'Then it holds there.', '', ...EXAMPLE) },
  ),
  change(
    'the same fact twice, the first deleted',
    { 'docs/d.md': page(FACT, '', 'Some prose in between.', '', FACT) },
    { 'docs/d.md': page('Some prose in between.', '', FACT) },
  ),
  change(
    'a long claim is quoted short',
    { 'docs/q.md': page(`${LONG} <!-- fact: LIMIT -->`) },
    { 'docs/q.md': page(LONG) },
  ),
  change(
    'a quotation cut just after an astral character',
    { 'docs/q.md': page(`${ASTRAL(50)} <!-- fact: LIMIT -->`) },
    { 'docs/q.md': page(ASTRAL(50)) },
  ),
  change(
    'a quotation cut inside an astral character',
    { 'docs/q.md': page(`${ASTRAL(51)} <!-- fact: LIMIT -->`), 'docs/r.md': page(FACT) },
    { 'docs/q.md': page(ASTRAL(51)), 'docs/r.md': page(BARE) },
  ),
  change(
    'a quotation cut just before an astral character',
    { 'docs/q.md': page(`${ASTRAL(52)} <!-- fact: LIMIT -->`) },
    { 'docs/q.md': page(ASTRAL(52)) },
  ),
  change(
    'a claim of exactly sixty units',
    { 'docs/q.md': page(`${'y'.repeat(48)} is \`3\` here. <!-- fact: LIMIT -->`) },
    { 'docs/q.md': page(`${'y'.repeat(48)} is \`3\` here.`) },
  ),
  change(
    'words with marks that are alphabetic but not letters',
    { 'docs/h.md': page('यह सीमा कुल `3` बार की ही होती है। <!-- fact: LIMIT -->') },
    { 'docs/h.md': page('यह सीमा कुल `3` बार का ही होती है।') },
  ),
  change(
    'words with marks, reworded more',
    { 'docs/h.md': page('की की की `3` कि। <!-- fact: LIMIT -->') },
    { 'docs/h.md': page('का का की `3` कि।') },
  ),
  change(
    'lower-casing: final sigma and dotted I',
    { 'docs/g.md': page('ΟΔΟΣ İSTANBUL LIMIT IS `3` HERE. <!-- fact: LIMIT -->') },
    { 'docs/g.md': page('οδος i̇stanbul limit is `3` here.') },
  ),
  change(
    'no-break spaces between words',
    { 'docs/n.md': page('The\u00a0limit\u00a0is\u00a0`3`\u00a0tries. <!-- fact: LIMIT -->') },
    { 'docs/n.md': page('The limit is `3` tries.') },
  ),
  change(
    'numbers are literals',
    { 'docs/n.md': page('It waits 1.5 seconds at most `3` times. <!-- fact: LIMIT -->') },
    {
      'docs/n.md': page(
        'It waits 2.5 seconds at most `3` times.',
        '',
        'It waits 1.5 seconds, at most `3` times!',
      ),
    },
  ),
  change(
    'page with a byte order mark',
    { 'docs/b.md': `\ufeff${PAGE}` },
    { 'docs/b.md': `\ufeff${page(BARE, '', CLAIM, '', ...EXAMPLE)}` },
  ),
  change(
    'invalid UTF-8 in a page',
    {
      'docs/b.md': Buffer.concat([
        Buffer.from('# B\n\nThe limit \xff\xfe is `3` tries. <!-- fact: LIMIT -->\n', 'latin1'),
      ]),
    },
    { 'docs/b.md': Buffer.from('# B\n\nIntro.\n\nThe limit \xff\xfe is `3` tries.\n', 'latin1') },
  ),
  change(
    'doc comment in a CRLF source',
    { 'src/double.ts': doc(DOC_EXAMPLE).replaceAll('\n', '\r\n') },
    { 'src/double.ts': `// moved\r\n${doc('').replaceAll('\n', '\r\n')}` },
  ),
  change(
    'cases with headers named like fields',
    {
      'docs/c.md': page(
        'Adding small and large numbers works. <!-- cases: adds -->',
        '',
        ...HEADERS,
      ),
    },
    {
      'docs/c.md': page(
        'New prose.',
        '',
        'Adding small and large numbers works. <!-- cases: adds -->',
        '',
        HEADERS[0],
        HEADERS[1],
        '| small | z | w | 1 | 2 | 3 |',
      ),
    },
  ),
  change(
    'cases with headers named like fields, a counted cell edited',
    {
      'docs/c.md': page(
        'Adding small and large numbers works. <!-- cases: adds -->',
        '',
        ...HEADERS,
      ),
    },
    {
      'docs/c.md': page(
        'Adding small and large numbers works. <!-- cases: adds -->',
        '',
        HEADERS[0],
        HEADERS[1],
        '| small | x | y | 1 | 2 | 4 |',
      ),
    },
  ),
  change(
    'many pages at the base',
    Object.fromEntries([
      ['docs/retry.md', PAGE],
      ...Array.from({ length: 2500 }, (_, k): [string, string] => [
        `docs/many/p${k}.md`,
        `# Page ${k}\n\nNothing attested here.\n`,
      ]),
    ]),
    { 'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE) },
    [['check', '--base', 'HEAD', '--json']],
  ),
  change(
    'non-ASCII claim reworded lightly',
    {
      'docs/ü.md': page(
        'Größe und Zähler bleiben höchstens `3` Versuche lang gültig. <!-- fact: LIMIT -->',
      ),
    },
    { 'docs/ü.md': page('Größe und Zähler bleiben maximal `3` Versuche lang gültig.') },
  ),
  change(
    'a short claim is not substantive',
    {
      'docs/s.md': page(
        'Limit `3`. <!-- fact: LIMIT -->',
        '',
        'Tries: <!-- fact: LIMIT -->',
        '',
        '`3`',
      ),
    },
    { 'docs/s.md': page('Limit `3`.', '', 'Tries:', '', '`3`') },
  ),
  change(
    'fact under a heading only',
    { 'docs/h.md': '# The limit\n\n<!-- fact: KEYS -->\n- `A`\n- `B`\n' },
    { 'docs/h.md': '# The limit\n\n- `A`\n- `B`\n' },
  ),
  change(
    'crlf page loses its fact',
    { 'docs/r.md': PAGE.replaceAll('\n', '\r\n') },
    { 'docs/r.md': page(BARE, '', CLAIM, '', ...EXAMPLE).replaceAll('\n', '\r\n') },
  ),
  change(
    'an attestation with an error is deleted',
    {
      'docs/e.md': page(
        'The limit is `3` here. <!-- fact LIMIT -->',
        '',
        'Unclosed `3` here. <!-- fact: LIMIT',
      ),
    },
    { 'docs/e.md': page('The limit is `3` here.', '', 'Unclosed `3` here.') },
  ),
  change(
    'claim in a list item',
    {
      'docs/i.md': page(
        '- The limit is `3` tries. <!-- fact: LIMIT -->',
        '- Another item entirely.',
      ),
    },
    { 'docs/i.md': page('- Another item entirely.', '- The limit is `3` tries.') },
  ),
];
