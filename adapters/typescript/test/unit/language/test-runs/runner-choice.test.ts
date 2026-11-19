import { describe, expect, it } from 'vitest';
import { memoryFiles, MEMORY_ROOT } from '../../../../src/files/memory.js';
import { chooseRunner } from '../../../../src/language/test-runs/runner-choice.js';

const at = (dir: string) => (dir ? `${MEMORY_ROOT}/${dir}` : MEMORY_ROOT);
const manifest = (devDependencies: Record<string, string> = {}) =>
  JSON.stringify({ name: 'p', devDependencies });
const installed = (name: string, bin: unknown) => JSON.stringify({ name, bin });

describe('chooseRunner', () => {
  it('runs a file with the runner it imports, from the package that declares it', () => {
    const files = memoryFiles({
      'package.json': manifest(),
      'node_modules/vitest/package.json': installed('vitest', { vitest: './vitest.mjs' }),
      'pkg/package.json': manifest({ vitest: '^3' }),
      'pkg/test/package.json': JSON.stringify({ type: 'module' }),
    });
    expect(chooseRunner(files, 'pkg/test/a.test.ts', 'vitest')).toEqual({
      ok: true,
      runner: { kind: 'vitest', dir: at('pkg'), bin: at('node_modules/vitest/vitest.mjs') },
    });
  });

  it('runs a file that imports nothing with the runner the nearest package declares', () => {
    const files = memoryFiles({
      'package.json': manifest({ vitest: '^3' }),
      'pkg/package.json': manifest({ jest: '^29' }),
      'pkg/node_modules/jest/package.json': installed('jest', './bin/jest.js'),
    });
    expect(chooseRunner(files, 'pkg/test/a.test.ts', undefined)).toEqual({
      ok: true,
      runner: { kind: 'jest', dir: at('pkg'), bin: at('pkg/node_modules/jest/bin/jest.js') },
    });
  });

  it("runs node:test with the adapter's Node from the nearest package", () => {
    const files = memoryFiles({ 'pkg/package.json': manifest() });
    expect(chooseRunner(files, 'pkg/test/a.test.ts', 'node:test')).toEqual({
      ok: true,
      runner: { kind: 'node:test', dir: at('pkg') },
    });
    expect(chooseRunner(memoryFiles({}), 'a.test.ts', 'node:test')).toEqual({
      ok: true,
      runner: { kind: 'node:test', dir: at('') },
    });
  });

  it('has no runner for a file that names none, or whose runner is not installed', () => {
    const files = memoryFiles({ 'package.json': manifest({ vitest: '^3' }) });
    expect(chooseRunner(memoryFiles({}), 'test/a.test.ts', undefined)).toEqual({
      ok: false,
      message:
        'test/a.test.ts imports its test functions from none of vitest, @jest/globals and ' +
        'node:test, and no package.json declares vitest or jest',
    });
    expect(chooseRunner(files, 'test/a.test.ts', undefined)).toEqual({
      ok: false,
      message: "Vitest is not installed: no node_modules/vitest from . up to the project's root",
    });
  });
});
