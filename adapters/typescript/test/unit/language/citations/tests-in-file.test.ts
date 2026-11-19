import { describe, expect, it } from 'vitest';
import { findTests } from '../../../../src/language/citations/tests-in-file.js';

const titles = (code: string) =>
  findTests('/t.test.ts', code).tests.map((t) => [...t.describes, t.title].join(' > '));

const VITEST = "import { describe, it, test } from 'vitest';";
const JEST = "import { describe, it, test } from '@jest/globals';";

describe('the tests of a table the file writes', () => {
  it('titles each row of an each table as the runner prints it', () => {
    const code = [
      VITEST,
      "it.each([1, 2])('adds %i', () => {});",
      "it.each([[1, 'a'], [2, 'b']])('row %# is %s %s', () => {});",
      "test.each([{ name: 'one', n: 1 }])('$name has $n', () => {});",
      "test.for([[3, 4]])('for %i and %i', () => {});",
    ].join('\n');
    expect(titles(code)).toEqual([
      'adds %i',
      'adds 1',
      'adds 2',
      'row %# is %s %s',
      'row 0 is 1 a',
      'row 1 is 2 b',
      '$name has $n',
      "'one' has 1",
      'for %i and %i',
      'for 3 and 4',
    ]);
  });

  it('titles a $name as each runner shows it, and as both do when the file names neither', () => {
    const table = "it.each([{ name: 'one', n: 1 }])('$name $n', () => {});";
    expect(titles(`${VITEST}\n${table}`)).toEqual(['$name $n', "'one' 1"]);
    expect(titles(`${JEST}\n${table}`)).toEqual(['$name $n', 'one 1']);
    expect(titles(table)).toEqual(['$name $n']);
    expect(titles("it.each([{ n: 1 }])('$n', () => {});")).toEqual(['$n', '1']);
  });

  it('reads a template-literal table by its header', () => {
    const code = [
      JEST,
      'test.each`',
      '  a    | b    | sum',
      '  ${1} | ${2} | ${1 + 2}',
      "  ${'x'} | ${'y'} | ${'xy'}",
      "`('$a and $b make $sum', () => {});",
    ].join('\n');
    expect(titles(code)).toEqual(['$a and $b make $sum', '1 and 2 make 3', 'x and y make xy']);
  });

  it('expands describe.each and binds its parameters for the tests inside', () => {
    const code = [
      VITEST,
      "describe.each([['nl', 21], ['de', 19]])('in %s', (region, rate) => {",
      '  it(`taxes ${rate}%`, () => {});',
      '});',
    ].join('\n');
    expect(titles(code)).toEqual(['in nl > taxes 21%', 'in de > taxes 19%']);
  });

  it('titles the tests of a for-of loop and a forEach over a table the file writes', () => {
    const code = [
      'const cases = [',
      "  { name: 'rounds down', price: 1 },",
      "  { name: 'rounds up', price: 2 },",
      '] as const;',
      "describe('price', () => {",
      '  for (const c of cases) it(c.name, () => {});',
      '  cases.forEach(({ name }, k) => test(`${k}: ${name}`, () => {}));',
      "  for (const [key, value] of Object.entries({ a: 1, b: 2 })) it(key + '=' + value, () => {});",
      '});',
    ].join('\n');
    expect(titles(code)).toEqual([
      'price > rounds down',
      'price > rounds up',
      'price > 0: rounds down',
      'price > 1: rounds up',
      'price > a=1',
      'price > b=2',
    ]);
  });

  it('cannot cite what the file does not write as a literal, or changes', () => {
    const code = [
      "import { cases } from './cases.js';",
      'for (const c of cases) it(c.name, () => {});',
      "let later = [{ name: 'let' }];",
      'for (const c of later) it(c.name, () => {});',
      "const grown = [{ name: 'grown' }];",
      "grown.push({ name: 'more' });",
      'for (const c of grown) it(c.name, () => {});',
      "const made = [1, 2].map((n) => ({ name: 'n' + n }));",
      'for (const c of made) it(c.name, () => {});',
      'it.each(cases)("%s", () => {});',
      "it.each([{ a: [1] }])('$a', () => {});",
      "it.each([[{}]])('%s', () => {});",
    ].join('\n');
    expect(titles(code)).toEqual(['%s', '$a']);
  });

  it('declares a title that two rows give once, at the first', () => {
    const tests = findTests(
      '/t.test.ts',
      "for (const n of [1, 1, 2]) it('n' + n, () => {});",
    ).tests;
    expect(tests.map((t) => t.title)).toEqual(['n1', 'n2']);
  });

  it('lets an inner loop variable shadow an outer one', () => {
    const code = [
      'for (const c of [1, 2]) {',
      "  describe('c' + c, () => {",
      '    for (const c of [9]) it(`inner ${c}`, () => {});',
      '  });',
      '}',
    ].join('\n');
    expect(titles(code)).toEqual(['c1 > inner 9', 'c2 > inner 9']);
  });
});

describe('the paths a runner reports a test by', () => {
  const reported = (code: string) =>
    Object.fromEntries(
      findTests('/t.test.ts', code).tests.map((t) => [
        [...t.describes, t.title].join(' > '),
        t.reported?.map((path) => path.join(' > ')),
      ]),
    );

  it("are a test's own path, or each row's for a table's title as written, at every level", () => {
    const code = [
      VITEST,
      "describe.each(['a', 'b'])('group %s', () => {",
      "  it.each([1, 2])('adds %i', () => {});",
      "  for (const c of ['x']) it(c, () => {});",
      '});',
      "it('alone', () => {});",
    ].join('\n');
    expect(reported(code)).toMatchObject({
      'group %s > adds %i': [
        'group a > adds 1',
        'group a > adds 2',
        'group b > adds 1',
        'group b > adds 2',
      ],
      'group a > adds 1': ['group a > adds 1'],
      'group %s > x': ['group a > x', 'group b > x'],
      'group b > x': ['group b > x'],
      alone: ['alone'],
    });
  });

  it('cannot be told for a table with a row whose title cannot be', () => {
    const code = [VITEST, "it.each([[1], [{}]])('takes %s', () => {});"].join('\n');
    expect(reported(code)).toEqual({ 'takes %s': undefined, 'takes 1': ['takes 1'] });
  });
});
