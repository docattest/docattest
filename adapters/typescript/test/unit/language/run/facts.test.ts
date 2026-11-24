import { describe, expect, it } from 'vitest';
import { createProject } from 'docattest';

const CONFIG = JSON.stringify({
  pages: ['docs/**/*.md'],
  surfaces: { default: './src/index.ts', cli: './src/main.ts' },
  adapter: 'typescript',
});

// A library, and a command's entry script, which runs (and here fails) when it is imported.
const SOURCES = {
  'docattest.config.json': CONFIG,
  'src/index.ts': [
    'const SECOND = 1000;',
    'const requestTimeout = 30 * SECOND;',
    "export const DEFAULTS = { region: 'nl', retries: 3 } as const;",
    'function round(n: number): number { return Math.round(n); }',
    'let calls = 0;',
    'const started = Date.now();',
    'const seen: string[] = [];',
    'export function track(name: string): number { seen.push(name); calls += 1; return started; }',
    'export const half = (n: number) => round(n / 2);',
  ].join('\n'),
  'src/main.ts': [
    'const listenPort = 8080;',
    "export const regions = ['nl', 'de'] as const;",
    "export function usage(): string { return 'usage'; }",
    "throw new Error('main.ts ran');",
  ].join('\n'),
};

const check = async (page: string) => {
  const report = await createProject({ files: { ...SOURCES, 'docs/a.md': page } }).check();
  return report.results.map((r) => [r.status, r.message]);
};

describe('a fact over a constant', () => {
  it('reads its value from source, exported or not', async () => {
    const page = [
      'A request is cancelled after `30000` ms. <!-- fact: requestTimeout -->',
      'which is `30` seconds. <!-- fact: requestTimeout / 1000 -->',
      'The default region is `NL`. <!-- fact: DEFAULTS.region.toUpperCase() -->',
    ].join('\n\n');
    expect(await check(page)).toEqual([
      ['pass', 'requestTimeout is 30000'],
      ['pass', 'requestTimeout / 1000 is 30'],
      ['pass', 'DEFAULTS.region.toUpperCase() is NL'],
    ]);
  });

  it('never runs the module, so an entry script that runs on import can be a surface', async () => {
    const page = [
      'The server listens on `8080`. <!-- fact @cli: listenPort -->',
      'It serves: <!-- fact @cli: regions -->\n\n- `nl`\n- `de`',
      'Its usage is `usage`. <!-- fact @cli: usage() -->',
    ].join('\n\n');
    const [port, regions, usage] = await check(page);
    expect([port, regions]).toEqual([
      ['pass', 'listenPort is 8080'],
      ['pass', '2 items match regions'],
    ]);
    // A function is read from the module, which then runs.
    expect(usage).toEqual([
      'error',
      expect.stringMatching(/surface "cli" failed to load: .*main\.ts ran/),
    ]);
  });

  it('reads only constants of the names the surface does not export', async () => {
    const page = [
      'Rounds `2`. <!-- fact: round(2) -->',
      'Counted `0`. <!-- fact: calls -->',
      'Started `0`. <!-- fact: started -->',
      'Seen `0`. <!-- fact: seen.length -->',
    ].join('\n\n');
    const only = 'of its unexported names, a fact reads only constants';
    const computed = 'and the module computes or changes its value as it runs';
    expect(await check(page)).toEqual([
      [
        'error',
        `fact \`round(2)\`: round is a function that surface "default" does not export; ${only}`,
      ],
      [
        'error',
        `fact \`calls\`: calls is a variable that surface "default" does not export; ${only}`,
      ],
      [
        'error',
        `fact \`started\`: started is a const that surface "default" does not export, ${computed}; ${only}`,
      ],
      [
        'error',
        `fact \`seen.length\`: seen is a const that surface "default" does not export, ${computed}; ${only}`,
      ],
    ]);
  });
});

describe('a constant a fact reads', () => {
  const page = 'The server listens on `8080`. <!-- fact @cli: listenPort -->\n';

  it('is attested, as `where` shows, while `uncovered` lists only exports', async () => {
    const project = createProject({ files: { ...SOURCES, 'docs/a.md': page } });
    const where = await project.where('listenPort');
    expect(where.map((r) => [r.page, r.kind])).toEqual([['docs/a.md', 'fact']]);
    expect(await project.uncovered('cli')).toEqual(['regions', 'usage']);
  });

  it('is listed by `impact` when it changes, with the facts that read it', async () => {
    const files = { ...SOURCES, 'docs/a.md': page };
    const changed = {
      ...files,
      'src/main.ts': files['src/main.ts'].replace('8080', '8081'),
    };
    const impact = await createProject({ files: changed }).impact({ baseFiles: files });
    expect(impact.symbols).toEqual(['listenPort']);
    expect(impact.references.map((r) => [r.symbol, r.kind])).toEqual([['listenPort', 'fact']]);
  });
});

describe('a value the package entry does not re-export', () => {
  const files = {
    'docattest.config.json': JSON.stringify({
      pages: ['docs/**/*.md'],
      surfaces: { default: './src/index.ts', internal: './src/caps.ts' },
      adapter: 'typescript',
    }),
    'src/caps.ts': "export const FORBIDDEN = new Set(['sys_admin'].map((c) => c.toUpperCase()));",
    'src/index.ts': [
      "import { FORBIDDEN } from './caps.js';",
      'export const allowed = (cap: string) => !FORBIDDEN.has(cap);',
    ].join('\n'),
    'docs/a.md': 'Never granted: <!-- fact @internal: [...FORBIDDEN] -->\n\n- `SYS_ADMIN`\n',
  };

  it('is read and attested on a surface of the module that exports it', async () => {
    const project = createProject({ files });
    expect((await project.check()).results.map((r) => r.status)).toEqual(['pass']);
    expect((await project.where('FORBIDDEN')).map((r) => [r.page, r.kind])).toEqual([
      ['docs/a.md', 'fact'],
    ]);
    expect(await project.uncovered()).toEqual(['allowed']);
  });
});

describe('ESM __dirname declarations and internal surface paths without ./', () => {
  it('loads an ESM surface declaring const __dirname and resolves paths without ./', async () => {
    const project = createProject({
      files: {
        'docattest.config.json': JSON.stringify({
          pages: ['docs/**/*.md'],
          surfaces: {
            default: './src/index.ts',
            internal: { path: 'src/internal.ts', internal: true },
          },
          adapter: 'typescript',
        }),
        'src/index.ts': [
          "import * as path from 'node:path';",
          "import { fileURLToPath } from 'node:url';",
          'const __filename = fileURLToPath(import.meta.url);',
          'const __dirname = path.dirname(__filename);',
          'export const DIR_BASE = path.basename(__dirname);',
        ].join('\n'),
        'src/internal.ts': "const SECRET_SCOPE = 'read:all';\nexport const ok = true;\n",
        'docs/a.md': [
          'Lives in `src`. <!-- fact: DIR_BASE -->',
          'Default scope is `read:all`. <!-- fact @internal: SECRET_SCOPE -->',
        ].join('\n'),
      },
    });
    const report = await project.check();
    expect(report.results.map((r) => r.status)).toEqual(['pass', 'pass']);
  });
});
