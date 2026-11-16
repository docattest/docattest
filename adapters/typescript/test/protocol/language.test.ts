// The questions about text, which need no project and no compiler: PROTOCOL.md §3,
// "Language level".
import { describe, expect, it } from 'vitest';
import { byId, exchange, HELLO, open } from './exchange.ts';

describe('the adapter server, asked about text', () => {
  it('answers a warm greeting the same, and loads its compiler only when a request needs it', () => {
    const warm = { id: 0, method: 'hello', params: { protocol: 1, warm: true } };
    const greeted = exchange([warm]);
    expect(byId(greeted, 0)?.result).toEqual(byId(exchange([HELLO]), 0)?.result);
    expect(greeted.code).toBe(0);
    // The questions about text are answered without the compiler, before a request that needs it.
    const files = [{ page: 'src/a.ts', text: '/** One. */\nexport const a = 1;' }];
    const x = exchange([
      HELLO,
      { id: 1, method: 'docComments', params: { files } },
      { id: 2, method: 'expressions', params: { expressions: ['a'] } },
      { id: 3, method: 'open', params: open(3, { 'src/index.ts': 'export const N = 3;' }).params },
      { id: 4, method: 'exports', params: { surface: 'default' } },
    ]);
    expect(byId(x, 1)?.result).toEqual([]);
    expect(byId(x, 2)?.result).toEqual([{ shape: '·', names: ['a'] }]);
    expect(byId(x, 4)?.result).toEqual(['N']);
    expect(x.code).toBe(0);
  });

  it('says which examples assert, and reads doc comments as prose', () => {
    const examples = [
      { lang: 'ts', code: 'assert.equal(1, 1);' },
      { lang: 'ts', code: 'console.log(1);' },
      { lang: 'js', code: 'assert (x)' },
      { lang: 'ts', code: 'reassert.ok(1); // we assert nothing' },
    ];
    const text =
      '// x\n/**\n * Doubles a number.\n *   Twice.\n * @example\n * // check\n */\nexport const d = 1;\n/** One line. */\n/** @internal */';
    const files = [
      { page: 'src/a.ts', text },
      { page: 'docs/a.md', text },
      { page: 'src/empty.ts', text: '' },
    ];
    const x = exchange([
      HELLO,
      { id: 1, method: 'assertions', params: { examples } },
      { id: 2, method: 'docProse', params: { files } },
      { id: 3, method: 'docProse', params: { files: [{ page: 'a.ts' }] } },
    ]);
    expect(byId(x, 1)?.result).toEqual([true, false, true, false]);
    expect(byId(x, 2)?.result).toEqual([
      [
        { text: 'Doubles a number. Twice.', start: 2, end: 7 },
        { text: 'One line.', start: 9, end: 9 },
        { text: '', start: 10, end: 10 },
      ],
      [],
      [],
    ]);
    expect(byId(x, 3)?.error?.kind).toBe('internal');
  });

  it('says which symbol a code span names, and what shape an expression has', () => {
    const spans = [
      'retry',
      ' retry(fn) ',
      'new Client()',
      'await  load(x)',
      'RETRY.max',
      'retry-after',
      'retry.ts',
      'retry.tsy',
      'parse',
      'new Client-x',
      '',
    ];
    const symbols = ['retry', 'Client', 'load', 'RETRY', 'new'];
    const expressions = ['Math.min(LIMIT, 3)', "f('a b', x)", 'Math.min(MAX, 3)', ''];
    const x = exchange([
      HELLO,
      { id: 1, method: 'mentions', params: { spans, symbols } },
      { id: 2, method: 'expressions', params: { expressions } },
    ]);
    const named = byId(x, 1)?.result as (string | null)[];
    expect(named).toEqual([
      'retry',
      'retry',
      'Client',
      'load',
      'RETRY',
      null,
      null,
      'retry',
      null,
      'new',
      null,
    ]);
    // A name is always part of the span that names it.
    named.forEach((name, k) => {
      expect(name === null || spans[k]?.includes(name)).toBe(true);
    });
    const [a, b, c, d] = byId(x, 2)?.result as { shape: string; names: string[] }[];
    expect(a).toEqual({ shape: '· . · ( · , 3 )', names: ['Math', 'min', 'LIMIT'] });
    expect(b).toEqual({ shape: "· ( 'a b' , · )", names: ['f', 'x'] });
    expect(c.shape).toBe(a.shape);
    expect(d).toEqual({ shape: '', names: [] });
  });
});
