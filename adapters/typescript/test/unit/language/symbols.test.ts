import { describe, expect, it } from 'vitest';
import { createProject } from 'docattest';

// A helper reads one export in its own code and another through a setup function it calls.
const FILES = {
  'docattest.config.json': JSON.stringify({
    pages: ['docs/**/*.md'],
    surfaces: { default: './src/index.ts' },
    setup: 'docs/docattest.setup.ts',
    adapter: 'typescript',
  }),
  'src/index.ts': [
    "export const FORBIDDEN_CAPS = ['SYS_ADMIN', 'NET_RAW'];",
    "export const COMMAND_PATH = '/usr/bin/shopd';",
    'export const UNUSED = 1;',
  ].join('\n'),
  'docs/docattest.setup.ts': [
    "import { COMMAND_PATH, FORBIDDEN_CAPS } from 'docattest-fixture';",
    'function command(): string { return COMMAND_PATH; }',
    'export const facts = { caps: () => [...FORBIDDEN_CAPS, command()] };',
  ].join('\n'),
  'docs/a.md': [
    'Never granted, nor run: <!-- fact: caps() -->',
    '',
    '- `SYS_ADMIN`',
    '- `NET_RAW`',
    '- `/usr/bin/shopd`',
    '',
  ].join('\n'),
};

describe('a fact that calls a named fact of the setup module', () => {
  it('attests the exports its helper reaches, as `where` and `uncovered` show', async () => {
    const project = createProject({ files: FILES });
    const where = await Promise.all(
      ['FORBIDDEN_CAPS', 'COMMAND_PATH'].map((s) => project.where(s)),
    );
    expect(where.map((refs) => refs.map((r) => [r.page, r.line]))).toEqual([
      [['docs/a.md', 1]],
      [['docs/a.md', 1]],
    ]);
    expect(await project.uncovered()).toEqual(['UNUSED']);
  });

  it('is listed by `impact` when one of those exports changes', async () => {
    const changed = { ...FILES, 'src/index.ts': FILES['src/index.ts'].replace('/usr', '/opt') };
    const impact = await createProject({ files: changed }).impact({ baseFiles: FILES });
    expect(impact.symbols).toEqual(['COMMAND_PATH']);
    expect(impact.references.map((r) => [r.symbol, r.line])).toEqual([['COMMAND_PATH', 1]]);
  });
});

// ENV is built from COMMAND_PATH, itself built from BIN, and from a constant the module keeps
// private. `total` reads LIMIT in its body, which calling it does not attest.
const VALUES = {
  ...FILES,
  'src/index.ts': [
    'const STDERR_OFFSET = 2;',
    "export const BIN = '/bin';",
    'export const COMMAND_PATH = `${BIN}/run`;',
    'export const ENV = [`PATH=${COMMAND_PATH}`, `STDERR=${STDERR_OFFSET}`];',
    'export const LIMIT = 3;',
    'export const total = (): number => LIMIT;',
  ].join('\n'),
  'docs/docattest.setup.ts': [
    "import { ENV } from 'docattest-fixture';",
    'export const facts = { env: () => ENV };',
    'export const cases = {',
    '  env: (row: { line: string }) => {',
    '    if (!ENV.includes(row.line)) throw new Error(`${row.line} is not set`);',
    '  },',
    '};',
  ].join('\n'),
};
const LIST = ['', '- `PATH=/bin/run`', '- `STDERR=2`', ''];
const PROOFS: [string, string[]][] = [
  ['a fact', ['The command runs with: <!-- fact: ENV -->', ...LIST]],
  ['a named fact', ['The command runs with: <!-- fact: env() -->', ...LIST]],
  ['a cases runner', ['<!-- cases: env -->', '| line |', '|---|', '| PATH=/bin/run |', '']],
];

describe.each(PROOFS)('%s that reads a value built from other names', (_, page) => {
  const files = {
    ...VALUES,
    'docs/a.md': [...page, 'At most `3`. <!-- fact: total() -->'].join('\n'),
  };

  it('attests the names its initializer uses, at any depth, private ones too', async () => {
    const project = createProject({ files });
    expect((await project.check()).results.map((r) => r.status)).toEqual(['pass', 'pass']);
    const names = ['ENV', 'COMMAND_PATH', 'BIN', 'STDERR_OFFSET', 'LIMIT'];
    const where = await Promise.all(names.map((s) => project.where(s)));
    expect(where.map((refs) => refs.map((r) => r.line))).toEqual([[1], [1], [1], [1], []]);
    expect(await project.uncovered()).toEqual(['LIMIT']);
  });

  it('is listed by `impact` when one of those names changes', async () => {
    const changed = { ...files, 'src/index.ts': files['src/index.ts'].replace("'/bin'", "'/opt'") };
    const impact = await createProject({ files: changed }).impact({ baseFiles: files });
    expect(impact.references.map((r) => [r.symbol, r.line])).toEqual([
      ['BIN', 1],
      ['COMMAND_PATH', 1],
      ['ENV', 1],
    ]);
  });
});

describe('a citation whose unit test imports a relative submodule', () => {
  it('attributes symbols imported from a submodule re-exported by a source or built surface', async () => {
    const files = {
      'package.json': JSON.stringify({ name: '@acme/keys', version: '1.0.0' }),
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: {
          default: './src/index.ts',
          built: { import: '@acme/built', path: 'packages/built/dist/index.js' },
        },
        adapter: 'typescript',
      }),
      'src/publicKeys.ts': 'export function parseKeys(raw: string) { return raw.trim(); }\n',
      'src/index.ts': "export { parseKeys } from './publicKeys.js';\n",
      'test/publicKeys.test.ts': [
        "import { parseKeys } from '../src/publicKeys.js';",
        "it('parses keys', () => { parseKeys('a'); });",
      ].join('\n'),
      'packages/built/package.json': JSON.stringify({ name: '@acme/built', version: '1.0.0' }),
      'packages/built/dist/index.js': 'export function buildKey() { return 1; }\n',
      'packages/built/dist/index.d.ts': 'export declare function buildKey(): number;\n',
      'packages/built/src/key.ts': 'export function buildKey() { return 1; }\n',
      'packages/built/src/__tests__/key.test.ts': [
        "import { buildKey } from '../key.js';",
        "it('builds key', () => { buildKey(); });",
      ].join('\n'),
      'docs/a.md': [
        '`parseKeys` trims keys. <!-- test: test/publicKeys.test.ts > "parses keys" -->',
        '`buildKey` creates a key. <!-- test: packages/built/src/__tests__/key.test.ts > "builds key" -->',
      ].join('\n'),
    };
    const project = createProject({ files });
    const report = await project.check();
    expect(report.results.map((r) => r.status)).toEqual(['pass', 'pass']);
    expect(report.warnings).toBeUndefined();
    expect((await project.where('parseKeys')).map((r) => r.kind)).toEqual(['test']);
    expect((await project.where('buildKey')).map((r) => r.kind)).toEqual(['test']);
  });
});
