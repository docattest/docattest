// Several surfaces, surfaces that do not load, and suggestions evaluated again.
import { lines } from '../../lib/lines.ts';
import { config, exportList, fence, list, one, TWO_SURFACES, type AuditCase } from './fixtures.ts';

export const severalSurfaces: AuditCase[] = [
  // More than one surface.
  [
    'surfaces: the default is preferred, others are named',
    TWO_SURFACES({
      'docs/p.md': [
        list('s1', 's2', 's3'),
        list('8080', '9090'),
        list('d1', 'd2'),
        fence('', 'welcome to the server,   friend'),
      ].join('\nText.\n\n'),
    }),
  ],
  [
    'surfaces: search across surfaces',
    TWO_SURFACES({ 'docs/p.md': 'x\n' }),
    [
      ['audit', '--inventory', 'S'],
      ['audit', '--inventory', 'ports', '--json'],
      ['audit', '--inventory', 'WELCOME'],
      ['audit', '--inventory', 'server'],
    ],
  ],
  [
    'surfaces: numeric names',
    {
      ...one(
        exportList('K', ['k1', 'k2']) + '\n',
        list('k1', 'k2') + '\nText.\n\n' + list('t1', 't2') + '\nText.\n\n' + list('o1', 'o2'),
      ),
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: {
          zeta: './src/z.ts',
          2: './src/two.ts',
          default: './src/index.ts',
          1: './src/one.ts',
        },
        adapter: 'typescript',
      }),
      'src/z.ts': exportList('K', ['k1', 'k2']) + '\n',
      'src/two.ts': exportList('T', ['t1', 't2']) + '\n',
      'src/one.ts': lines(exportList('T', ['t1', 't2']), exportList('O', ['o1', 'o2'])),
    },
  ],
  [
    'surfaces: no default',
    {
      ...one('', list('k1', 'k2')),
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { b: './src/b.ts', a: './src/a.ts' },
        adapter: 'typescript',
      }),
      'src/a.ts': exportList('K', ['k1', 'k2']) + '\n',
      'src/b.ts': exportList('K', ['k1', 'k2']) + '\n',
    },
  ],
  [
    'surfaces: an external package',
    {
      ...one(exportList('K', ['k1', 'k2']) + '\n', list('k1', 'k2')),
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/index.ts', os: 'node:os' },
        adapter: 'typescript',
      }),
    },
    [
      ['audit'],
      ['audit', '--json'],
      ['audit', '--inventory', 'EOL'],
      ['audit', '--inventory', 'signals', '--json'],
    ],
  ],
];

export const failingSurfaces: AuditCase[] = [
  // Surfaces that do not load.
  [
    'failing: the second surface throws',
    TWO_SURFACES({
      'src/server.ts': 'throw new Error("server down");\n',
      'docs/p.md': list('s1', 's2', 's3'),
    }),
  ],
  [
    'failing: a surface that cannot be found',
    {
      ...one(exportList('K', ['k1', 'k2']) + '\n', list('k1', 'k2')),
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/index.ts', gone: 'no-such-package' },
        adapter: 'typescript',
      }),
    },
  ],
  [
    'failing: a surface file that does not exist',
    {
      ...one(exportList('K', ['k1', 'k2']) + '\n', list('k1', 'k2')),
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/nowhere.ts' },
        adapter: 'typescript',
      }),
    },
  ],
  [
    'failing: one cannot be found, one throws',
    {
      ...one('throw new Error("default down");\n', list('k1', 'k2')),
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/index.ts', gone: 'no-such-package' },
        adapter: 'typescript',
      }),
    },
  ],
  ['failing: a syntax error', one('export const K = [;\n', list('k1', 'k2'))],
  [
    'failing: a rejected top-level await',
    one(
      "await Promise.reject(new Error('later'));\nexport const K = ['k1', 'k2'];\n",
      list('k1', 'k2'),
    ),
  ],
  [
    'failing: a surface that never loads',
    {
      ...one('await new Promise(() => {});\nexport const K = 1;\n', list('k1', 'k2')),
      'docattest.config.json': config({ runtime: { timeoutMs: 700 } }),
    },
    [['audit'], ['audit', '--inventory', '--json']],
  ],
  ['failing: no config', { 'README.md': '# x\n' }],
  [
    'failing: no surfaces configured',
    {
      ...one('', 'x\n'),
      'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], adapter: 'typescript' }),
    },
  ],
];

export const evaluatedAgain: AuditCase[] = [
  // Suggestions are evaluated again.
  [
    'verify: a value that changes between reads',
    one(
      lines(
        'let n = 0;',
        "export const FLIP = { get items() { n += 1; return n % 2 ? ['a1', 'a2'] : ['b1', 'b2']; } };",
        'let m = 0;',
        "export const GROW = { get items() { m += 1; return ['g1', 'g2', 'g3'].slice(0, m + 1); } };",
        exportList('STABLE', ['s1', 's2']),
      ),
      [
        list('a1', 'a2'),
        list('b1', 'b2'),
        list('g1', 'g2'),
        list('g1', 'g2', 'g3'),
        list('s1', 's2'),
      ].join('\nText.\n\n'),
    ),
  ],
  [
    'verify: a setup helper named like an export',
    {
      ...one(
        lines(exportList('K', ['k1', 'k2']), exportList('helper', ['h1', 'h2'])),
        list('k1', 'k2') + '\nText.\n\n' + list('h1', 'h2'),
      ),
      'docattest.config.json': config({ setup: 'docs/docattest.setup.ts' }),
      'docs/docattest.setup.ts': 'export const helper = () => 1;\nexport const cases = {};\n',
    },
  ],
  [
    'verify: the same list on several pages',
    {
      ...one(
        exportList('K', ['k1', 'k2', 'k3']) + '\n',
        list('k1', 'k2', 'k3') +
          '\nText.\n\n' +
          list('k3', 'k2', 'k1') +
          '\nText.\n\n' +
          list('k1', 'k2'),
      ),
      'docs/q.md': list('k1', 'k2', 'k3'),
      'docs/a.md': 'x\n\n' + list('k2', 'k3'),
    },
  ],
];
