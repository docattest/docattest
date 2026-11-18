import { describe, expect, it } from 'vitest';
import {
  jestArrayTitles,
  jestTemplateTitle,
} from '../../../../src/language/citations/jest-titles.js';

// The titles Jest's `test.each` prints for these rows, by jest-each's rules: placeholders take
// a row's values in turn through Node's `util.format`, and a table of objects whose title has
// no placeholder is read by `$name`, each value shown as `String` shows it.
describe('jestArrayTitles', () => {
  it('formats placeholders one value of a row at a time', () => {
    expect(jestArrayTitles('n %i', [1, 2])).toEqual(['n 1', 'n 2']);
    expect(jestArrayTitles('%s and %s', [['a', 'b']])).toEqual(['a and b']);
    expect(jestArrayTitles('few %s', [[1, 2, 3]])).toEqual(['few 1']);
    expect(jestArrayTitles('many %s %s', [[1]])).toEqual(['many 1 %s']);
    expect(jestArrayTitles('%# is %d, %f, %j', [[1.5, '2.25', 'x']])).toEqual([
      '0 is 1.5, 2.25, "x"',
    ]);
    expect(jestArrayTitles('100%% %s', [['%s']])).toEqual(['100% %s']);
    expect(jestArrayTitles('100%% done', [[1]])).toEqual(['100% done']);
    expect(jestArrayTitles('%s', [-0, 10n, null])).toEqual(['-0', '10n', 'null']);
  });

  it('reads a table of objects by $name when its title has no placeholder', () => {
    const rows = [{ name: 'one', n: 1, deep: { k: 'v' } }];
    expect(jestArrayTitles('$name is $n, $deep.k, row $#', rows)).toEqual(['one is 1, v, row 0']);
    expect(jestArrayTitles('$deep', rows)).toEqual([undefined]);
  });

  it('gives no title where Jest would pretty-print a value, or for what it is not sure of', () => {
    expect(jestArrayTitles('%s', [[{ a: 1 }]])).toEqual([undefined]);
    expect(jestArrayTitles('%p', [1])).toEqual([undefined]);
    expect(jestArrayTitles('%# %#', [1])).toEqual([undefined]);
  });
});

describe('jestTemplateTitle', () => {
  it('reads a template-literal row by $name', () => {
    expect(jestTemplateTitle('$a + $b', { a: 1, b: 'x' }, 0)).toBe('1 + x');
    expect(jestTemplateTitle('$a %', { a: 1 }, 0)).toBeUndefined();
  });
});
