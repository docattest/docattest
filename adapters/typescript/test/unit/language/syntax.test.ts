import { describe, expect, it } from 'vitest';
import { docCommentExamples } from '../../../src/language/syntax/doc-comments.js';
import {
  expressionIdentifiers,
  expressionProblem,
} from '../../../src/language/syntax/expression.js';
import { helperSymbols, objectKeys } from '../../../src/language/syntax/setup-module.js';
import { importedSymbols } from '../../../src/language/syntax/imports.js';

const surfaceOf = (specifier: string) =>
  specifier === 'kit' ? 'default' : specifier === 'srv' ? 'server' : undefined;

describe('importedSymbols', () => {
  it('collects named, default and namespace members, by surface', () => {
    const code =
      "import d, { a, b as c } from 'kit';\nimport * as s from 'srv';\nimport x from 'other';\ns.go(); s.go();";
    expect(importedSymbols('/x.ts', code, surfaceOf)).toEqual([
      { name: 'default', surface: 'default' },
      { name: 'a', surface: 'default' },
      { name: 'b', surface: 'default' },
      { name: 'go', surface: 'server' },
    ]);
  });
});

describe('fact expressions', () => {
  it('lists free identifiers only', () => {
    expect(expressionIdentifiers('ROUTES.map((r) => r.path).concat(extra({ key: v }))')).toEqual([
      'ROUTES',
      'extra',
      'v',
    ]);
  });

  it('rejects anything but one expression', () => {
    expect(expressionProblem('N.x')).toBeUndefined();
    expect(expressionProblem('N; M')).toMatch(/cannot parse|single expression/);
    expect(expressionProblem('(')).toMatch(/cannot parse fact expression/);
  });
});

describe('setup helpers', () => {
  const setup = [
    "import { ERRORS, other } from 'kit';",
    "import { serve } from 'srv';",
    'const pick = () => ERRORS.list;',
    'export const facts = { errorFields: () => pick().map(String), unrelated: () => other } as const;',
    'export const cases = { run(row: unknown) { serve(row); } };',
  ].join('\n');

  it('follows a helper into the top-level helpers it calls', () => {
    expect(helperSymbols('/s.ts', setup, 'facts', 'errorFields', surfaceOf)).toEqual([
      { name: 'ERRORS', surface: 'default' },
    ]);
    expect(helperSymbols('/s.ts', setup, 'cases', 'run', surfaceOf)).toEqual([
      { name: 'serve', surface: 'server' },
    ]);
    expect(helperSymbols('/s.ts', setup, 'facts', 'missing', surfaceOf)).toEqual([]);
  });

  it('lists the keys of an exported object literal', () => {
    expect(objectKeys('/s.ts', setup, 'facts')).toEqual(['errorFields', 'unrelated']);
    expect(objectKeys('/s.ts', 'export const facts = make();', 'facts')).toEqual([]);
  });
});

describe('docCommentExamples', () => {
  it('finds tagged @example blocks with their owner and claim', () => {
    const text = [
      '/**',
      ' * Doubles a number.',
      ' * @example',
      ' * ```ts',
      ' * // check',
      ' * assert.equal(double(2), 4);',
      ' * ```',
      ' * @example',
      ' * // just prose',
      ' */',
      'export function double(n: number) { return n * 2; }',
    ].join('\n');
    expect(docCommentExamples('src/d.ts', text)).toMatchObject([
      {
        page: 'src/d.ts',
        line: 1,
        owner: 'double',
        claim: 'Doubles a number.',
        mode: 'check',
        code: 'assert.equal(double(2), 4);',
        weak: false,
      },
    ]);
  });
});
