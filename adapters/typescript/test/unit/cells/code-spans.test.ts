import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { createProject } from 'docattest';
import { codeSpans } from '../../../src/cells/code-spans.js';

/** The table every language's cell helper and the core are tested against. */
const SHARED_CASES = new URL('../../../../../protocol/test-data/code-spans.json', import.meta.url);

interface SharedCase {
  name: string;
  cell: string;
  spans: string[];
}

function sharedCases(): SharedCase[] {
  const table = JSON.parse(readFileSync(SHARED_CASES, 'utf8')) as { cases: SharedCase[] };
  return table.cases;
}

describe('codeSpans', () => {
  it.each(sharedCases())('$name', ({ cell, spans }) => {
    expect(codeSpans(cell)).toEqual(spans);
  });
});

// Cells as a page writes them, each with at least one code span: a cell with none is
// compared by its text, which codeSpans does not give.
const CELLS_AS_WRITTEN = [
  '`FOO=1` `BAR=2`',
  '`fmt.Println(1)`',
  '``a`b``',
  '`` `x` ``',
  '```a``b```',
  '`  padded  `',
  '`  `x',
  'text `a` more `b` end',
  '`a\\|b`',
  '\\`x` `y`',
  '\\\\`x`',
  '`x\\`y`',
  '``a` `b`',
  '`a`` `b`',
  '\t`\u00a0tab and no-break\u00a0` ',
  '`\ufeffbyte order mark\u0085`',
  '`a` <!-- `b` --> `c`',
  '<span title="`">`x`</span>',
  '<https://example.com/`a`> `b`',
  '<a@b.c> `d`',
  'a < b `c` > d',
  '**`bold`** _`em`_',
  '[`a`](b) `c`',
  '[a](`b`) `c`',
  '[a](u "`t`") `c`',
  "[a](u '`t`') `c`",
  '[a](u (`t`)) `c`',
  '[a](<`b` c>) `d`',
  '[a](b(`c`)) `d`',
  '[a] (`b`)',
  'a](`b`)',
  '\\[a](`b`)',
  '[[a](b)](`c`) `z`',
  '![[a](b)](`c`) `z`',
  '[![a](b)](`c`) `z`',
  '[a [b] c](`d`) `z`',
  '[a][`b`]',
  '[a`](b)` `c`',
  '[a](b `c`) `d`',
  '<!-->`a`',
  '<?x `a` ?> `b`',
  '<!X `a`> `b`',
  '<![CDATA[`a`]]> `b`',
  '</a `b`> `c`',
];

// A fact's expression sits in an HTML comment, which a `>` in a string could close.
const literal = (spans: string[]) =>
  JSON.stringify(spans).replace(/[<>]/g, (c) => `\\u00${c.charCodeAt(0).toString(16)}`);

describe('codeSpans and the core', () => {
  // Each cell is the one row of a table whose column fact lists what codeSpans gives for the
  // cell as a runner gets it, so a fact that fails is a cell the two read differently.
  it('gives what a column fact compares', async () => {
    const spans = CELLS_AS_WRITTEN.map((cell) => codeSpans(cell.replaceAll('\\|', '|')));
    const page = CELLS_AS_WRITTEN.map(
      (cell, k) =>
        `<!-- fact column=cell: ${literal(spans[k] ?? [])} -->\n| cell |\n|---|\n| ${cell} |\n`,
    ).join('\n');
    const report = await createProject({
      files: {
        'docattest.config.json': JSON.stringify({
          pages: ['cells.md'],
          surfaces: { default: './src/index.ts' },
          adapter: 'typescript',
        }),
        'src/index.ts': 'export const unused = 1;',
        'cells.md': page,
      },
    }).check();
    const failed = report.results
      .filter((r) => r.status !== 'pass')
      .map((r) => `L${r.line}: ${r.message}`);
    expect(failed).toEqual([]);
    expect(report.results).toHaveLength(CELLS_AS_WRITTEN.length);
  });
});
