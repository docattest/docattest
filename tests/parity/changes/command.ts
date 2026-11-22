// The command: several changes at once and exit codes, page filters with a base, the base
// itself, and user errors.
import {
  BARE,
  change,
  CLAIM,
  config,
  EXAMPLE,
  FACT,
  JSON_AND_TEXT,
  LIST,
  page,
  PAGE,
  PROJECT,
  SRC,
  type ChangeCase,
} from './fixtures.ts';

export const several: ChangeCase[] = [
  // ----- several at once; exit codes -----
  change(
    'every kind of change',
    { 'docs/retry.md': PAGE, 'docs/k.md': page(...LIST), 'docs/z.md': page(FACT) },
    {
      'docs/retry.md': page(
        FACT.replace('LIMIT', 'Math.min(LIMIT, 3)'),
        '',
        CLAIM,
        '',
        '```ts compile',
        ...EXAMPLE.slice(1),
      ),
      'docs/k.md': page('The keys are:', '- `A`', '- `B`'),
      'docs/z.md': `---\ndocattest: false\n---\n${page(FACT)}`,
    },
  ),
  change(
    'warnings only, exit 0',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': page(FACT, '', CLAIM, '', '```ts compile', ...EXAMPLE.slice(1)) },
    [['check', '--base', 'HEAD', '--quiet']],
  ),
  change(
    'failing result and no change',
    { 'docs/retry.md': PAGE },
    { 'src/index.ts': SRC.replace('= 3', '= 4') },
  ),
  change(
    'pages sort by UTF-16 units',
    { 'docs/😀.md': page(FACT), 'docs/！.md': page(FACT), 'docs/a.md': page(FACT) },
    { 'docs/😀.md': page(BARE), 'docs/！.md': page(BARE), 'docs/a.md': page(BARE) },
  ),
];

export const pageFilters: ChangeCase[] = [
  // ----- page filters with a base -----
  change(
    'page filter',
    { 'docs/retry.md': PAGE, 'docs/k.md': page(...LIST) },
    { 'docs/k.md': page('The keys are:', '- `A`', '- `B`') },
    [
      ['check', '--base', 'HEAD', 'docs/retry.md', '--json'],
      ['check', 'docs/k.md', '--base', 'HEAD'],
      ['check', '--base', 'HEAD', 'docs/*.md', '--quiet'],
    ],
  ),
  change('page filter that matches nothing', { 'docs/retry.md': PAGE }, {}, [
    ['check', '--base', 'HEAD', 'docs/none.md'],
    ['check', '--base', 'nope', 'docs/none.md'],
  ]),
  change(
    'page filter on a page frozen in the change',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': `---\ndocattest: false\n---\n${PAGE}` },
    [['check', '--base', 'HEAD', 'docs/retry.md']],
  ),
];

export const theBase: ChangeCase[] = [
  // ----- the base -----
  {
    name: 'base without a config',
    base: { 'README.md': 'before docattest\n' },
    head: { ...PROJECT, 'docs/retry.md': PAGE },
    runs: JSON_AND_TEXT,
  },
  {
    name: 'base with a config and no pages',
    base: PROJECT,
    head: { 'docs/retry.md': PAGE },
    runs: JSON_AND_TEXT,
  },
  {
    name: 'base config not JSON',
    base: { ...PROJECT, 'docattest.config.json': '{ nope' },
    head: { 'docattest.config.json': config() },
    runs: JSON_AND_TEXT,
  },
  {
    name: 'base config invalid',
    base: {
      ...PROJECT,
      'docattest.config.json': JSON.stringify({ pages: 'docs', adapter: 'python', extra: 1 }),
    },
    head: { 'docattest.config.json': config() },
    runs: JSON_AND_TEXT,
  },
  {
    name: 'base config an array',
    base: { ...PROJECT, 'docattest.config.json': '[]' },
    head: { 'docattest.config.json': config() },
    runs: JSON_AND_TEXT,
  },
  {
    name: 'base config with an empty glob',
    base: { ...PROJECT, 'docattest.config.json': config({ pages: [''] }) },
    head: { 'docattest.config.json': config() },
    runs: JSON_AND_TEXT,
  },
  {
    name: 'base config with an empty frozen glob',
    base: { ...PROJECT, 'docattest.config.json': config({ frozen: [''] }), 'docs/retry.md': PAGE },
    head: { 'docattest.config.json': config() },
    runs: JSON_AND_TEXT,
  },
  {
    name: 'head config invalid, and an unknown ref',
    base: PROJECT,
    head: { 'docattest.config.json': '{' },
    runs: [
      ['check', '--base', 'nope'],
      ['check', '--base', 'HEAD'],
    ],
  },
  {
    name: 'base without package.json or sources',
    base: {
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/index.ts' },
        adapter: 'typescript',
      }),
      'src/index.ts': SRC,
      'docs/retry.md': PAGE.replace("'fixture'", "'docattest-fixture'"),
    },
    head: { 'docs/retry.md': page(BARE, '', CLAIM) },
    runs: JSON_AND_TEXT,
  },
  change(
    'hidden and dependency directories at the base',
    {
      'docs/.drafts/a.md': page(FACT),
      'docs/node_modules/x/readme.md': page(FACT),
      'docs/retry.md': PAGE,
    },
    { 'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE) },
  ),
  change(
    'other refs',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE) },
    [
      ['check', '--base', 'main', '--json'],
      ['check', '--base=HEAD~0'],
      ['check', '--base', 'HEAD^{tree}'],
      ['check', '--base', 'HEAD:docs/retry.md'],
      ['check', '--base', 'HEAD~1'],
    ],
  ),
  {
    name: 'an earlier commit',
    base: { ...PROJECT, 'docs/retry.md': PAGE },
    commits: [{ 'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE) }],
    head: { 'docs/retry.md': page(BARE, '', CLAIM) },
    runs: [
      ['check', '--base', 'HEAD', '--json'],
      ['check', '--base', 'HEAD~1', '--json'],
      ['check', '--base', 'HEAD~1'],
      ['check', '--base', 'HEAD~2'],
    ],
  },
  {
    name: 'project in a subdirectory',
    inside: 'packages/app',
    base: { ...PROJECT, 'docs/retry.md': PAGE },
    outside: { 'docs/retry.md': page(FACT), 'README.md': '# repo\n' },
    head: { 'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE) },
    runs: [
      ['check', '--base', 'HEAD', '--json'],
      ['check', '--base', 'HEAD'],
      ['check', '--base', 'HEAD', '--root', '.'],
      ['check', '--base', 'HEAD', '--root', '__ROOT__'],
    ],
  },
  {
    name: 'project in a subdirectory, from the repository root',
    inside: 'packages/app',
    cwd: 'repository',
    base: { ...PROJECT, 'docs/retry.md': PAGE },
    head: { 'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE) },
    runs: [
      ['check', '--base', 'HEAD', '--root', 'packages/app', '--json'],
      ['check', '--base', 'HEAD', '--root', 'packages/app/'],
    ],
  },
];

export const userErrors: ChangeCase[] = [
  // ----- user errors -----
  {
    name: 'not a git repository',
    git: false,
    base: { ...PROJECT, 'docs/retry.md': PAGE },
    head: {},
    runs: [
      ['check', '--base', 'HEAD'],
      ['check', '--base', 'HEAD', '--json'],
      ['check', '--base=-x'],
      ['check', '--base', 'HEAD', '--root', '__ROOT__/.'],
    ],
  },
  change('unknown ref', { 'docs/retry.md': PAGE }, {}, [
    ['check', '--base', 'v9'],
    ['check', '--base', 'v9', '--json'],
    ['check', '--base', 'origin/main', '--quiet'],
    ['check', '--base', 'HEAD~5'],
    ['check', '--base', 'a b'],
    ['check', '--base', '"HEAD"'],
    ['check', '--base', 'réf😀'],
  ]),
  change('a ref that starts with a dash', { 'docs/retry.md': PAGE }, {}, [
    ['check', '--base=-x'],
    ['check', '--base=--output=x', '--json'],
    ['check', '--base', '-'],
    ['check', '--base', '-x'],
    ['check', '--base=-'],
  ]),
  change('an empty ref', { 'docs/retry.md': PAGE }, {}, [
    ['check', '--base='],
    ['check', '--base', ''],
    ['check', '--base', '  '],
    ['check', '--base'],
    ['check', '--base', '--json'],
  ]),
  change(
    'base given twice, and with other commands',
    { 'docs/retry.md': PAGE },
    { 'docs/retry.md': page(BARE, '', CLAIM, '', ...EXAMPLE) },
    [
      ['check', '--base', 'nope', '--base', 'HEAD'],
      ['where', '--base', 'nope'],
      ['where', 'LIMIT', '--base', 'HEAD'],
      ['uncovered', '--base', 'nope', '--json'],
    ],
  ),
  {
    name: 'no config at the head',
    base: { 'README.md': '# x\n' },
    head: {},
    runs: [
      ['check', '--base', 'HEAD'],
      ['check', '--base', 'nope'],
      ['check', '--base', 'HEAD', '--root', 'nowhere'],
    ],
  },
  {
    name: 'git is not installed',
    env: { PATH: '/nonexistent' },
    base: { ...PROJECT, 'docs/retry.md': PAGE },
    head: {},
    runs: [
      ['check', '--base', 'HEAD'],
      ['check', '--base=-x', '--json'],
    ],
  },
];
