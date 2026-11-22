// How pages are read: non-ASCII text, CRLF, bound and frozen pages, column names.
import { lines } from '../../lib/lines.ts';
import {
  config,
  exportList,
  fence,
  KINDS_SRC,
  list,
  one,
  table,
  TEMPLATE,
  type AuditCase,
} from './fixtures.ts';

export const nonAscii: AuditCase[] = [
  // Non-ASCII items, headers and pages.
  [
    'unicode: items and their order',
    one(
      exportList('K', ['größe', '日本', '😀smile', '！wide', 'zeta', 'Ünï']) + '\n',
      list('größe', '日本', 'zeta'),
    ),
  ],
  [
    'unicode: unexpected items sorted by UTF-16 unit',
    one(
      exportList('K', ['aa', 'bb', 'cc', 'dd']) + '\n',
      list('aa', 'bb', 'cc', 'dd', '！wide', '😀smile', 'éa', 'Zz'),
    ),
  ],
  [
    'unicode: an astral item is one character',
    one(
      lines(
        exportList('E', ['😀', '🎉']),
        exportList('ONE', ['é', 'ü']),
        exportList('TWO', ['éé', 'üü']),
      ),
      [list('😀', '🎉'), list('é', 'ü'), list('éé', 'üü')].join('\nText.\n\n'),
    ),
  ],
  [
    'unicode: headers',
    one(
      lines(
        exportList('K', ['k1', 'k2']),
        exportList('L', ['l1', 'l2']),
        exportList('M', ['m1', 'm2']),
      ),
      [
        table(['Größe'], ['`k1`'], ['`k2`']),
        table(['a b'], ['`l1`'], ['`l2`']),
        table(['日本 語'], ['`m1`'], ['`m2`']),
      ].join('\nText.\n\n'),
    ),
  ],
  [
    'unicode: block of astral characters',
    one(
      lines(
        `export const S8 = '${'😀'.repeat(8)}';`,
        `export const S7 = '${'🎉'.repeat(7)}';`,
        "export const NB = 'spaced\\u00a0\\u3000out\\ufeffwith \\u0085 odd spaces';",
      ),
      lines(
        fence('', '😀'.repeat(8)),
        'x',
        '',
        fence('', '🎉'.repeat(7)),
        'x',
        '',
        fence('', 'spaced out with \u0085 odd spaces'),
      ),
    ),
  ],
  [
    'unicode: pages in code-unit order',
    {
      ...one(exportList('K', ['k1', 'k2', 'k3']) + '\n', list('k1', 'k2')),
      'docs/é.md': list('k1', 'k2', 'k3'),
      'docs/Z.md': list('k3', 'k2'),
      'docs/a😀.md': list('k1', 'k3'),
      'docs/a！.md': list('k1', 'k2', 'k4'),
      'docs/sub/q.md': list('k1', 'k2'),
    },
  ],
  [
    'unicode: search',
    one(
      lines(exportList('K', ['Größe', 'ΣΊΣΥΦΟΣ', 'İstanbul']), "export const TITLE = 'DÉJÀ vu';"),
      'x\n',
    ),
    [
      ['audit', '--inventory', 'GRÖSSE'],
      ['audit', '--inventory', 'größe', '--json'],
      ['audit', '--inventory', 'σίσυφος'],
      ['audit', '--inventory', 'σίσυφοσ'],
      ['audit', '--inventory', 'déjà'],
      ['audit', '--inventory', 'i̇stanbul'],
      ['audit', '--inventory', 'İ'],
    ],
  ],
];

export const crlf: AuditCase[] = [
  // CRLF pages.
  [
    'crlf: every kind',
    one(
      KINDS_SRC,
      lines(
        '# Kinds',
        '',
        '- `read`',
        '- `write`',
        '- `admin`',
        '',
        '| Method | Request path |',
        '|---|---|',
        '| GET | `/jobs` |',
        '| POST | `/projects` |',
        '',
        '```yaml',
        ...TEMPLATE.trim().split('\n'),
        '```',
        '',
        '```yaml',
        'name: demo',
        'region: eu',
        'port: 8080',
        '```',
      ).replace(/\n/g, '\r\n'),
    ),
  ],
  [
    'crlf: surface string with CRLF',
    one(
      `export const T = ${JSON.stringify(TEMPLATE.replace(/\n/g, '\r\n'))};\n`,
      fence('yaml', TEMPLATE.trim()) +
        '\nx\n\n' +
        fence('yaml', 'name: demo\nregion: eu\nport: 8080'),
    ),
  ],
];

export const boundAndFrozen: AuditCase[] = [
  // Bound and frozen pages.
  [
    'bound: whatever a directive binds',
    one(
      lines(
        exportList('KEYS', ['A', 'B']),
        exportList('P', ['/a', '/b']),
        `export const SHAPE = ${JSON.stringify(TEMPLATE)};`,
        'export const N = 3;',
      ),
      lines(
        'Keys: <!-- fact: KEYS -->',
        '- `A`',
        '- `B`',
        '',
        '<!-- fact column=Path: P -->',
        '',
        '| Path |',
        '|---|',
        '| `/a` |',
        '| `/b` |',
        '',
        '<!-- fact: SHAPE -->',
        '```yaml',
        ...TEMPLATE.trim().split('\n'),
        '```',
        '',
        '| limit | value |',
        '|---|---|',
        '| `/a` | `3` <!-- fact: N --> |',
        '| `/b` | x |',
        '',
        'Free:',
        '',
        '- `A`',
        '- `B`',
      ),
    ),
  ],
  [
    'bound: malformed, unclosed and ordinary comments',
    one(
      lines(exportList('KEYS', ['A', 'B', 'C']), exportList('P', ['/a', '/b'])),
      lines(
        '<!-- fact KEYS -->',
        '- `A`',
        '- `B`',
        '',
        '<!-- just a note -->',
        '- `A`',
        '- `C`',
        '',
        'Trailing <!-- facts: KEYS --> text',
        '',
        '- `B`',
        '- `C`',
        '',
        '<!-- cases: runner -->',
        '- `A`',
        '- `B`',
        '- `C`',
        '',
        '| Path | Note |',
        '|---|---|',
        '| `/a` | open <!-- fact: P |',
        '| `/b` | x |',
      ),
    ),
  ],
  [
    'bound: examples are not blocks',
    one(
      `export const T = ${JSON.stringify(TEMPLATE)};\n`,
      lines(
        fence('yaml check', TEMPLATE.trim()),
        'x',
        '',
        fence('yaml precompile', TEMPLATE.trim()),
        'x',
        '',
        fence('yaml title=compile-me', TEMPLATE.trim()),
        'x',
        '',
        fence('yaml checked', TEMPLATE.trim()),
      ),
    ),
  ],
  [
    'bound: lists and blocks inside other blocks',
    one(
      lines(exportList('K', ['k1', 'k2', 'k3']), `export const T = ${JSON.stringify(TEMPLATE)};`),
      lines(
        '> - `k1`',
        '> - `k2`',
        '',
        '1. Step',
        '',
        '   - `k1`',
        '   - `k3`',
        '',
        '   ```yaml',
        ...TEMPLATE.trim()
          .split('\n')
          .map((l) => '   ' + l),
        '   ```',
        '',
        '- outer',
        '',
        '  | K |',
        '  |---|',
        '  | `k2` |',
        '  | `k3` |',
        '',
        '    name: demo',
        '    region: eu',
        '    port: 8080',
        '    log: info',
      ),
    ),
  ],
  [
    'frozen: by glob and by frontmatter',
    {
      ...one(exportList('K', ['k1', 'k2', 'k3']) + '\n', list('k1', 'k2', 'k3')),
      'docattest.config.json': config({ frozen: ['docs/old/**'] }),
      'docs/old/a.md': list('k1', 'k2'),
      'docs/cold.md': '---\ndocattest: false\n---\n' + list('k1', 'k2'),
      'docs/warm.md': '---\ndocattest: true\n---\n' + list('k2', 'k3'),
    },
  ],
  [
    'living: sources are not pages',
    {
      ...one(exportList('K', ['k1', 'k2', 'k3']) + '\n', list('k1', 'k2')),
      'docattest.config.json': config({ sources: ['src/**/*.ts'] }),
      'src/math.ts':
        '/**\n * Adds.\n * - `k1`\n * - `k2`\n * @example\n * ```ts\n * // check\n * import { K } from "fixture";\n * assert.ok(K);\n * ```\n */\nexport function add2(a: number) { return a; }\n',
    },
  ],
];

export const columns: AuditCase[] = [
  // Column names.
  [
    'columns: names a directive can and cannot read',
    one(
      lines(
        ...['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'].map((n) => exportList(n, [`${n}1`, `${n}2`])),
      ),
      [
        table(['a:b'], ['`A1`'], ['`A2`']),
        table(['say "x"'], ['`B1`'], ['`B2`']),
        table([`it's "x"`], ['`C1`'], ['`C2`']),
        table(["it's"], ['`D1`'], ['`D2`']),
        table(['Dup', 'Dup'], ['x', '`E1`'], ['y', '`E2`']),
        table(['Dup', 'Dup'], ['`F1`', 'x'], ['`F2`', 'y']),
        table(['', 'Named'], ['`G1`', 'x'], ['`G2`', 'x']),
        table(['`code` head'], ['`H1`'], ['`H2`']),
      ].join('\nText.\n\n'),
    ),
  ],
  [
    'columns: an arrow in a header',
    one(exportList('A', ['A1', 'A2']) + '\n', table(['a --> b'], ['`A1`'], ['`A2`'])),
  ],
  [
    'columns: ragged rows',
    one(
      lines(exportList('A', ['A1', 'A2', 'A3']), exportList('B', ['B1', 'B2'])),
      lines(
        '| Left | Right |',
        '|---|---|',
        '| `A1` | `B1` |',
        '| `A2` |',
        '| `A3` | `B2` | extra |',
      ),
    ),
  ],
];
