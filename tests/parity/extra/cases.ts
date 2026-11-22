// The command line, configuration, an adapter nobody installed, pages, and the queries.
import { config, project, type ExtraCase } from './fixtures.ts';

export const cases: ExtraCase[] = [
  // The command line.
  ['help', {}, ['--help']],
  ['no command', {}, []],
  ['unknown command', {}, ['bogus']],
  ['unknown option', {}, ['check', '--nope']],
  ['ambiguous root', {}, ['check', '--root', '--json']],
  ['missing root value', {}, ['check', '--root']],
  ['no config', { 'README.md': '# x' }, ['check']],
  ['root elsewhere', {}, ['check', '--root', 'nowhere']],
  // Configuration.
  ['config not json', { 'docattest.config.json': '{ "pages": [' }, ['check']],
  ['config bad token', { 'docattest.config.json': '{ pages: 1 }' }, ['check']],
  ['config trailing', { 'docattest.config.json': '{}x' }, ['check']],
  ['config array', { 'docattest.config.json': '[]' }, ['check']],
  ['config bom', { 'docattest.config.json': '﻿{}' }, ['check']],
  [
    'config every problem',
    {
      'docattest.config.json': JSON.stringify({
        pagez: 1,
        pages: 'x',
        frozen: [1],
        surfaces: { default: '', other: { import: 'a' } },
        setup: ' ',
        snapshots: { maxAgeDays: 1.5 },
        adapter: 5,
        runtime: { timeoutMs: 0, workers: 'x', extra: 1 },
        ignoreTags: ['a b'],
      }),
    },
    ['check'],
  ],
  ['config reserved tag', project({}, config({ ignoreTags: ['check'] })), ['check']],
  [
    'config adapter missing',
    { 'docattest.config.json': JSON.stringify({ pages: [], surfaces: { default: 'x' } }) },
    ['check'],
  ],
  ['config adapter shape', project({}, config({ adapter: 'Type-Script' })), ['check']],
  // An adapter nobody installed: any name is a valid config, and both say where they looked.
  [
    'adapter not found: check',
    project({ 'docs/a.md': '`3` <!-- fact: N -->\n' }, config({ adapter: 'python' })),
    ['check'],
  ],
  ['adapter not found: index', project({}, config({ adapter: 'python' })), ['where', '--json']],
  ['adapter not found: audit', project({}, config({ adapter: 'rust_2024' })), ['audit']],
  ['adapter not found: refresh', project({}, config({ adapter: 'python' })), ['refresh']],
  [
    'config numeric surface names',
    project(
      { 'docs/a.md': '`3` <!-- fact: N -->\n' },
      JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/index.ts', 2: './src/index.ts', 1: './src/index.ts' },
        adapter: 'typescript',
      }),
    ),
    ['check', '--json'],
  ],
  // Pages.
  ['empty project', project({}), ['check']],
  ['empty project json', project({}), ['check', '--json']],
  [
    'facts',
    project({
      'docs/a.md':
        '# A\n\nAt most `3` tries. <!-- fact: N -->\n\n<!-- fact: KEYS -->\n- `a`\n- `b`\n\nWrong `4`. <!-- fact: N -->\n',
    }),
    ['check'],
  ],
  [
    'facts json',
    project({
      'docs/a.md': 'At most `3` tries. <!-- fact: N -->\n\n<!-- fact ordered: KEYS -->\n- b\n- a\n',
    }),
    ['check', '--json'],
  ],
  [
    'unclosed directives',
    project({ 'docs/a.md': 'Text <!-- fact: N\n\nmore\n\n<!-- cases: t\n' }),
    ['check'],
  ],
  [
    'table in list',
    project({
      'docs/a.md':
        '- item\n\n  <!-- fact column=v: KEYS -->\n  | v |\n  |---|\n  | `a` |\n  | `b` |\n',
    }),
    ['check'],
  ],
  [
    'fact in table cell',
    project({ 'docs/a.md': '| limit | value |\n|---|---|\n| tries | `3` <!-- fact: N --> |\n' }),
    ['check', '--json'],
  ],
  [
    'crlf',
    project({
      'docs/a.md':
        '# A\r\n\r\nAt most `3` tries. <!-- fact: N -->\r\n\r\n```ts check\r\nimport { add } from "fixture";\r\nassert.equal(add(1, 2), 3);\r\n```\r\n',
    }),
    ['check'],
  ],
  [
    'non-ascii',
    project({
      'docs/é.md': '# Größe 日本\n\nZählt `3` 😀 mal. <!-- fact: N -->\n',
      'docs/z.md': '# z\n',
      'docs/a😀.md': '# a\n',
    }),
    ['check'],
  ],
  [
    'examples',
    project({
      'docs/a.md':
        '```ts check\nimport { add } from "fixture";\nassert.equal(add(1, 2), 3);\n```\n\n```ts compile\nconst x: number = 1;\n```\n\n```ts check\nconsole.log(1);\n```\n\n```ts chekc\nx\n```\n\n```sh\necho\n```\n',
    }),
    ['check'],
  ],
  [
    'example failure',
    project({
      'docs/a.md':
        'Adds.\n\n```ts check\nimport { add } from "fixture";\nassert.equal(add(1, 2), 4);\n```\n',
    }),
    ['check', '--json'],
  ],
  [
    'citation',
    project({
      'docs/a.md':
        'A call adds. <!-- test: test/a.test.ts > "adds" -->\n\nMissing. <!-- test: test/none.test.ts > "x" -->\n',
      'test/a.test.ts': 'import { it } from "vitest";\nit("adds", () => {});\n',
    }),
    ['check'],
  ],
  [
    'frozen',
    project(
      {
        'docs/a.md': '---\ndocattest: false\n---\n`3` <!-- fact: N -->\n',
        'docs/old/b.md': '`9` <!-- fact: N -->\n',
        'docs/c.md': '# nothing\n',
      },
      config({ frozen: ['docs/old/**'] }),
    ),
    ['check'],
  ],
  [
    'page filter',
    project({ 'docs/a.md': '`3` <!-- fact: N -->\n', 'docs/b.md': '`4` <!-- fact: N -->\n' }),
    ['check', 'docs/a.md'],
  ],
  [
    'page filter glob',
    project({ 'docs/a.md': '`3` <!-- fact: N -->\n', 'docs/b.md': '`4` <!-- fact: N -->\n' }),
    ['check', 'docs/*.md', '--json'],
  ],
  ['page filter none', project({ 'docs/a.md': '`3` <!-- fact: N -->\n' }), ['check', 'docs/zz.md']],
  [
    'page filter frozen',
    project(
      { 'docs/old/a.md': '`3` <!-- fact: N -->\n', 'docs/b.md': 'x' },
      config({ frozen: ['docs/old/**'] }),
    ),
    ['check', 'docs/old/*.md'],
  ],
  [
    'quiet',
    project({ 'docs/a.md': '`3` <!-- fact: N -->\n\n`4` <!-- fact: N -->\n' }),
    ['check', '--quiet'],
  ],
  [
    'snapshots',
    project(
      {
        'docs/a.md': '`3` <!-- fact: N -->\n',
        'docs/.snapshots/ok.json': JSON.stringify({
          source: 'https://x',
          fetchedAt: '2020-01-01',
          data: { a: 1 },
        }),
        'docs/.snapshots/bad.json': '{',
        'docs/.snapshots/shape.json': '{"source":1}',
        'docs/.snapshots/date.json': '{"source":"s","fetchedAt":"nope","data":1}',
      },
      config({ snapshots: { dir: 'docs/.snapshots', maxAgeDays: 30 } }),
    ),
    ['check'],
  ],
  [
    'positionless literal',
    project({ 'docs/a.md': "See 'www.example.com' for more.\n" }),
    ['where', '--json'],
  ],
  [
    'doc comments',
    project(
      {
        'docs/a.md': '# A\n',
        'src/math.ts':
          '/**\n * Adds.\n * @example\n * ```ts\n * // check\n * import { add } from "fixture";\n * assert.equal(add(1, 2), 3);\n * ```\n */\nexport function add2(a: number) { return a; }\n',
      },
      config({ sources: ['src/**/*.ts'] }),
    ),
    ['check'],
  ],
  [
    'cases table',
    project(
      {
        'docs/a.md':
          'Adding works.\n\n<!-- cases: adds -->\n| case | a | b | sum |\n|---|---|---|---|\n| small | 1 | 2 | 3 |\n| wrong | 1 | 1 | 3 |\n\n<!-- cases: nope -->\n| case |\n|---|\n| x |\n',
        'docs/docattest.setup.ts':
          'import assert from "node:assert";\nimport { add } from "fixture";\nexport const cases = { adds(row: Record<string, string>) { assert.equal(add(Number(row.a), Number(row.b)), Number(row.sum)); } };\n',
      },
      config({ setup: 'docs/docattest.setup.ts' }),
    ),
    ['check', '--json'],
  ],
  [
    'unresolved surface',
    project(
      { 'docs/a.md': '`3` <!-- fact @gone: N -->\n' },
      JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/index.ts', gone: 'no-such-package' },
        adapter: 'typescript',
      }),
    ),
    ['check'],
  ],
  [
    'network tag',
    project({
      'docs/a.md': '```ts check network\nassert.ok(1);\n```\n\n```ts compile network\nx\n```\n',
    }),
    ['check'],
  ],
  [
    'absolute page path',
    project({ 'docs/a.md': '`3` <!-- fact: N -->\n', 'docs/b.md': '`4` <!-- fact: N -->\n' }),
    ['check', '__ROOT__/docs/a.md'],
  ],
  [
    'text and column facts',
    project({
      'docs/a.md':
        '<!-- fact: KEYS.join("\\n") -->\n```\na\nb\n```\n\n<!-- fact column=k: KEYS -->\n\n| k | v |\n|---|---|\n| `a` | 1 |\n| `c` | 2 |\n\nBad <!-- fact: nope( -->\n',
    }),
    ['check'],
  ],
  // Queries.
  [
    'index',
    project({
      'docs/a.md':
        '# A\n\nAt most `3` tries. <!-- fact: N -->\n\n```ts check\nimport { add, N } from "fixture";\nassert.equal(add(N, 2), 5);\n```\n',
    }),
    ['where'],
  ],
  [
    'index json',
    project({ 'docs/a.md': 'At most `3` tries. <!-- fact: N -->\n' }),
    ['where', '--json'],
  ],
  ['where', project({ 'docs/a.md': 'At most `3` tries. <!-- fact: N -->\n' }), ['where', 'N']],
  [
    'where json',
    project({ 'docs/a.md': 'At most `3` tries. <!-- fact: N -->\n' }),
    ['where', 'N', '--json'],
  ],
  ['where nothing', project({ 'docs/a.md': '# A\n' }), ['where', 'nope']],
  ['where toString', project({ 'docs/a.md': '# A\n' }), ['where', 'toString']],
  ['where usage', project({}), ['where', 'a', 'b']],
  ['uncovered', project({ 'docs/a.md': 'At most `3` tries. <!-- fact: N -->\n' }), ['uncovered']],
  ['uncovered json', project({ 'docs/a.md': '# A\n' }), ['uncovered', '--json']],
  ['uncovered surface flag', project({ 'docs/a.md': '# A\n' }), ['uncovered', '--surface']],
  [
    'uncovered unknown surface',
    project({ 'docs/a.md': '# A\n' }),
    ['uncovered', '--surface', 'nope'],
  ],
];
