import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { foldConstant } from '../../../../src/language/syntax/constant-value.js';
import { parseSource } from '../../../../src/language/syntax/parse.js';

/** The value of an expression whose names are `names`; undefined when it is no constant. */
function fold(expression: string, names: Record<string, unknown> = {}) {
  const [statement] = parseSource('/c.ts', `(${expression});`).statements;
  if (!ts.isExpressionStatement(statement)) throw new Error('not an expression');
  const resolve = (name: ts.Identifier) =>
    Object.hasOwn(names, name.text) ? { value: names[name.text] } : undefined;
  return foldConstant(statement.expression, resolve);
}

describe('foldConstant', () => {
  it('folds literals, arrays, objects and templates', () => {
    expect(fold("'a'")?.value).toBe('a');
    expect(fold('1_000')?.value).toBe(1000);
    expect(fold('0x10')?.value).toBe(16);
    expect(fold('10n')?.value).toBe(10n);
    expect(fold('[true, null, undefined]')?.value).toEqual([true, null, undefined]);
    expect(fold("{ a: 1, 'b c': [2], [`k${1}`]: 3, 4: 'four' }")?.value).toEqual({
      a: 1,
      'b c': [2],
      k1: 3,
      4: 'four',
    });
    expect(fold('`${N} items`', { N: 3 })?.value).toBe('3 items');
  });

  it('applies operators, and reads names, members and spreads of folded values', () => {
    expect(fold('30 * SECOND', { SECOND: 1000 })?.value).toBe(30000);
    expect(fold("'v' + 1 + 2")?.value).toBe('v12');
    expect(fold('1 << 10 | 1')?.value).toBe(1025);
    expect(fold('(-x) ** 2 === 4 ? "yes" : "no"', { x: 2 })?.value).toBe('yes');
    expect(fold('A ?? B', { A: null, B: 'b' })?.value).toBe('b');
    expect(fold('LIMITS.max', { LIMITS: { max: 5 } })?.value).toBe(5);
    expect(fold('LIST[1].toUpperCase', { LIST: ['a', 'b'] })).toBeUndefined();
    expect(fold('LIST.length', { LIST: ['a', 'b'] })?.value).toBe(2);
    expect(fold('[...A, ...B]', { A: [1], B: 'xy' })?.value).toEqual([1, 'x', 'y']);
    expect(fold('{ ...A, b: 2 }', { A: { a: 1 } })?.value).toEqual({ a: 1, b: 2 });
    expect(fold("(['a'] as const) satisfies readonly string[]")?.value).toEqual(['a']);
  });

  it('folds nothing that would run code, or that would throw', () => {
    expect(fold('f()')).toBeUndefined();
    expect(fold('new Date()')).toBeUndefined();
    expect(fold('() => 1')).toBeUndefined();
    expect(fold('unknown + 1')).toBeUndefined();
    expect(fold('1n + 1')).toBeUndefined();
    expect(fold('{ get a() { return 1; } }')).toBeUndefined();
    expect(fold('{ __proto__: null }')).toBeUndefined();
    expect(fold('/x/')).toBeUndefined();
  });
});
