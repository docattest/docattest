// Agents other than the default, shipped or described by a file (`hooks install`, both hook
// modes, `skill install`), and `agents install`.
import { Buffer } from 'node:buffer';
import {
  acme,
  acmeFiles,
  badAgents,
  base,
  BEGIN,
  END,
  event,
  failing,
  hook,
  inRepo,
  parseChanged,
  passing,
  stop,
} from './fixtures.ts';
import type { ImpactCase } from './types.ts';

export const otherAgents: ImpactCase[] = [
  // -------------------------------------------------------------------- other agents
  ...['gemini', 'cursor', './tools/acme.json', 'tools/acme.json', '__ROOT__/tools/acme.json'].map(
    (agent) => ({
      name: `hooks install: --agent ${agent}`,
      files: acmeFiles,
      steps: [
        ['hooks', 'install', '--agent', agent],
        ['hooks', 'install', '--agent', agent],
        {
          args: ['hooks', 'install', '--agent', agent],
          env: { DOCATTEST_COMMAND: '{root}/.venv/bin/docattest' },
        },
      ],
    }),
  ),
  {
    name: 'hooks install: --agent into settings that are there',
    files: {
      ...acmeFiles,
      '.gemini/settings.json': JSON.stringify({
        theme: 'x',
        hooks: {
          AfterTool: [{ matcher: 'run_shell_command', hooks: [hook('echo hi')] }],
          AfterAgent: [{ hooks: [hook('docattest check')] }],
        },
      }),
      '.cursor/hooks.json': JSON.stringify({
        hooks: {
          stop: [{ command: './audit.sh', loop_limit: 10 }],
          afterFileEdit: [{ command: './format.sh' }],
        },
        version: 1,
      }),
      'acme/config.json': JSON.stringify({
        automation: {
          'file.saved': [{ exec: 'lint' }, { exec: 'cd x && docattest impact' }],
          'turn.ending': null,
        },
        schema: 2,
      }),
    },
    steps: [
      ['hooks', 'install', '--agent', 'gemini'],
      ['hooks', 'install', '--agent', 'cursor'],
      ['hooks', 'install', '--agent', './tools/acme.json'],
      ['hooks', 'install'],
    ],
  },
  {
    name: 'hooks install: --agent into settings of another shape',
    files: {
      ...acmeFiles,
      '.cursor/hooks.json': JSON.stringify({
        version: 1,
        hooks: { stop: [{ hooks: [hook('docattest check --hook')] }, 'docattest check', null] },
      }),
      'acme/config.json': JSON.stringify({ automation: [] }),
      '.gemini/settings.json': JSON.stringify({
        hooks: { AfterTool: [{ command: 'docattest impact' }], AfterAgent: 'x' },
      }),
    },
    steps: [
      ['hooks', 'install', '--agent', 'cursor'],
      ['hooks', 'install', '--agent', './tools/acme.json'],
      ['hooks', 'install', '--agent', 'gemini'],
    ],
  },
  {
    name: 'hooks install: the command the launcher gives',
    files: acmeFiles,
    steps: [
      { args: ['hooks', 'install'], env: { DOCATTEST_COMMAND: '{root}/.venv/bin/docattest' } },
      {
        args: ['hooks', 'install', '--agent', 'cursor'],
        env: { DOCATTEST_COMMAND: "  '/my env/bin/python' -m docattest  " },
      },
      {
        args: ['hooks', 'install', '--agent', 'gemini'],
        env: { DOCATTEST_COMMAND: 'pnpm exec docattest' },
      },
      {
        args: ['hooks', 'install', '--agent', './tools/acme.json'],
        env: { DOCATTEST_COMMAND: 'a\nb "c"' },
      },
      { args: ['agents', 'install'], env: { DOCATTEST_COMMAND: 'x\ry' } },
    ],
  },
  {
    name: 'hooks install: a descriptor outside the project, and one whose path needs quoting',
    files: { ...acmeFiles, "my tools/it's acme.json": acme },
    home: { 'acme.json': acme },
    steps: [
      ['hooks', 'install', '--agent', '__HOME__/acme.json'],
      ['hooks', 'install', '--agent', '../home/acme.json', '--root', 'tools'],
      ['hooks', 'install', '--agent', "my tools/it's acme.json", '--root', 'docs'],
    ],
  },
  {
    name: 'hooks install: agents nobody ships and descriptors that are wrong',
    files: {
      ...acmeFiles,
      ...Object.fromEntries(badAgents.map((text, k) => [`bad/${k}.json`, text])),
      'bad/dir.json/x': '',
    },
    steps: [
      ['hooks', 'install', '--agent', 'nobody'],
      ['hooks', 'install', '--agent', 'index'],
      ['hooks', 'install', '--agent', ''],
      ['hooks', 'install', '--agent', 'Claude'],
      ['hooks', 'install', '--agent', './gone.json'],
      ['hooks', 'install', '--agent', 'bad/dir.json'],
      ['hooks', 'install', '--agent'],
      ...badAgents.map((_, k) => ['hooks', 'install', '--agent', `bad/${k}.json`]),
      {
        args: ['impact', '--hook', '--agent', 'bad/3.json'],
        stdin: event('__ROOT__/src/parse.ts'),
      },
      { args: ['check', '--hook', '--agent', 'nobody'], stdin: stop() },
      ['skill', 'install', '--agent', 'bad/0.json'],
    ],
  },
  inRepo('impact --hook for other agents', { ...base, ...acmeFiles }, parseChanged, [
    {
      args: ['impact', '--hook', '--agent', 'gemini'],
      stdin: JSON.stringify({
        hook_event_name: 'AfterTool',
        tool_name: 'replace',
        tool_input: { file_path: '__ROOT__/src/parse.ts', old_string: 'a', new_string: 'b' },
      }),
    },
    {
      args: ['impact', '--hook', '--agent', 'gemini'],
      stdin: JSON.stringify({ tool_input: { file_path: 'src/parse.ts' } }),
    },
    {
      args: ['impact', '--hook', '--agent', 'gemini'],
      stdin: JSON.stringify({ tool_input: { file_path: '__ROOT__/docs/guide.md' } }),
    },
    {
      args: ['impact', '--hook', '--agent', 'gemini'],
      stdin: JSON.stringify({ file_path: '__ROOT__/src/parse.ts' }),
    },
    {
      args: ['impact', '--hook', '--agent', 'cursor'],
      stdin: JSON.stringify({ file_path: '__ROOT__/src/parse.ts', edits: [] }),
    },
    {
      args: ['impact', '--hook', '--agent', 'tools/acme.json'],
      stdin: JSON.stringify({ change: { target: { path: '__ROOT__/src/parse.ts' } } }),
    },
    {
      args: ['impact', '--hook', '--agent', '../tools/acme.json'],
      cwd: 'docs',
      stdin: JSON.stringify({ change: { target: { path: '../src/parse.ts' } } }),
    },
    {
      args: ['impact', '--hook', '--agent', 'tools/acme.json'],
      stdin: event('__ROOT__/src/parse.ts'),
    },
    {
      args: ['impact', '--hook', '--agent', 'tools/acme.json'],
      stdin: JSON.stringify({ change: { target: 'src/parse.ts' } }),
    },
    {
      args: ['impact', '--hook', '--agent', 'tools/acme.json'],
      stdin: JSON.stringify({ change: { target: { path: '__ROOT__/docs/guide.md' } } }),
    },
    {
      args: ['impact', '--hook', '--agent', 'tools/fixed.json'],
      stdin: JSON.stringify({ path: 'src/parse.ts' }),
    },
  ]),
  {
    name: 'check --hook for other agents',
    files: { ...failing, ...acmeFiles },
    steps: [
      {
        args: ['check', '--hook', '--agent', 'gemini'],
        stdin: JSON.stringify({
          hook_event_name: 'AfterAgent',
          prompt: 'p',
          prompt_response: 'r',
          stop_hook_active: false,
        }),
      },
      {
        args: ['check', '--hook', '--agent', 'gemini'],
        stdin: JSON.stringify({ stop_hook_active: true }),
      },
      {
        args: ['check', '--hook', '--agent', 'cursor'],
        stdin: JSON.stringify({ status: 'completed', loop_count: 0 }),
      },
      {
        args: ['check', '--hook', '--agent', 'cursor'],
        stdin: JSON.stringify({ status: 'completed', loop_count: 1 }),
      },
      {
        args: ['check', '--hook', '--agent', 'cursor'],
        stdin: JSON.stringify({ status: 'completed', loop_count: 3 }),
      },
      {
        args: ['check', '--hook', '--agent', 'cursor'],
        stdin: JSON.stringify({ loop_count: '1' }),
      },
      {
        args: ['check', '--hook', '--agent', 'cursor'],
        stdin: JSON.stringify({ loop_count: true }),
      },
      { args: ['check', '--hook', '--agent', 'cursor'], stdin: JSON.stringify({ loop_count: -1 }) },
      {
        args: ['check', '--hook', '--agent', 'cursor'],
        stdin: JSON.stringify({ loop_count: 0.5 }),
      },
      { args: ['check', '--hook', '--agent', 'cursor'], stdin: '' },
      { args: ['check', '--hook'], stdin: stop({ stop_hook_active: 1 }) },
      { args: ['check', '--hook'], stdin: stop({ stop_hook_active: 0 }) },
      { args: ['check', '--hook', '--agent', 'claude'], stdin: stop() },
      {
        args: ['check', '--hook', '--agent', 'tools/acme.json'],
        stdin: JSON.stringify({ turn: { retries: 0 } }),
      },
      {
        args: ['check', '--hook', '--agent', 'tools/acme.json'],
        stdin: JSON.stringify({ turn: { retries: 2 } }),
      },
      {
        args: ['check', '--hook', '--agent', 'tools/acme.json'],
        stdin: JSON.stringify({ turn: 2, stop_hook_active: true }),
      },
      {
        args: ['check', '--hook', '--agent', 'tools/fixed.json'],
        stdin: JSON.stringify({ never: true, again: 9 }),
      },
      { args: ['check', '--hook', '--agent', 'tools/check-only.json'], stdin: '{}' },
      { args: ['impact', '--hook', '--agent', 'tools/check-only.json'], stdin: '{}' },
      { args: ['check', '--hook', '--agent', 'tools/impact-only.json'], stdin: '{}' },
      { args: ['hooks', 'install', '--agent', 'tools/check-only.json'] },
      { args: ['hooks', 'install', '--agent', 'tools/impact-only.json'] },
    ],
  },
  {
    name: 'check --hook for other agents on a passing project',
    files: { ...passing, ...acmeFiles },
    steps: [
      { args: ['check', '--hook', '--agent', 'gemini'], stdin: '{}' },
      { args: ['check', '--hook', '--agent', 'cursor'], stdin: '{}' },
      { args: ['check', '--hook', '--agent', 'tools/acme.json'], stdin: '{}' },
    ],
  },
  {
    name: 'skill install for other agents',
    files: acmeFiles,
    steps: [
      ['skill', 'install', '--agent', 'gemini'],
      ['skill', 'install', '--agent', 'cursor'],
      ['skill', 'install', '--agent', 'cursor', '--global'],
      ['skill', 'install', '--agent', 'tools/acme.json'],
      ['skill', 'install', '--agent', 'tools/acme.json'],
      ['skill', 'install', '--agent', 'tools/acme.json', '--global', '--force'],
      ['skill', 'install', '--agent', 'claude'],
      ['skill', 'install', '--agent', 'tools/check-only.json'],
      ['skill', 'install', '--agent', 'nobody'],
    ],
  },
];

export const agentsInstall: ImpactCase[] = [
  // ----------------------------------------------------------------------- agents install
  {
    name: 'agents install, then again',
    files: {},
    steps: [
      ['agents', 'install'],
      ['agents', 'install'],
      { args: ['agents', 'install'], env: { DOCATTEST_COMMAND: '{root}/.venv/bin/docattest' } },
      { args: ['agents', 'install'], env: { DOCATTEST_COMMAND: '{root}/.venv/bin/docattest' } },
    ],
  },
  ...Object.entries({
    'an empty file': '',
    'a file of the project': '# Agents\n\nUse tabs.\n',
    'a file that ends without a newline': '# Agents\n\nUse tabs.',
    'a file that ends with a blank line': '# Agents\n\n',
    'a file with CRLF line ends and bytes that are not UTF-8':
      '# Agents\r\n\r\nUse \xff\xfe tabs. é\r\n',
    'a section between other text': `# Agents\n\n${BEGIN}\nold text\n${END}\n\n## After\n\nKept.\n`,
    'a section with nothing in it, on one line': `before ${BEGIN}${END} after`,
    'a section that is the whole file': `${BEGIN}\n${END}`,
    'a section that never ends': `# A\n${BEGIN}\nold\n`,
    'an end with no beginning': `# A\n${END}\n`,
    'an end before the beginning': `${END}\n${BEGIN}\n`,
    'two sections': `${BEGIN}\n${END}\n${BEGIN}\n${END}\n`,
    'a second end': `${BEGIN}\n${END}\n${END}\n`,
    'markers in other words':
      '<!-- docattest:begin-->\n<!--docattest:end -->\n<!-- DOCATTEST:BEGIN -->\n',
  }).map(([name, text]) => ({
    name: `agents install into ${name}`,
    files: { 'AGENTS.md': Buffer.from(text, 'latin1') },
    steps: [
      ['agents', 'install'],
      ['agents', 'install'],
    ],
  })),
  {
    name: 'agents install where AGENTS.md is a directory, by --root, and usage',
    files: { 'AGENTS.md/x': '', 'sub/x': '' },
    steps: [
      ['agents', 'install'],
      ['agents', 'install', '--root', 'sub'],
      ['agents', 'install', '--root', 'gone'],
      ['agents'],
      ['agents', 'remove'],
      ['agents', 'install', 'x'],
      ['agents', 'install', '--agent', 'nobody', '--json'],
    ],
  },
];
