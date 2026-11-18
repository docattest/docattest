import { describe, expect, it } from 'vitest';
import { moduleNames } from '../../../../src/language/syntax/module-names.js';
import { parseSource } from '../../../../src/language/syntax/parse.js';

const names = (code: string) =>
  Object.fromEntries(
    [...moduleNames(parseSource('/m.ts', code))].map(([name, declared]) => [
      name,
      declared.kind === 'constant'
        ? declared.value
        : declared.kind === 'other'
          ? declared.what
          : 'computed',
    ]),
  );

describe('moduleNames', () => {
  it('reads each constant of a module, naming what else it declares', () => {
    const code = [
      'const SECOND = 1000;',
      'export const TIMEOUT = 30 * SECOND;',
      "const REGIONS = ['nl', 'de'] as const;",
      'let counter = 0;',
      'function round(n: number) { return n; }',
      'class Box {}',
      'enum Color { Red }',
      'interface Shape { x: number }',
      "import { other } from './other.js';",
      'const imported = other;',
      'const made = Date.now();',
    ].join('\n');
    expect(names(code)).toEqual({
      SECOND: 1000,
      TIMEOUT: 30000,
      REGIONS: ['nl', 'de'],
      counter: 'a variable',
      round: 'a function',
      Box: 'a class',
      Color: 'an enum',
      Shape: 'a type',
      imported: 'computed',
      made: 'computed',
    });
  });

  it('reads no const the module changes in place, or that names one declared below it', () => {
    const code = [
      'const LIST = [1];',
      'LIST.push(2);',
      'const CONFIG = { port: 1 };',
      'export function configure() { CONFIG.port = 2; }',
      'const EARLY = LATE + 1;',
      'const LATE = 1;',
    ].join('\n');
    expect(names(code)).toMatchObject({
      LIST: 'computed',
      CONFIG: 'computed',
      EARLY: 'computed',
      LATE: 1,
    });
  });
});
