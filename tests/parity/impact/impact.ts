// `impact`, in text and JSON, on changes in a repository.
import { Buffer } from 'node:buffer';
import { renameSync, symlinkSync, unlinkSync } from 'node:fs';
import { join } from 'node:path';
import { lines } from '../../lib/lines.ts';
import {
  base,
  baseFile,
  commit,
  config,
  constChanged,
  fileOf,
  inRepo,
  parseChanged,
  project,
  repo,
  retryChanged,
} from './fixtures.ts';
import type { ImpactCase } from './types.ts';

export const impact: ImpactCase[] = [
  // ----------------------------------------------------------------------------- impact
  inRepo('impact text, proven and mentions', base, parseChanged, [
    ['impact'],
    ['impact', '--json'],
  ]),
  inRepo('impact text, one symbol', base, retryChanged, [['impact'], ['impact', '--json']]),
  inRepo('impact a constant with a fact', base, constChanged, [['impact'], ['impact', '--json']]),
  inRepo(
    'impact a manifest a fact reads, beside a symbol',
    project({
      'src/index.ts': 'export const n = 1;\n',
      'engines.json': '{ "node": ">=22.12" }\n',
      'docs/install.md': lines(
        'Node 22.12 or later. <!-- fact: manifest("engines.json").node.slice(2) -->',
        '',
        'The version is `1`. <!-- fact: n -->',
      ),
    }),
    { 'engines.json': '{ "node": ">=24" }\n', 'src/index.ts': 'export const n = 2;\n' },
    [['impact'], ['impact', '--json']],
  ),
  inRepo('impact several symbols', base, { ...parseChanged, ...retryChanged, ...constChanged }, [
    ['impact'],
    ['impact', '--json'],
  ]),
  inRepo('impact nothing changed', base, {}, [['impact'], ['impact', '--json']]),
  inRepo(
    'impact only prose changed',
    base,
    { 'docs/guide.md': '# Guide\n\nNow `parse` only.\n', 'README.md': 'new\n' },
    [['impact'], ['impact', '--json']],
  ),
  inRepo(
    'impact only a comment changed',
    base,
    { 'src/parse.ts': `// Parses.\n${baseFile('src/parse.ts')}` },
    [['impact']],
  ),
  inRepo(
    'impact unnamed symbol',
    project({ 'src/index.ts': 'export const hidden = 1;\n', 'docs/a.md': '# A\n' }),
    { 'src/index.ts': 'export const hidden = 2;\n' },
    [['impact'], ['impact', '--json']],
  ),
  inRepo(
    'impact mention only',
    project({ 'src/index.ts': 'export const n = 1;\n', 'docs/n.md': 'Use `n`.' }),
    { 'src/index.ts': 'export const n = 2;\n' },
    [['impact'], ['impact', '--json']],
  ),
  inRepo(
    'impact untracked, deleted and renamed files',
    base,
    {
      'src/index.ts': lines(
        "export { retry, RETRY_DEFAULTS } from './retry.js';",
        "export { parse } from './parsing.js';",
        "export { Client } from './client.js';",
        "export { fresh } from './fresh.js';",
      ),
      'src/client.ts': baseFile('src/client.ts').replace('./parse.js', './parsing.js'),
      'src/fresh.ts': 'export const fresh = () => 1;\n',
      'src/backoff.ts': null,
      'src/retry.ts': baseFile('src/retry.ts').replace(
        "import { backoff } from './backoff.js';",
        'const backoff = (n: number) => n * 2;',
      ),
      'docs/fresh.md': 'Call `fresh()` now.\n',
    },
    [['impact'], ['impact', '--json']],
    {
      after: (root) => {
        renameSync(join(root, 'src/parse.ts'), join(root, 'src/parsing.ts'));
      },
    },
  ),
  inRepo(
    'impact a deleted export',
    base,
    {
      'src/index.ts': baseFile('src/index.ts').replace(
        "export { Client } from './client.js';\n",
        '',
      ),
      'src/client.ts': null,
    },
    [['impact']],
  ),
  inRepo('impact staged and unstaged', base, parseChanged, [['impact', '--json']], {
    after: (root, git, put) => {
      git('add', '-A');
      put(retryChanged);
    },
  }),
  inRepo(
    'impact ignored files are not changes',
    { ...base, '.gitignore': 'scratch/\n' },
    { ...parseChanged, 'scratch/x.ts': 'export const x = 1;\n' },
    [['impact', '--json']],
  ),
  {
    name: 'impact --base',
    files: base,
    prepare: (root, git, put) => {
      repo(git);
      git('tag', 'v1');
      put(parseChanged);
      commit(git, 'second');
      git('branch', 'second');
      put(retryChanged);
      commit(git, 'third');
      put(constChanged);
    },
    steps: [
      ['impact'],
      ['impact', '--base', 'HEAD'],
      ['impact', '--base', 'HEAD~1'],
      ['impact', '--base', 'HEAD~2', '--json'],
      ['impact', '--base=v1'],
      ['impact', '--base', 'second', '--json'],
      ['impact', '--base', 'main'],
      ['impact', '--base', 'HEAD^{tree}'],
      ['impact', '--base', 'v1:src'],
      ['impact', '--base', 'nope'],
      ['impact', '--base', ''],
      ['impact', '--base=-x'],
      ['impact', '--base=--output=x', '--json'],
      ['impact', '--base', '-'],
      ['impact', '--base', '--json'],
      ['impact', '--base'],
    ],
  },
  {
    name: 'impact outside a repository',
    files: base,
    steps: [['impact'], ['impact', '--json'], ['impact', '--base', 'main']],
  },
  {
    name: 'impact in a repository without commits',
    files: base,
    prepare: (root, git) => {
      git('init', '-q', '-b', 'main');
    },
    steps: [['impact'], ['impact', '--base', 'main']],
  },
  {
    name: 'impact without git',
    files: base,
    prepare: (root, git) => {
      repo(git);
    },
    steps: [{ args: ['impact'], env: { PATH: '/nonexistent-bin' } }],
  },
  inRepo('impact usage', base, parseChanged, [
    ['impact', 'extra'],
    ['impact', 'a', 'b', '--json'],
    ['impact', '--hook', 'x'],
  ]),
  {
    name: 'impact without a config',
    files: { 'README.md': '# x\n' },
    prepare: (root, git) => {
      repo(git);
    },
    steps: [['impact'], ['impact', '--base', 'nope']],
  },
  inRepo('impact with a broken config', { ...base }, { 'docattest.config.json': '{ "pages": [' }, [
    ['impact'],
  ]),
  {
    name: 'impact from a project below the repository root',
    files: Object.fromEntries([
      ...Object.entries(base).map(([k, v]): [string, string] => [`pkg/app/${k}`, v]),
      ['other/x.ts', 'export const x = 1;\n'],
    ]),
    prepare: (root, git, put) => {
      repo(git);
      put({
        'pkg/app/src/parse.ts': fileOf(parseChanged, 'src/parse.ts'),
        'other/x.ts': 'export const x = 2;\n',
        'pkg/app/src/new file é.ts': 'export {};\n',
      });
    },
    steps: [
      ['impact', '--root', 'pkg/app'],
      ['impact', '--root', '__ROOT__/pkg/app', '--json'],
      { args: ['impact'], cwd: 'pkg/app' },
    ],
  },
  {
    name: 'impact with many changed files of every sort',
    files: {
      ...base,
      ...Object.fromEntries(
        Array.from({ length: 30 }, (_, k) => [
          `src/gen/m ${k} é.ts`,
          `export const v${k} = ${k};\n`,
        ]),
      ),
      'assets/blob.bin': Buffer.from([0, 255, 254, 200, 10, 13, 0xe2, 0x82]),
      'assets/crlf.txt': 'a\r\nb\r\n',
      'assets/bom.json': '﻿{"a":1}',
      'src/empty.ts': '',
    },
    prepare: (root, git, put) => {
      repo(git);
      put({
        ...parseChanged,
        ...Object.fromEntries(
          Array.from({ length: 30 }, (_, k) => [
            `src/gen/m ${k} é.ts`,
            k % 3 ? `export const v${k} = ${k + 1};\n` : null,
          ]),
        ),
        'assets/blob.bin': Buffer.from([1, 2, 3]),
        'assets/crlf.txt': 'a\nb\n',
        'assets/bom.json': '{"a":2}',
        'src/empty.ts': 'export const now = 1;\n',
        'src/index.ts': `${baseFile('src/index.ts')}export { v1, v2 } from './gen/m 1 é.js';\n`,
      });
    },
    steps: [['impact'], ['impact', '--json']],
  },
  {
    name: 'impact with a changed submodule and a replaced symbolic link',
    files: { ...base, 'sub/a.ts': 'export const a = 1;\n' },
    prepare: (root, git, put) => {
      git('-C', 'sub', 'init', '-q', '-b', 'main');
      git('-C', 'sub', 'add', '-A');
      git('-C', 'sub', 'commit', '-q', '-m', 'inner');
      symlinkSync('src/parse.ts', join(root, 'link.ts'));
      symlinkSync('docs', join(root, 'pages'));
      repo(git);
      put({ 'sub/a.ts': 'export const a = 2;\n' });
      git('-C', 'sub', 'commit', '-q', '-am', 'inner 2');
      unlinkSync(join(root, 'link.ts'));
      unlinkSync(join(root, 'pages'));
      put({ ...parseChanged, 'link.ts': 'export {};\n', pages: 'now a file\n' });
    },
    steps: [['impact'], ['impact', '--json']],
  },
  inRepo(
    'impact names from Object.prototype',
    project({
      'src/index.ts': 'export function hasOwnProperty() { return 1; }\nexport const other = 1;\n',
      'docs/a.md': 'See `other` and `hasOwnProperty`.\n',
    }),
    { 'src/index.ts': 'export function hasOwnProperty() { return 2; }\nexport const other = 2;\n' },
    [['impact'], ['impact', '--json']],
  ),
  inRepo(
    'impact an attested toString',
    project({
      'src/index.ts': 'export function toString() { return "a"; }\n',
      'docs/a.md': 'Prints `a`. <!-- fact: toString() -->\n\nAlso `toString` here.\n',
    }),
    { 'src/index.ts': 'export function toString() { return "b"; }\n' },
    [['impact'], ['impact', '--json']],
  ),
  inRepo(
    'impact mentions: covered lines, lists and tables bound to directives',
    project(
      {
        'src/index.ts':
          'export const KEYS = ["a", "b"];\nexport function add(a: number, b: number) { return a + b; }\n',
        'docs/a.md': lines(
          'The `KEYS` are these:',
          '',
          '<!-- fact: KEYS -->',
          '- `a`',
          '- `b`',
          '',
          'Later `KEYS` again, and `add`.',
          '',
          'Adding with `add` works.',
          '',
          '<!-- cases: adds -->',
          '',
          '| case | a | b | sum |',
          '|---|---|---|---|',
          '| `add` small | 1 | 2 | 3 |',
          '',
          '- `add` in a list after the table',
          '',
          '<!-- fact: KEYS.length -->',
          '',
          '| `KEYS` | n |',
          '|---|---|',
          '| x | `2` |',
          '',
          'Setext `add`',
          '============',
          '',
          '[link `add`](x) and *emphasis `KEYS`* and ~~gone `add`~~.',
          '',
          '    indented `add` code',
          '',
          '```',
          'fenced `add`',
          '```',
          '',
          '<div>html `add`</div>',
          '',
          'Double ``add`` and `` add `` and ` KEYS.x ` and `add()`; `add`.',
          '',
          '[^1]: Footnote `add` here. End.',
        ),
        'docs/docattest.setup.ts':
          'import assert from "node:assert";\nimport { add } from "fixture";\nexport const cases = { adds(row: Record<string, string>) { assert.equal(add(Number(row.a), Number(row.b)), Number(row.sum)); } };\n',
      },
      config({ setup: 'docs/docattest.setup.ts' }),
    ),
    {
      'src/index.ts':
        'export const KEYS = ["a", "b", "c"];\nexport function add(a: number, b: number) { return b + a; }\n',
    },
    [['impact'], ['impact', '--json']],
  ),
  inRepo(
    'impact mentions: CRLF, a byte order mark, frozen pages and sources',
    project(
      {
        'src/index.ts':
          '/**\n * Adds.\n * @example\n * ```ts\n * // check\n * import { n } from "fixture";\n * assert.equal(n, 1);\n * ```\n */\nexport const n = 1;\n',
        'docs/crlf.md': '# `n`\r\n\r\nUse `n` here. And\r\n`n` there!\r\n',
        'docs/bom.md': '﻿Use `n` here. Größe `n`. Ende.\n',
        'docs/old/frozen.md': 'Old `n`.\n',
        'docs/front.md': '---\ndocattest: false\n---\nFrozen `n`.\n',
        'docs/title.md': '---\ntitle: n\n---\nLiving `n`? Yes.\n',
        'notes/out.md': 'Not a page: `n`.\n',
      },
      config({ frozen: ['docs/old/**'], sources: ['src/**/*.ts'] }),
    ),
    {
      'src/index.ts':
        '/**\n * Adds.\n * @example\n * ```ts\n * // check\n * import { n } from "fixture";\n * assert.equal(n, 2);\n * ```\n */\nexport const n = 2;\n',
    },
    [['impact'], ['impact', '--json']],
  ),
];
