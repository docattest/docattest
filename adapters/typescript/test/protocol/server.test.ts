// The server's side of PROTOCOL.md, line by line: the handshake, errors that leave it
// running, answers in completion order, end of input, and a stdout that carries nothing
// but responses.
import { spawnSync } from 'node:child_process';
import { describe, expect, it } from 'vitest';
import { sourceFileMatcher } from '../../src/files/glob.js';
import { byId, example, exchange, fact, HELLO, open } from './exchange.ts';

describe('the adapter server', () => {
  it('greets with its protocol, languages and capabilities, and refuses another protocol', () => {
    const x = exchange([HELLO, { id: 1, method: 'hello', params: { protocol: 2 } }]);
    expect(byId(x, 0)?.result).toMatchObject({
      protocol: 1,
      adapter: { name: 'typescript' },
      sourceFiles: {
        include: ['**/*.{ts,tsx,mts,mtsx,cts,ctsx,js,jsx,mjs,mjsx,cjs,cjsx}'],
        exclude: ['**/*.d.{ts,mts,cts}'],
      },
      ignoredNames: ['node_modules'],
      capabilities: ['tests', 'runTests', 'observables', 'affected', 'display', 'mutants'],
    });
    expect(byId(x, 1)?.error).toEqual({
      kind: 'user',
      message: 'docattest: the typescript adapter speaks protocol 1, not 2',
    });
    expect(x.code).toBe(0);
  });

  it('reads doc comments in exactly the files its greeting calls source files', () => {
    const comment = [
      '/**',
      ' * Doubles.',
      ' * @example',
      ' * ```ts',
      ' * // check',
      ' * double(2);',
    ];
    const text = [...comment, ' * ```', ' */', 'export const double = 1;', ''].join('\n');
    const dirs = ['', 'src/', '.hidden/', 'a/.b/c/', 'x.d.ts/', 'sp ace/'];
    const names = ['a', '.', 'a.d', 'a.test', 'A.D', ''];
    const exts = ['ts', 'tsx', 'mts', 'cts', 'js', 'jsx', 'mjs', 'cjs', 'mtsx', 'cjsx'];
    const others = ['d.ts', 'd.mts', 'd.cts', 'd.tsx', 'TS', 'md', 'json', 'ts.map', 'tsxx', ''];
    const pages = dirs.flatMap((dir) =>
      names.flatMap((name) => [...exts, ...others].map((ext) => `${dir}${name}.${ext}`)),
    );
    const files = pages.map((page) => ({ page, text }));
    const x = exchange([HELLO, { id: 1, method: 'docComments', params: { files } }]);
    const hello = byId(x, 0)?.result as { sourceFiles: { include: string[]; exclude: string[] } };
    const found = (byId(x, 1)?.result as { page: string; kind: string }[]).map((e) => e.page);
    // The core applies the greeting's globs as it applies the config's own.
    expect(found).toEqual(pages.filter(sourceFileMatcher(hello.sourceFiles)));
    expect(found).toContain('a/.b/c/a.test.mts');
    expect(found).not.toContain('src/a.d.ts');
    expect(found.length).toBeGreaterThan(300);
  });

  it('answers protocol mistakes with errors and keeps serving', () => {
    const x = exchange([
      { id: 1, method: 'exports', params: { surface: 'default' } },
      HELLO,
      'not json',
      '{"method":"hello"}',
      { id: 2, method: 'nope', params: {} },
      { id: 3, method: 'exports', params: { surface: 'default' } },
      open(4, { 'src/index.ts': 'export const N = 3;' }),
      open(5, {}),
      { id: 6, method: 'exports', params: {} },
      { id: 7, method: 'exports', params: { surface: 'default' } },
      { id: 8, method: 'toString', params: {} },
    ]);
    // Answers come in any order; sorted, they read in the order the lines were sent.
    const order = (id: unknown) => (id === null ? 1.5 : Number(id));
    const errors = x.responses
      .filter((r) => r.error)
      .map((r) => [r.id, r.error?.message])
      .sort((a, b) => order(a[0]) - order(b[0]));
    expect(errors).toEqual([
      [1, '"exports" before "hello": hello is always first'],
      [null, 'a request line is not JSON: not json'],
      [null, 'a request has no "id" (a number or a string): {"method":"hello"}'],
      [2, 'unknown method "nope"'],
      [3, '"exports" before "open"'],
      [5, '"open" was already called: one session per process'],
      [6, 'exports: "surface" must be a string'],
      [8, 'unknown method "toString"'],
    ]);
    expect(x.responses.filter((r) => r.error).every((r) => r.error?.kind === 'internal')).toBe(
      true,
    );
    expect(byId(x, 7)?.result).toEqual(['N']);
    expect(x.code).toBe(0);
  });

  it('answers each request when it finishes, not in the order it was asked', () => {
    const slow = example('slow', 'await new Promise((r) => setTimeout(r, 400));');
    const x = exchange([
      HELLO,
      open(1, { 'src/index.ts': 'export const N = 3;' }, [slow]),
      { id: 2, method: 'run', params: { examples: [slow], cases: [] } },
      { id: 3, method: 'exports', params: { surface: 'default' } },
    ]);
    expect(x.responses.map((r) => r.id).slice(-2)).toEqual([3, 2]);
    expect(byId(x, 2)?.result).toMatchObject([{ status: 'pass' }]);
  });

  it('keeps stdout to protocol lines while examples and surfaces print', () => {
    const surface = [
      "process.stdout.write('surface wrote to stdout\\n');",
      "console.log('surface logged');",
      'export const N = 3;',
    ].join('\n');
    const loud = example(
      'loud',
      "console.log('example logged'); process.stdout.write('example wrote\\n');",
    );
    const x = exchange([
      HELLO,
      open(1, { 'src/index.ts': surface }, [loud]),
      { id: 2, method: 'evaluate', params: { facts: [fact('n', 'N')] } },
      { id: 3, method: 'run', params: { examples: [loud], cases: [] } },
      { id: 4, method: 'observables', params: { surface: 'default' } },
      { id: 5, method: 'close', params: {} },
    ]);
    expect(x.stdout).not.toMatch(/wrote|logged/);
    expect(x.stdout.split('\n').filter(Boolean)).toHaveLength(6);
    expect(byId(x, 2)?.result).toMatchObject({ n: { ok: true, value: { text: '3' } } });
    expect(byId(x, 3)?.result).toMatchObject([{ status: 'pass' }]);
    expect(byId(x, 5)?.result).toEqual({});
  });

  it('turns even a direct write to descriptor 1 into stderr', () => {
    const direct = example(
      'direct',
      "import { writeSync } from 'node:fs'; writeSync(1, 'straight to fd 1\\n');",
    );
    const x = exchange([
      HELLO,
      open(1, { 'src/index.ts': 'export const N = 3;' }, [direct]),
      { id: 2, method: 'run', params: { examples: [direct], cases: [] } },
    ]);
    expect(byId(x, 2)?.result).toMatchObject([{ status: 'pass' }]);
    expect(x.stdout).not.toMatch(/straight/);
    expect(x.stderr).toMatch(/straight to fd 1/);
  });

  it('refuses to start without the launcher that gives it a descriptor for responses', () => {
    const serve = new URL('../../dist/protocol/serve.js', import.meta.url).pathname;
    const run = spawnSync(process.execPath, [serve], { input: '', encoding: 'utf8' });
    expect(run.status).not.toBe(0);
    expect(run.stdout).toBe('');
    expect(run.stderr).toMatch(/start it with docattest-adapter-typescript/);
  });
});
