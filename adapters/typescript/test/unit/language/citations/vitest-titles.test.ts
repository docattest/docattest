import { describe, expect, test } from 'vitest';
import { vitestTitle } from '../../../../src/language/citations/vitest-titles.js';

// Titles and the rows they are given. Each table is declared below with Vitest's own
// `test.for`, which titles its rows as `test.each` does (but for a row of `null` or
// `undefined`, which gives it no values), so a row's test is named with the title Vitest
// gives it: `vitestTitle` must give the same.
const TABLES: [string, unknown[]][] = [
  ['n %i', [1, 2]],
  [
    'pair %i %s',
    [
      [1, 'a'],
      [2, 'b'],
    ],
  ],
  ['$name $n $ok', [{ name: 'foo', n: 1, ok: true }]],
  ['s %s idx %# one %$ pct %%', ['x', 'y']],
  ['few %s', [[1, 2, 3]]],
  ['many %s %s', [[1]]],
  ['esc %s %s', [['%s', 'z']]],
  ['%% %s', [[1, 2, 3]]],
  ['100% %s', [[1, 2, 3]]],
  ['$0 $1 $x', [['a'], ['b']]],
  ['%s $0', [1]],
  ['$a', [{ a: '$b', b: 'q' }]],
  ['Says $name. and $name!', [{ name: 'x' }]],
  [
    '$n $l $big $f $nul $u $missing $s.length',
    [
      {
        n: 0,
        l: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        big: 10n,
        f: 1.5,
        nul: null,
        u: undefined,
        s: 'abc',
      },
    ],
  ],
  ['v %s %d %i %f %j', [[1.5, '2', 3.7, '4.25', 'j']]],
  ['v %s', [NaN, 1e21, 10n, null, undefined, true]],
  ['$a.b and $c.0', [{ a: { b: 'deep' }, c: [7] }]],
];

describe.each(TABLES)('rows titled %s', (template, rows) => {
  test.for(rows)(template, (row, { task }) => {
    const items = row === null || row === undefined ? [] : Array.isArray(row) ? row : [row];
    expect(vitestTitle(template, items, rows.indexOf(row))).toBe(task.name);
  });
});

describe('vitestTitle', () => {
  test('gives no title where Vitest would show an object, an escaped or cut string, or -0', () => {
    expect(vitestTitle('%s', [{ a: 1 }], 0)).toBeUndefined();
    expect(vitestTitle('$a', [{ a: [1] }], 0)).toBeUndefined();
    expect(vitestTitle('$s', [{ s: "it's" }], 0)).toBeUndefined();
    expect(vitestTitle('$s', [{ s: 'b'.repeat(39) }], 0)).toBeUndefined();
    expect(vitestTitle('%d', [-0], 0)).toBeUndefined();
    expect(vitestTitle('%o', [1], 0)).toBeUndefined();
    expect(vitestTitle('$c[0]', [{ c: [7] }], 0)).toBeUndefined();
  });
});
