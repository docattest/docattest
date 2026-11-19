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
