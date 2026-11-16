// `impact --file`, and finding the project root from wherever a command runs.
import { base, baseFile, event, failing, inRepo, nested, parseChanged, stop } from './fixtures.ts';
import type { ImpactCase } from './types.ts';

export const impactFile: ImpactCase[] = [
  // ------------------------------------------------------------------------ impact --file
  inRepo('impact --file after an edit that affects exports', base, parseChanged, [
    ['impact', '--file', 'src/parse.ts'],
    ['impact', '--file', '__ROOT__/src/parse.ts'],
    ['impact', '--file', './src/../src/parse.ts', '--json'],
    ['impact', '--file', '__ROOT__/src/client.ts'],
    ['impact', '--file=src/parse.ts', '--base', 'HEAD'],
    ['impact', '--file', 'src/parse.ts', '--base', 'nope'],
    { args: ['impact', '--file', 'parse.ts'], cwd: 'src' },
    { args: ['impact', '--file', '../src/parse.ts', '--json'], cwd: 'docs' },
    { args: ['impact', '--file', 'src/parse.ts', '--root', '..'], cwd: 'docs' },
    {
      args: ['impact', '--file', 'src/parse.ts', '--hook'],
      stdin: event('__ROOT__/docs/guide.md'),
    },
  ]),
  inRepo('impact --file stays silent', base, parseChanged, [
    ['impact', '--file', 'docs/guide.md'],
    ['impact', '--file', 'docs/guide.md', '--json'],
    ['impact', '--file', 'package.json'],
    ['impact', '--file', 'node_modules/x/index.ts'],
    ['impact', '--file', 'src/types.d.ts'],
    ['impact', '--file', '/elsewhere/a.ts'],
    ['impact', '--file', '../elsewhere/a.ts'],
    ['impact', '--file', ''],
    ['impact', '--file', '.'],
    ['impact', '--file', 'docs/guide.md', '--base', 'nope'],
  ]),
  inRepo(
    'impact --file when the edit affects nothing',
    base,
    { 'src/parse.ts': `// note\n${baseFile('src/parse.ts')}` },
    [
      ['impact', '--file', 'src/parse.ts'],
      ['impact', '--file', 'src/parse.ts', '--json'],
    ],
  ),
  {
    name: 'impact --file outside a repository, without a config, and its usage',
    files: base,
    steps: [
      ['impact', '--file', 'src/parse.ts'],
      ['impact', '--file', 'docs/guide.md'],
      ['impact', '--file'],
      ['impact', '--file', 'a.ts', 'b.ts'],
      { args: ['impact', '--file', '../a.ts'], cwd: 'docs' },
    ],
  },
  // The edited file names its project: the nearest config at or above it, before the one
  // the working directory is in.
  inRepo(
    'impact --file in a project inside another',
    { ...base, ...nested },
    { 'packages/inner/src/index.ts': 'export const inner = 2;\n', ...parseChanged },
    [
      ['impact', '--file', 'packages/inner/src/index.ts'],
      ['impact', '--file', 'src/parse.ts'],
      { args: ['impact', '--file', '../../src/parse.ts'], cwd: 'packages/inner' },
      { args: ['impact', '--file', 'src/index.ts'], cwd: 'packages/inner' },
      ['impact', '--file', 'packages/inner/src/index.ts', '--root', '.'],
      { args: ['impact', '--hook'], stdin: event('__ROOT__/packages/inner/src/index.ts') },
      {
        args: ['impact', '--hook', '--root', '__ROOT__'],
        stdin: event('__ROOT__/packages/inner/src/index.ts'),
      },
    ],
  ),
];

export const findingTheRoot: ImpactCase[] = [
  // ------------------------------------------------------------------ finding the root
  {
    name: 'root found above the working directory',
    files: { ...failing, 'docs/deep/er/x': '', 'src/y': '' },
    steps: [
      { args: ['check'], cwd: 'docs' },
      { args: ['check', 'docs/a.md', '--quiet'], cwd: 'docs/deep/er' },
      { args: ['where', 'N'], cwd: 'src' },
      { args: ['index'], cwd: 'docs/deep' },
      { args: ['check', '--root', '.'], cwd: 'docs' },
      { args: ['check', '--root', '..'], cwd: 'docs' },
      { args: ['check', '--hook'], cwd: 'docs/deep', stdin: stop() },
      { args: ['hooks', 'install'], cwd: 'docs/deep' },
      { args: ['skill', 'install'], cwd: 'src' },
      { args: ['agents', 'install'], cwd: 'docs' },
      { args: ['hooks', 'install', '--root', '.'], cwd: 'src' },
    ],
  },
  {
    name: 'root is the working directory when no config is above it',
    files: { 'a/b/x': '', 'docattest.config.json/x': '' },
    steps: [
      { args: ['check'], cwd: 'a/b' },
      { args: ['hooks', 'install'], cwd: 'a' },
      { args: ['agents', 'install'], cwd: 'a/b' },
    ],
  },
  {
    name: 'root of the nearest config',
    files: { ...failing, ...nested },
    steps: [
      { args: ['check'], cwd: 'packages/inner/src' },
      { args: ['check'], cwd: 'packages' },
      { args: ['uncovered'], cwd: 'packages/inner' },
    ],
  },
];
