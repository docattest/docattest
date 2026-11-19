// The mirror test's own scenarios, beside the benchmark's: what the adapters must agree on
// that a drift benchmark does not measure. Each is written for TypeScript, as a benchmark
// scenario is, and translated by the tables of python.ts, go.ts and rust.ts. A fact's
// value is a constant's in every language, so a scenario names constants as all four may.
import { lines } from '../lib/lines.ts';
import { project, type Config } from '../bench/fixture.ts';
import type { Outcome, ScenarioBody } from '../bench/scenario.ts';

/** A scenario of the mirror's own: a project, a command, and what every language must get. */
export interface Agreement extends ScenarioBody {
  id: string;
  title: string;
}

const LIBRARY = './src/server.ts';
const CHECK = ['check', '--json'];

/** The library every scenario but two has: an unexported constant and function, and an export. */
const SERVER = lines(
  'const PORT = 8080;',
  'function secret(): number { return 7; }',
  "export const NAME = 'shop';",
);

/** A config of the bench fixture's shape, with these surfaces and keys besides. */
const configured = (surfaces: Record<string, string>, more: Config = {}): Config => ({
  pages: ['docs/**/*.md'],
  surfaces,
  adapter: 'typescript',
  ...more,
});

/** The kind and status of each result of `check --json`, in order. */
const results = (out: Outcome): string =>
  (out.json?.results ?? []).map((r) => `${r.kind}:${r.status}`).join(' ');

/** `check --json` exited with `code` and gave exactly these results. */
const gave = (out: Outcome, code: number, want: string[]): boolean =>
  out.code === code && results(out) === want.join(' ');

/** What a scenario sets up and runs, before it says what it expects. */
type Setup = Omit<ScenarioBody, 'expect'>;

/** A check of one page over the library, `src/server.ts`. */
const checked = (page: string, files: Record<string, string> = {}): Setup => ({
  files: project(
    { 'src/server.ts': SERVER, 'docs/page.md': page, ...files },
    configured({ default: LIBRARY }),
  ),
  run: CHECK,
});

const FACTS: Agreement[] = [
  {
    id: 'const-01',
    title: 'a fact reads a constant its surface does not export',
    ...checked(lines('The server listens on port `8080`. <!-- fact: PORT -->')),
    expect: (out) => gave(out, 0, ['fact:pass']),
  },
  {
    id: 'const-02',
    title: 'a fact over a function its surface does not export is an error',
    ...checked(lines('Secretly `7`. <!-- fact: secret() -->')),
    expect: (out) =>
      gave(out, 1, ['fact:error']) &&
      out.stdout.includes('of its unexported names, a fact reads only constants'),
  },
  {
    id: 'program-01',
    title: "a fact reads a program's constant, and the program never runs",
    files: project(
      {
        'src/server.ts': SERVER,
        'src/main.ts': lines('const PORT = 8080;', "throw new Error('shopd needs a terminal');"),
        'docs/page.md': lines('The program listens on port `8080`. <!-- fact @cli: PORT -->'),
      },
      configured({ default: LIBRARY, cli: './src/main.ts' }),
    ),
    run: CHECK,
    expect: (out) => gave(out, 0, ['fact:pass']),
  },
  {
    id: 'file-01',
    title: 'file("path") gives a project file its text',
    ...checked(lines('Released as `2.4.0`. <!-- fact: file("VERSION") -->'), {
      VERSION: '2.4.0',
    }),
    expect: (out) => gave(out, 0, ['fact:pass']),
  },
  {
    id: 'file-02',
    title: 'matched("path", regex) gives what the core matched, and no match is an error',
    ...checked(
      lines(
        'It runs as user `10001`. <!-- fact: matched("Dockerfile", "^USER (\\\\d+)$") -->',
        '',
        'It starts `server`. <!-- fact: matched("Dockerfile", "^ENTRYPOINT (\\\\w+)$") -->',
      ),
      { Dockerfile: 'FROM scratch\nUSER 10001\n' },
    ),
    expect: (out) => gave(out, 1, ['fact:pass', 'fact:error']),
  },
  {
    id: 'file-03',
    title:
      'manifestField("path", keys) gives the value at a path of a manifest, as the language holds it',
    ...checked(
      lines(
        'Version `0.2.2`, <!-- fact: manifestField("ci.yml", "deploy.variables.VERSION") -->',
        'on `3` replicas. <!-- fact: manifestField("ci.yml", "deploy.replicas") -->',
      ),
      { 'ci.yml': 'deploy:\n  replicas: 3\n  variables:\n    VERSION: 0.2.2\n' },
    ),
    expect: (out) => gave(out, 0, ['fact:pass', 'fact:pass']),
  },
  {
    id: 'field-01',
    title: "field= compares a JSON block's true, 1.0 and null with the language's own values",
    files: project(
      {
        'src/settings.ts': lines(
          'export const STRICT = true;',
          'export const RATIO = 1.0;',
          'export const NOTHING = null;',
        ),
        'docs/page.md': lines(
          '<!-- fact field=strict: STRICT -->',
          '<!-- fact field=ratio: RATIO -->',
          '<!-- fact field=nothing: NOTHING -->',
          '',
          '```json',
          '{ "strict": true, "ratio": 1.0, "nothing": null }',
          '```',
        ),
      },
      configured({ default: './src/settings.ts' }),
    ),
    run: CHECK,
    expect: (out) => gave(out, 0, ['fact:pass', 'fact:pass', 'fact:pass']),
  },
];

/** Tests of a table the file writes: each row is a test, titled as its runner titles it. */
const PRICE_TEST = lines(
  "import { describe, it } from 'vitest';",
  '',
  "const cases = [{ name: 'rounds down', cents: 1 }, { name: 'rounds up', cents: 5 }];",
  '',
  "describe('price', () => {",
  '  for (const c of cases) it(c.name, () => {});',
  '});',
);

const cited = (row: string): Setup =>
  checked(
    lines('# Price', '', `A price ${row}. <!-- test: test/price.test.ts > "price" > "${row}" -->`),
    {
      'test/price.test.ts': PRICE_TEST,
    },
  );

const CITATIONS: Agreement[] = [
  {
    id: 'table-01',
    title: 'a citation of one row of a table-driven test',
    ...cited('rounds up'),
    expect: (out) => gave(out, 0, ['test:pass']),
  },
  {
    id: 'table-02',
    title: 'a citation of a row the table does not have',
    ...cited('rounds over'),
    expect: (out) => gave(out, 1, ['test:error']),
  },
];

/** A cases runner that compares what the language's cell helper reads of a cell. */
const SETUP = lines(
  "import assert from 'node:assert/strict';",
  "import { codeSpans } from 'docattest';",
  '',
  'export const cases = {',
  "  spans: (row: Record<string, string>) => assert.equal(codeSpans(row.cell).join(','), row.spans),",
  '};',
);

const SETUP_AND_UNDOCUMENTED: Agreement[] = [
  {
    id: 'undoc-01',
    title: '`undocumented` keeps a symbol out of uncovered',
    files: project(
      {
        'src/rates.ts': lines(
          'export const RATE = 3;',
          'export const QUOTE = 2;',
          'export const DEBUG_LEVEL = 1;',
        ),
        'docs/page.md': lines('The rate is `3`. <!-- fact: RATE -->'),
      },
      configured({ default: './src/rates.ts' }, { undocumented: { default: ['DEBUG_*'] } }),
    ),
    run: ['uncovered', '--json'],
    expect: (out) => out.code === 0 && JSON.stringify(JSON.parse(out.stdout)) === '["QUOTE"]',
  },
  {
    id: 'cells-01',
    title: "a cases runner reads its cells' code with the language's helper",
    files: project(
      {
        'src/server.ts': SERVER,
        'docs/docattest.setup.ts': SETUP,
        'docs/page.md': lines(
          '<!-- cases: spans -->',
          '| case | cell | spans |',
          '|---|---|---|',
          '| two spans | `FOO=1` `BAR=2` | FOO=1,BAR=2 |',
          '| a backtick in one | ``a`b`` | a`b |',
          '| wrong on purpose | `x` | y |',
        ),
      },
      configured({ default: LIBRARY }, { setup: 'docs/docattest.setup.ts' }),
    ),
    run: CHECK,
    expect: (out) => gave(out, 1, ['cases:pass', 'cases:pass', 'cases:fail']),
  },
];

/** The mirror's own scenarios, in the order they run. */
export const agreements: readonly Agreement[] = [...FACTS, ...CITATIONS, ...SETUP_AND_UNDOCUMENTED];
