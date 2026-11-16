import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { ModuleHost } from '../../../../src/language/run/host.js';
import { MEMORY_ROOT, memoryFiles } from '../../../../src/files/memory.js';

const at = (file: string) => join(MEMORY_ROOT, file);
const files = memoryFiles({
  'src/index.ts': "export * from './util.js';",
  'src/util.ts': 'export const u = 1;',
  'src/data.json': '{"a":1}',
});
const host = new ModuleHost(files, new Map([['docattest-fixture', at('src/index.ts')]]));

describe('ModuleHost', () => {
  it('resolves surfaces by import name and relative imports by their compiled names', () => {
    expect(host.resolve('docattest-fixture', at('docs/x.ts'))).toEqual({
      kind: 'project',
      file: at('src/index.ts'),
    });
    expect(host.resolve('./util.js', at('src/index.ts'))).toEqual({
      kind: 'project',
      file: at('src/util.ts'),
    });
    expect(host.resolve('./data.json', at('src/index.ts'))).toEqual({
      kind: 'project',
      file: at('src/data.json'),
    });
  });

  it('says what is missing, relative to the project', () => {
    expect(host.resolve('./nope.js', at('src/index.ts'))).toEqual({
      kind: 'missing',
      message: "Cannot find module './nope.js' from src/index.ts",
    });
  });

  it('leaves dependencies to the worker, resolved from where docattest runs for in-memory files', () => {
    expect(host.resolve('node:fs', at('src/index.ts'))).toEqual({
      kind: 'external',
      base: join(process.cwd(), 'noop.js'),
    });
    expect(host.resolve('typescript', at('src/index.ts'))).toMatchObject({ kind: 'external' });
  });

  it('serves CommonJS for scripts and text for JSON', () => {
    expect(host.source(at('src/util.ts'))).toMatchObject({
      format: 'script',
      code: expect.stringContaining('exports.u = 1'),
    });
    expect(host.source(at('src/data.json'))).toEqual({ format: 'json', text: '{"a":1}' });
    expect(host.source(at('src/none.ts'))).toBeUndefined();
  });

  it('rejects malformed requests as a bug', () => {
    expect(() => host.handle({ op: 'eval' })).toThrow(/internal error/);
    expect(
      host.handle({ op: 'resolve', specifier: './util.js', from: at('src/index.ts') }),
    ).toMatchObject({ op: 'resolve' });
  });
});
