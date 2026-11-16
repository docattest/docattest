import { describe, expect, it } from 'vitest';
import { affectedExports } from '../../../src/language/impact/affected.js';
import { sourceOf } from '../../../src/language/impact/program.js';
import { MEMORY_ROOT, memoryFiles } from '../../../src/files/memory.js';

const SPECS = [{ name: 'default', import: 'docattest-fixture', path: 'src/index.ts' }];

const LIB: Record<string, string> = {
  'src/index.ts': [
    "export { run, Options, describe as explain, LIMIT, make, register } from './a.js';",
    "export type { Shape } from './types.js';",
    "export { default as defaulted } from './d.js';",
  ].join('\n'),
  'src/a.ts': [
    "import { helper } from './b.js';",
    "import { Box } from './box.js';",
    "import { registry } from './reg.js';",
    "import './plugins.js';",
    'export interface Options { retries: number }',
    'export const LIMIT = 10;',
    'export function run(o: Options): number { return helper(o.retries) + LIMIT; }',
    "export function describe(): string { return 'a'; }",
    'export function make(): Box { return new Box(); }',
    'export function register(): string[] { return [...registry.keys()]; }',
  ].join('\n'),
  'src/b.ts': [
    "import { deep } from './c.js';",
    'export function helper(n: number): number { return deep(n) * 2; }',
    'export function other(): number { return 1; }',
  ].join('\n'),
  'src/c.ts': 'export function deep(n: number): number { return n + 1; }',
  'src/box.ts': "export class Box { open() { return 'open'; } close() { return 'shut'; } }",
  'src/types.ts': 'export interface Shape { x: number }\nexport type Unused = string;',
  'src/reg.ts': 'export const registry = new Map<string, () => void>();',
  'src/plugins.ts': "import { registry } from './reg.js';\nregistry.set('a', () => {});",
  'src/d.ts': 'export default function (): number { return 1; }',
  'src/dead.ts': 'export const dead = 1;',
};

/** Names the change from LIB to LIB + `head` affects (`null` deletes a file). */
function affected(head: Record<string, string | null>, base = LIB): string[] {
  const merged = Object.entries({ ...base, ...head });
  const now = Object.fromEntries(merged.filter((e): e is [string, string] => e[1] !== null));
  const changed = [...new Set([...Object.keys(base), ...Object.keys(head)])];
  const change = { base: memoryFiles(base), changed };
  return affectedExports(memoryFiles(now), SPECS, change).map((r) => r.name);
}

const edit = (file: string, from: string, to: string) => {
  const text = LIB[file] ?? '';
  if (!text.includes(from)) throw new Error(`fixture: ${from} not in ${file}`);
  return { [file]: text.replace(from, to) };
};

describe('affectedExports', () => {
  it('finds the export above a helper changed two calls down', () => {
    expect(affected(edit('src/c.ts', 'n + 1', 'n + 2'))).toEqual(['run']);
  });

  it('follows types: a field added to an interface affects it and its users', () => {
    expect(affected(edit('src/a.ts', 'retries: number }', 'retries: number; d?: 1 }'))).toEqual([
      'Options',
      'run',
    ]);
    expect(affected(edit('src/types.ts', 'x: number }', 'x: number; y: number }'))).toEqual([
      'Shape',
    ]);
  });

  it('follows class members, re-exports under another name and default exports', () => {
    expect(affected(edit('src/box.ts', "'shut'", "'closed'"))).toEqual(['make']);
    expect(affected(edit('src/a.ts', "return 'a'", "return 'b'"))).toEqual(['explain']);
    expect(affected(edit('src/d.ts', 'return 1', 'return 2'))).toEqual(['defaulted']);
  });

  it('counts side effects at import as part of what they change', () => {
    expect(affected(edit('src/plugins.ts', "'a'", "'b'"))).toEqual(['register']);
  });

  it('says nothing about code no export reaches', () => {
    expect(affected(edit('src/dead.ts', '1', '2'))).toEqual([]);
    expect(affected(edit('src/b.ts', 'return 1;', 'return 2;'))).toEqual([]);
    expect(affected(edit('src/types.ts', 'Unused = string', 'Unused = number'))).toEqual([]);
  });

  it('ignores comments, whitespace, semicolons, trailing commas, quotes and parentheses', () => {
    const reformatted = LIB['src/a.ts']
      .replace('export const LIMIT = 10;', '/** The limit. */\nexport const LIMIT = (10)\n')
      .replace("return 'a';", 'return "a"');
    expect(affected({ 'src/a.ts': reformatted })).toEqual([]);
    const wrapped = 'export function deep(\n  n: number,\n): number {\n  return n + 1.0\n}\n';
    expect(affected({ 'src/c.ts': wrapped })).toEqual([]);
    expect(affected({})).toEqual([]);
  });

  it('treats renaming a parameter or a local as no change, unless it changes what code means', () => {
    const helper = (body: string) => ({
      'src/b.ts': `import { deep } from './c.js';\n${body}\nexport function other(): number { return 1; }`,
    });
    const base = {
      ...LIB,
      ...helper('export function helper(n: number) { const d = deep(n); return d * 2; }'),
    };
    const renamed =
      'export function helper(count: number) { const x = deep(count); return x * 2; }';
    expect(affected(helper(renamed), base)).toEqual([]);
    const shadowed =
      'export function helper(n: number) { const deep = (k: number) => k; return deep(n) * 2; }';
    expect(affected(helper(shadowed), base)).toEqual(['run']);
    // A shorthand property's name is its key: renaming the local renames the key.
    const keyed = (name: string) =>
      `export function deep(n: number) { const ${name} = n; return Object.keys({ ${name} })[0] === 'v' ? n : 0; }`;
    const base2 = { ...LIB, 'src/c.ts': keyed('v') };
    expect(affected({ 'src/c.ts': keyed('w') }, base2)).toEqual(['run']);
  });

  it('treats an internal helper renamed or moved along with its callers as no change', () => {
    const renamed = LIB['src/b.ts'].replace(/deep/g, 'increment');
    expect(
      affected({
        'src/b.ts': renamed,
        'src/c.ts': LIB['src/c.ts'].replace('deep', 'increment'),
      }),
    ).toEqual([]);
    const moved = {
      'src/b.ts': LIB['src/b.ts'].replace("'./c.js'", "'./util/math.js'"),
      'src/c.ts': null,
      'src/util/math.ts': LIB['src/c.ts'],
    };
    expect(affected(moved)).toEqual([]);
    // A whole module moved: its declarations use each other, and still nothing changed.
    const pair =
      'export const half = (n: number) => n / 2;\nexport const quarter = (n: number) => half(half(n));';
    const before = { ...LIB, 'src/c.ts': `${LIB['src/c.ts']}\n${pair}` };
    const relocated = {
      'src/c.ts': null,
      'src/lib/c.ts': before['src/c.ts'],
      'src/b.ts': moved['src/b.ts'].replace('./util/math.js', './lib/c.js'),
    };
    expect(affected(relocated, before)).toEqual([]);
    const renamedAndChanged = LIB['src/c.ts']
      .replace('deep', 'increment')
      .replace('n + 1', 'n + 2');
    expect(affected({ 'src/b.ts': renamed, 'src/c.ts': renamedAndChanged })).toEqual(['run']);
  });

  it('counts new, removed and retargeted exports, and declarations that changed kind', () => {
    const index = LIB['src/index.ts'] ?? '';
    expect(affected({ 'src/index.ts': `${index}\nexport { other } from './b.js';` })).toEqual([
      'other',
    ]);
    expect(affected({ 'src/index.ts': index.replace(', LIMIT', '') })).toEqual(['LIMIT']);
    expect(affected({ 'src/index.ts': index.replace('describe as', 'run as') })).toEqual([
      'explain',
    ]);
    expect(affected(edit('src/a.ts', 'export const LIMIT', 'export let LIMIT'))).toEqual([
      'LIMIT',
      'run',
    ]);
  });

  it('sees a dependency the change removed or pointed at other code', () => {
    const inlined = 'export function helper(n: number): number { return (n + 1) * 2; }';
    expect(affected({ 'src/b.ts': inlined, 'src/c.ts': null })).toEqual(['run']);
    // Pointing an import at an identical copy changes nothing; at different code, it does.
    const pointed = { 'src/b.ts': LIB['src/b.ts'].replace("'./c.js'", "'./c2.js'") };
    const copy = LIB['src/c.ts'];
    expect(affected(pointed, { ...LIB, 'src/c2.ts': copy })).toEqual([]);
    expect(affected(pointed, { ...LIB, 'src/c2.ts': copy.replace('n + 1', 'n - 1') })).toEqual([
      'run',
    ]);
  });
});

describe('sourceOf', () => {
  it('traces a built entry back to its source through the source map', () => {
    const files = memoryFiles({
      'dist/index.js': 'export {};',
      'dist/index.d.ts': 'export {};',
      'dist/index.js.map': JSON.stringify({ version: 3, sources: ['../src/index.ts'] }),
      'src/index.ts': 'export {};',
    });
    expect(sourceOf(files, `${MEMORY_ROOT}/dist/index.d.ts`)).toBe(`${MEMORY_ROOT}/src/index.ts`);
    expect(sourceOf(files, `${MEMORY_ROOT}/src/index.ts`)).toBe(`${MEMORY_ROOT}/src/index.ts`);
    expect(sourceOf(files, `${MEMORY_ROOT}/lib/other.js`)).toBe(`${MEMORY_ROOT}/lib/other.js`);
  });
});
