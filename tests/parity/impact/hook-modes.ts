// The two hook modes: `impact --hook` after an edit, `check --hook` when the agent stops.
import {
  base,
  baseFile,
  config,
  event,
  failing,
  inRepo,
  parseChanged,
  passing,
  project,
  stop,
} from './fixtures.ts';
import type { ImpactCase } from './types.ts';

export const impactHook: ImpactCase[] = [
  // ------------------------------------------------------------------------ impact --hook
  inRepo('impact --hook after a source edit', base, parseChanged, [
    { args: ['impact', '--hook'], stdin: event('__ROOT__/src/parse.ts') },
    { args: ['impact', '--hook', '--json'], stdin: event('__ROOT__/src/client.ts') },
    { args: ['impact', '--hook'], stdin: event('src/parse.ts') },
    { args: ['impact', '--hook'], stdin: event('./src/../src/parse.ts') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__/lib/x.mjs') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__/a.tsx', { extra: [1, 2] }) },
    { args: ['impact', '--hook', '--base', 'HEAD'], stdin: event('__ROOT__/src/parse.ts') },
    { args: ['impact', '--hook', '--base', 'nope'], stdin: event('__ROOT__/src/parse.ts') },
  ]),
  inRepo('impact --hook stays silent', base, parseChanged, [
    { args: ['impact', '--hook'], stdin: event('__ROOT__/docs/guide.md') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__/src/types.d.ts') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__/src/types.d.mts') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__/package.json') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__/node_modules/x/index.ts') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__/src/node_modules/x.ts') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__') },
    { args: ['impact', '--hook'], stdin: event('__ROOT__/../elsewhere/a.ts') },
    { args: ['impact', '--hook'], stdin: event('/elsewhere/a.ts') },
    { args: ['impact', '--hook'], stdin: event('') },
    { args: ['impact', '--hook'], stdin: event(7) },
    { args: ['impact', '--hook'], stdin: JSON.stringify({ tool_input: 'src/parse.ts' }) },
    { args: ['impact', '--hook'], stdin: JSON.stringify({ tool_name: 'Edit' }) },
    { args: ['impact', '--hook'], stdin: '' },
    { args: ['impact', '--hook'], stdin: 'not json' },
    { args: ['impact', '--hook'], stdin: '[]' },
    { args: ['impact', '--hook'], stdin: `\uFEFF${event('__ROOT__/src/parse.ts')}` },
    { args: ['impact', '--hook', '--base', 'nope'], stdin: event('__ROOT__/docs/guide.md') },
  ]),
  inRepo(
    'impact --hook when the edit affects nothing',
    base,
    { 'src/parse.ts': `// note\n${baseFile('src/parse.ts')}` },
    [{ args: ['impact', '--hook'], stdin: event('__ROOT__/src/parse.ts') }],
  ),
  {
    name: 'impact --hook outside a repository',
    files: base,
    steps: [
      { args: ['impact', '--hook'], stdin: event('__ROOT__/src/parse.ts') },
      { args: ['impact', '--hook'], stdin: event('__ROOT__/docs/guide.md') },
    ],
  },
  {
    name: 'impact --hook without a config',
    files: { 'a.ts': 'export {};\n' },
    steps: [
      { args: ['impact', '--hook'], stdin: event('__ROOT__/a.ts') },
      { args: ['impact', '--hook'], stdin: event('__ROOT__/a.md') },
      { args: ['impact', '--hook'], stdin: event('/elsewhere/a.ts') },
      { args: ['impact', '--hook'], stdin: event('__ROOT__/node_modules/x/a.ts') },
    ],
  },
  // Which names are dependencies is the adapter's to say, and the config names the adapter:
  // inside the project the config is read, and the adapter found, whatever the file.
  {
    name: 'impact --hook with an adapter nobody installed',
    files: project({}, config({ adapter: 'nobody_installed_this' })),
    steps: [
      { args: ['impact', '--hook'], stdin: event('__ROOT__/src/a.py') },
      { args: ['impact', '--hook'], stdin: event('__ROOT__/node_modules/x/a.ts') },
      { args: ['impact', '--hook'], stdin: event('/elsewhere/a.py') },
      { args: ['check', '--hook'], stdin: stop() },
      { args: ['check', '--hook'], stdin: stop({ stop_hook_active: true }) },
      { args: ['impact'] },
    ],
  },
  inRepo(
    'impact --hook with text that needs escaping',
    project({
      'src/index.ts': 'export const n = 1;\n',
      'docs/n.md': 'Say "hi" \\ to `n`\tand <b>é😀</b>   there.\n',
    }),
    { 'src/index.ts': 'export const n = 2;\n' },
    [{ args: ['impact', '--hook'], stdin: event('__ROOT__/src/index.ts') }],
  ),
];

export const checkHook: ImpactCase[] = [
  // ------------------------------------------------------------------------- check --hook
  {
    name: 'check --hook on a failing project',
    files: failing,
    steps: [
      { args: ['check', '--hook'], stdin: stop() },
      { args: ['check', '--hook'], stdin: stop({ stop_hook_active: false }) },
      { args: ['check', '--hook'], stdin: stop({ stop_hook_active: 'true' }) },
      { args: ['check', '--hook'], stdin: stop({ stop_hook_active: true }) },
      { args: ['check', '--hook'], stdin: '' },
      { args: ['check', '--hook'], stdin: 'not json' },
      { args: ['check', '--hook'], stdin: '[true]' },
      { args: ['check', '--hook', '--json', '--quiet'], stdin: stop() },
      { args: ['check', '--hook', 'docs/nope.md'], stdin: stop() },
      { args: ['check', '--hook', '--base', 'nope'], stdin: stop() },
    ],
  },
  {
    name: 'check --hook on a passing project',
    files: passing,
    steps: [
      { args: ['check', '--hook'], stdin: stop() },
      { args: ['check', '--hook'], stdin: stop({ stop_hook_active: true }) },
    ],
  },
  {
    name: 'check --hook without a config',
    files: { 'README.md': 'x\n' },
    steps: [
      { args: ['check', '--hook'], stdin: stop() },
      { args: ['check', '--hook'], stdin: stop({ stop_hook_active: true }) },
    ],
  },
  {
    name: 'check --hook with an error result',
    files: project({
      'src/index.ts': 'export const N = 3;\n',
      'docs/a.md': 'Bad <!-- fact: nope( -->\n\n```ts check\nassert.equal(1, 2);\n```\n',
    }),
    steps: [{ args: ['check', '--hook'], stdin: stop() }],
  },
];
