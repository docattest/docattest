import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { literalMutants } from '../../../../src/language/syntax/literal-mutants.js';
import { parseSource } from '../../../../src/language/syntax/parse.js';

const PREFIX = 'const X = ';

/** Each mutant of `X`'s initializer: its description, and the initializer it leaves. */
function mutantsOf(initializer: string): [string, string][] {
  const code = `${PREFIX}${initializer};`;
  const source = parseSource('/m.ts', code);
  const [statement] = source.statements;
  if (!ts.isVariableStatement(statement)) throw new Error('not a variable');
  const value = statement.declarationList.declarations[0].initializer;
  if (!value) throw new Error('no initializer');
  return literalMutants(value, source, () => false).map((m) => {
    const edited = code.slice(0, m.start) + m.replacement + code.slice(m.end);
    return [m.description, edited.slice(PREFIX.length, -1)];
  });
}

describe('literalMutants', () => {
  it('adds one to a number, written as TypeScript reads a number', () => {
    expect(mutantsOf('8080')).toEqual([['8080 → 8081', '8081']]);
    expect(mutantsOf('1.5')).toEqual([['1.5 → 2.5', '2.5']]);
    expect(mutantsOf('1_000')).toEqual([['1_000 → 1001', '1001']]);
    expect(mutantsOf('0xff')).toEqual([['0xff → 256', '256']]);
    expect(mutantsOf('-5')).toEqual([['-5 → -4', '-4']]);
    expect(mutantsOf('10n')).toEqual([['10n → 11n', '11n']]);
    expect(mutantsOf('-1n')).toEqual([['-1n → 0n', '0n']]);
    expect(mutantsOf('2 ** 53')).toEqual([
      ['2 → 3', '3 ** 53'],
      ['53 → 54', '2 ** 54'],
    ]);
    expect(mutantsOf('9007199254740993')).toEqual([]);
  });

  it('puts an x before the closing quote of a string, keeping its quotes and escapes', () => {
    expect(mutantsOf("'shop'")).toEqual([["'shop' → 'shopx'", "'shopx'"]]);
    expect(mutantsOf('"it\'s"')).toEqual([['"it\'s" → "it\'sx"', '"it\'sx"']]);
    expect(mutantsOf("'a\\'b\\n'")).toEqual([["'a\\'b\\n' → 'a\\'b\\nx'", "'a\\'b\\nx'"]]);
    expect(mutantsOf('`plain`')).toEqual([['`plain` → `plainx`', '`plainx`']]);
    expect(mutantsOf('`port ${8080}`')).toEqual([['8080 → 8081', '`port ${8081}`']]);
  });

  it('flips a boolean and leaves null and undefined alone', () => {
    expect(mutantsOf('true')).toEqual([['true → false', 'false']]);
    expect(mutantsOf('false')).toEqual([['false → true', 'true']]);
    expect(mutantsOf('[null, undefined]')).toEqual([['undefined removed', '[null]']]);
  });

  it("removes an array's last item before changing its items", () => {
    expect(mutantsOf("['A', 'LOG_LEVEL']")).toEqual([
      ["'LOG_LEVEL' removed", "['A']"],
      ["'A' → 'Ax'", "['Ax', 'LOG_LEVEL']"],
      ["'LOG_LEVEL' → 'LOG_LEVELx'", "['A', 'LOG_LEVELx']"],
    ]);
    expect(mutantsOf("[ 'only' ]")).toEqual([
      ["'only' removed", '[]'],
      ["'only' → 'onlyx'", "[ 'onlyx' ]"],
    ]);
    expect(mutantsOf('[1, 2,]')[0]).toEqual(['2 removed', '[1,]']);
    expect(mutantsOf('[1, { a:\n  2 }]')[0]).toEqual(['{ a: 2 } removed', '[1]']);
    expect(mutantsOf('[]')).toEqual([]);
  });

  it('changes literals at any depth, string keys among them, in source order', () => {
    expect(mutantsOf("{ port: 80, 'log-level': 'info', nested: { on: [true] } }")).toEqual([
      ['80 → 81', "{ port: 81, 'log-level': 'info', nested: { on: [true] } }"],
      ["'log-level' → 'log-levelx'", "{ port: 80, 'log-levelx': 'info', nested: { on: [true] } }"],
      ["'info' → 'infox'", "{ port: 80, 'log-level': 'infox', nested: { on: [true] } }"],
      ['true removed', "{ port: 80, 'log-level': 'info', nested: { on: [] } }"],
      ['true → false', "{ port: 80, 'log-level': 'info', nested: { on: [false] } }"],
    ]);
    expect(mutantsOf("new Map([['a', 1]])").map(([d]) => d)).toEqual([
      "['a', 1] removed",
      '1 removed',
      "'a' → 'ax'",
      '1 → 2',
    ]);
  });

  it('changes nothing inside a function, a class or a type the value holds', () => {
    expect(mutantsOf("{ f: () => 1, g() { return 'a'; }, C: class { x = 2 } }")).toEqual([]);
    expect(mutantsOf("load<'a'>(3) as 'b' | 4")).toEqual([['3 → 4', "load<'a'>(4) as 'b' | 4"]]);
    expect(mutantsOf('compute()')).toEqual([]);
  });
});
