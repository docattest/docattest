import { describe, expect, it } from 'vitest';
import { citedSymbols, listTests } from '../../../../src/language/citations/listing.js';
import { findTests } from '../../../../src/language/citations/tests-in-file.js';
import { memoryFiles } from '../../../../src/files/memory.js';

const titles = (code: string) =>
  findTests('/t.test.ts', code).tests.map((t) => [...t.describes, t.title].join(' > '));

describe('findTests', () => {
  it('collects it and test calls with their enclosing describe titles', () => {
    const code = [
      "describe('outer', () => {",
      "  it('one', () => {});",
      "  describe('inner', () => { test('two', () => {}); });",
      '});',
      "test('three', () => {});",
    ].join('\n');
    expect(titles(code)).toEqual(['outer > one', 'outer > inner > two', 'three']);
    expect(findTests('/t.test.ts', code).suites.map((s) => s.title)).toEqual(['outer', 'inner']);
    expect(findTests('/t.test.ts', code).tests.map((t) => t.line)).toEqual([2, 3, 5]);
  });

  it('accepts only, skip, concurrent and each forms with a literal title', () => {
    const code = [
      "it.only('a', () => {});",
      "test.skip('b', () => {});",
      'test.concurrent(`c`, async () => {});',
      "it.each([1, 2])('d %i', () => {});",
      "test.each`x\n${1}`('e', () => {});",
      "it.skipIf(true)('f', () => {});",
      "describe.each([1])('g', () => { it('h', () => {}); });",
      "test.describe('pw', () => { test('i', () => {}); });",
    ].join('\n');
    expect(titles(code)).toEqual(['a', 'b', 'c', 'd %i', 'e', 'f', 'g > h', 'pw > i']);
  });

  it('ignores titles in comments, strings, computed titles, todos and other calls', () => {
    const code = [
      "// it('in a comment', () => {});",
      'const s = "test(\'in a string\', () => {})";',
      'it(name, () => {}); it(`t${1}`, () => {});',
      "it.todo('later');",
      "t.test('subtest', () => {}); other('x', () => {}); it.each('x');",
    ].join('\n');
    expect(titles(code)).toEqual([]);
  });

  it('marks a computed describe title as null', () => {
    const [found] = findTests('/t.test.ts', "describe(Name, () => { it('a', () => {}); });").tests;
    expect(found.describes).toEqual([null]);
  });
});

describe('listTests', () => {
  const files = memoryFiles({ 'test/a.test.ts': "it('a', () => {});", 'test/a.py': '' });

  it('lists a file, and says why when it cannot', () => {
    expect(listTests(files, 'test/a.test.ts')).toEqual({
      ok: true,
      tests: [{ title: 'a', describes: [], line: 1 }],
      suites: [],
    });
    expect(listTests(files, 'test/b.test.ts')).toEqual({
      ok: false,
      message: 'test/b.test.ts does not exist',
    });
    expect(listTests(files, 'test/a.py')).toMatchObject({ ok: false });
  });
});

describe('citedSymbols', () => {
  const surfaceOf = (specifier: string) => (specifier === 'kit' ? 'default' : undefined);
  const code = [
    "import { retry, wait as pause, unused } from 'kit';",
    "import * as kit from 'kit';",
    "import { helper } from './helper.js';",
    "describe('retry', () => {",
    "  it('throws', () => { retry(); kit.limit; helper(); });",
    "  it('waits', () => { pause(); });",
    '});',
  ].join('\n');

  it('takes the surface symbols the cited test body uses', () => {
    expect(citedSymbols('/t.test.ts', code, { title: 'throws', describes: [] }, surfaceOf)).toEqual(
      [
        { name: 'retry', surface: 'default' },
        { name: 'limit', surface: 'default' },
      ],
    );
    expect(
      citedSymbols('/t.test.ts', code, { title: 'waits', describes: ['retry'] }, surfaceOf),
    ).toEqual([{ name: 'wait', surface: 'default' }]);
    expect(citedSymbols('/t.test.ts', code, { title: 'gone', describes: [] }, surfaceOf)).toEqual(
      [],
    );
  });
});
