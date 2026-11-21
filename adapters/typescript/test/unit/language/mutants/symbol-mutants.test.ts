import { Buffer } from 'node:buffer';
import { describe, expect, it } from 'vitest';
import type { MutantEdit, SymbolMutants, SymbolRef } from '../../../../src/api/adapter.js';
import { memoryFiles } from '../../../../src/files/memory.js';
import { symbolMutants } from '../../../../src/language/mutants/symbol-mutants.js';

const SPECS = [{ name: 'default', import: 'docattest-fixture', path: 'src/index.ts' }];

const PROJECT: Record<string, string> = {
  'src/index.ts': [
    "export { PORT, HOSTS as SERVERS } from './config.js';",
    "export * from './flags.js';",
    "export { helper } from 'some-dependency';",
    'const TIMEOUT = 30;',
    'export function start(): number { return TIMEOUT; }',
    'export const stop = () => 0;',
    'export class Server {}',
    'export interface Options { port: number }',
    'export enum Level { Low = 1 }',
    'export const made = Date.now();',
    "export const lazy = { read: () => 'a' };",
    'export const Color = { red: 1 } as const;',
    'export type Color = (typeof Color)[keyof typeof Color];',
    'export const PAIR: [number, number] = [1, 2];',
    'export const LIST: number[] = [1, 2];',
    'export const COPY = TIMEOUT;',
  ].join('\n'),
  'src/config.ts': "export const PORT = 8080;\nexport const HOSTS = ['a', 'b'];\n",
  'src/flags.ts': 'export const DEBUG = false;\n',
};

function mutants(
  symbols: SymbolRef[],
  files: Record<string, string> = PROJECT,
  limit = 8,
): SymbolMutants[] {
  return symbolMutants(memoryFiles(files), SPECS, symbols, limit);
}

/** Each edit's description, and the line it leaves, with the edit applied to the file's bytes. */
function applied(answer: SymbolMutants, files: Record<string, string> = PROJECT) {
  return answer.edits.map((edit: MutantEdit) => {
    const bytes = Buffer.from(files[edit.path] ?? '');
    const edited = Buffer.concat([
      bytes.subarray(0, edit.start),
      Buffer.from(edit.replacement),
      bytes.subarray(edit.end),
    ]).toString();
    const line = bytes.subarray(0, edit.start).toString().split('\n').length - 1;
    return [edit.path, edit.description, edited.split('\n')[line]];
  });
}

const onSurface = (name: string): SymbolRef => ({ name, surface: 'default' });

describe('symbolMutants', () => {
  it("resolves a surface's symbol through re-exports to the file that declares it", () => {
    const [port, servers, debug] = mutants([
      onSurface('PORT'),
      onSurface('SERVERS'),
      onSurface('DEBUG'),
    ]);
    expect(applied(port)).toEqual([['src/config.ts', '8080 → 8081', 'export const PORT = 8081;']]);
    expect(applied(servers)).toEqual([
      ['src/config.ts', "'b' removed", "export const HOSTS = ['a'];"],
      ['src/config.ts', "'a' → 'ax'", "export const HOSTS = ['ax', 'b'];"],
      ['src/config.ts', "'b' → 'bx'", "export const HOSTS = ['a', 'bx'];"],
    ]);
    expect(applied(debug)).toEqual([
      ['src/flags.ts', 'false → true', 'export const DEBUG = true;'],
    ]);
  });

  it('finds a constant a fact reads from source, with its surface or without one', () => {
    const [attributed, bare] = mutants([onSurface('TIMEOUT'), { name: 'TIMEOUT' }]);
    const timeout = [['src/index.ts', '30 → 31', 'const TIMEOUT = 31;']];
    expect(applied(attributed)).toEqual(timeout);
    expect(applied(bare)).toEqual(timeout);
  });

  it('says why a symbol has no edits: a function, no literal, or not found', () => {
    const names = ['start', 'stop', 'Server', 'Options', 'Level', 'made', 'lazy'];
    const unknown = [onSurface('nothing'), { name: 'nothing' }, { name: 'PORT', surface: 'x' }];
    const answers = mutants([...names.map(onSurface), onSurface('helper'), ...unknown]);
    expect(answers.map((a) => ('why' in a ? a.why : a.edits.length))).toEqual([
      'function',
      'function',
      'function',
      'function',
      'function',
      'noLiteral',
      'noLiteral',
      'notFound',
      'notFound',
      'notFound',
      'notFound',
    ]);
    expect(answers.every((a) => 'edits' in a)).toBe(true);
  });

  it('reads the value of a name that a type shares, and keeps the length of a tuple', () => {
    const [color, pair, list] = mutants([onSurface('Color'), onSurface('PAIR'), onSurface('LIST')]);
    expect(applied(color).map(([, d]) => d)).toEqual(['1 → 2']);
    expect(applied(pair).map(([, d]) => d)).toEqual(['1 → 2', '2 → 3']);
    expect(applied(list).map(([, d]) => d)).toEqual(['2 removed', '1 → 2', '2 → 3']);
  });

  it('answers at most the limit, the first edits in source order', () => {
    const [list] = mutants([onSurface('LIST')], PROJECT, 2);
    expect(applied(list).map(([, d]) => d)).toEqual(['2 removed', '1 → 2']);
  });

  it('mutates a non-literal scalar constant as a typed expression', () => {
    const files = {
      'src/index.ts': [
        'const BASE = 5;',
        "const PREFIX = 'ok';",
        'const FLAG = true;',
        'export const PERIOD = BASE;',
        'export const LABEL = PREFIX;',
        'export const ALSO = FLAG;',
      ].join('\n'),
    };
    const [period, label, also] = mutants(
      [onSurface('PERIOD'), onSurface('LABEL'), onSurface('ALSO')],
      files,
    );
    expect(applied(period, files)).toEqual([
      ['src/index.ts', 'BASE → 2 * BASE', 'export const PERIOD = 2 * BASE;'],
    ]);
    expect(applied(label, files)).toEqual([
      ['src/index.ts', "PREFIX → PREFIX + 'x'", "export const LABEL = PREFIX + 'x';"],
    ]);
    expect(applied(also, files)).toEqual([
      ['src/index.ts', 'FLAG → !FLAG', 'export const ALSO = !FLAG;'],
    ]);
  });

  it('counts offsets in bytes of UTF-8, whatever comes before the literal', () => {
    const files = { 'src/index.ts': "// café ☕ 🚀\nexport const NAME = 'naïve';\n" };
    const [name] = mutants([onSurface('NAME')], files);
    expect(applied(name, files)).toEqual([
      ['src/index.ts', "'naïve' → 'naïvex'", "export const NAME = 'naïvex';"],
    ]);
    const [edit] = name.edits;
    const text = files['src/index.ts'];
    expect(edit?.start).toBe(Buffer.byteLength(text.slice(0, text.indexOf("'naïve'"))));
    expect(edit?.start).not.toBe(text.indexOf("'naïve'"));
  });
});
