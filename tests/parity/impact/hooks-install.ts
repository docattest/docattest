// `hooks install` into the default agent's settings, whatever they hold, and `hooks install --git`.
import { hook, INSTALL, repo, settings } from './fixtures.ts';
import type { ImpactCase } from './types.ts';

export const hooksInstall: ImpactCase[] = [
  // ------------------------------------------------------------------------ hooks install
  {
    name: 'hooks install: no settings file',
    files: {},
    steps: [
      ['init', '--hooks', '--no-audit'],
      ['init', '--hooks'],
    ],
  },
  {
    name: 'hooks install: no settings file, by --root',
    files: { 'sub/x': '' },
    steps: [
      ['init', '--hooks', '--no-audit', '--root', 'sub'],
      ['init', '--hooks', '--root', '__ROOT__/sub/'],
    ],
  },
  { name: 'hooks install: an empty object', files: settings('{}'), steps: INSTALL },
  {
    name: 'hooks install: other hooks and settings',
    files: settings({
      permissions: { allow: ['Bash(ls)'] },
      hooks: {
        PreToolUse: [{ matcher: 'Bash', hooks: [hook('echo pre')] }],
        PostToolUse: [{ matcher: 'Bash', hooks: [hook('echo hi')] }],
        Stop: [{ hooks: [hook('say done')] }],
      },
      model: 'x',
    }),
    steps: [...INSTALL, ...INSTALL],
  },
  {
    name: 'hooks install: already installed',
    files: settings({
      hooks: {
        PostToolUse: [{ matcher: 'Edit|Write', hooks: [hook('npx --no docattest impact --hook')] }],
        Stop: [{ hooks: [hook('npx --no docattest check --hook')] }],
      },
    }),
    steps: INSTALL,
  },
  {
    name: 'hooks install: installed in other words',
    files: settings({
      hooks: {
        PostToolUse: [
          {
            hooks: [
              hook('echo x'),
              hook('cd app && ./node_modules/.bin/docattest --root . impact --hook'),
            ],
          },
        ],
        Stop: [{ hooks: [hook('pnpm docattest-check')] }],
      },
    }),
    steps: INSTALL,
  },
  {
    name: 'hooks install: one of the two installed',
    files: settings({ hooks: { Stop: [{ hooks: [hook('docattest check')] }] } }),
    steps: INSTALL,
  },
  {
    name: 'hooks install: the other one installed',
    files: settings({ hooks: { PostToolUse: [{ hooks: [hook('docattest impact')] }] }, after: 1 }),
    steps: INSTALL,
  },
  {
    name: 'hooks install: commands that only look installed',
    files: settings({
      hooks: {
        PostToolUse: [
          {
            hooks: [
              hook('docattest check'),
              hook('impact docattest'),
              hook('docattestimpact'),
              hook('mydocattest impacts'),
              hook('docattest \\\n impact'),
              { command: 7 },
              'docattest impact',
              null,
            ],
          },
          'docattest impact',
          { hooks: 'docattest impact' },
        ],
        Stop: [
          { hooks: [{ type: 'command', comand: 'docattest check' }] },
          { command: 'docattest check' },
        ],
      },
    }),
    steps: INSTALL,
  },
  {
    name: 'hooks install: null hooks and null lists',
    files: settings('{"hooks":null,"x":1}'),
    steps: INSTALL,
  },
  {
    name: 'hooks install: a null list',
    files: settings('{"hooks":{"Stop":null,"PostToolUse":[]},"x":1}'),
    steps: INSTALL,
  },
  {
    name: 'hooks install: key order, numbers and escapes kept as written',
    files: settings(
      '{\n\t"z": 1.0, "10": 1e21, "2": -0, "a": [1.50, 1E3, 0.1e-6, 123456789012345678901234567890, true, null, [], {}, [[]], {"": ""}],\n "é": "\\u00e9\\n\\t\\u0001\\u007f \\/ \\" \\\\ 😀 \\ud83d\\ude00  ", "hooks": {"Stop": [], "1": [], "PostToolUse": [{"hooks": [], "matcher": "*"}]}, "a": "again", "__proto__": {"x": 1}, "constructor": 2 }',
    ),
    steps: INSTALL,
  },
  {
    name: 'hooks install: lone surrogates in the file',
    files: settings('{"\\ud800": "a\\udfffb\\ud83d\\ude00\\\\ud800 \\uD83Dx", "model": "\\udc00"}'),
    steps: INSTALL,
  },
  {
    name: 'hooks install: CRLF and trailing spaces in the file',
    files: settings('{\r\n  "model": "x"\r\n}\r\n  '),
    steps: INSTALL,
  },
  { name: 'hooks install: not JSON', files: settings('{ not json'), steps: INSTALL },
  { name: 'hooks install: an empty file', files: settings(''), steps: INSTALL },
  {
    name: 'hooks install: comments in the file',
    files: settings('{\n  // mine\n  "model": "x"\n}\n'),
    steps: INSTALL,
  },
  { name: 'hooks install: a trailing comma', files: settings('{"model": "x",}'), steps: INSTALL },
  { name: 'hooks install: a byte order mark', files: settings('﻿{}'), steps: INSTALL },
  { name: 'hooks install: an array', files: settings('[]'), steps: INSTALL },
  { name: 'hooks install: a string', files: settings('"x"'), steps: INSTALL },
  { name: 'hooks install: null', files: settings('null'), steps: INSTALL },
  { name: 'hooks install: hooks is a list', files: settings({ hooks: [] }), steps: INSTALL },
  { name: 'hooks install: hooks is a string', files: settings({ hooks: 'x' }), steps: INSTALL },
  {
    name: 'hooks install: PostToolUse is not a list',
    files: settings({ hooks: { PostToolUse: {} } }),
    steps: INSTALL,
  },
  {
    name: 'hooks install: Stop is not a list',
    files: settings({ hooks: { Stop: 'x' } }),
    steps: INSTALL,
  },
  {
    name: 'hooks install: settings.json is a directory',
    files: { '.claude/settings.json/x': '' },
    steps: INSTALL,
  },
  { name: 'hooks install: .claude is a file', files: { '.claude': 'x' }, steps: INSTALL },
  {
    name: 'hooks install: usage',
    files: {},
    steps: [
      ['init', '--local'],
      ['init', '--hooks', '--local', '--git-hook'],
      ['init', '--hooks', '--no-audit', '--json', '--quiet'],
    ],
  },
  {
    name: 'hooks install --local',
    files: {},
    steps: [
      ['init', '--hooks', '--local', '--no-audit'],
      ['init', '--hooks', '--local'],
      ['init', '--hooks', '--local', '--git-hook'],
      ['init', '--hooks', '--local', '--agent', 'cursor'],
    ],
  },
  {
    name: 'hooks install --local in a git repository',
    files: { 'README.md': 'hi\n' },
    prepare: (_root, git) => {
      repo(git);
    },
    steps: [['init', '--hooks', '--local', '--no-audit']],
  },
  {
    name: 'hooks install --local in a git repository with gitignore',
    files: { '.gitignore': '.claude/settings.local.json\n' },
    prepare: (_root, git) => {
      repo(git);
    },
    steps: [['init', '--hooks', '--local', '--no-audit']],
  },
];

export const hooksInstallGit: ImpactCase[] = [
  // ------------------------------------------------------------------ init --git-hook
  {
    name: 'hooks install --git: a new hook',
    files: { x: '' },
    prepare: (root, git) => {
      repo(git);
    },
    steps: [
      ['init', '--git-hook', '--no-audit'],
      ['init', '--git-hook'],
      ['init', '--git-hook', '--agent', 'cursor'],
      ['init', '--hooks'],
    ],
  },
  {
    name: 'hooks install --git: core.hooksPath, and a project below the top',
    files: { 'packages/my lib/x': '', 'packages/b/x': '' },
    prepare: (root, git) => {
      repo(git);
      git('config', 'core.hooksPath', '.githooks');
    },
    steps: [
      {
        args: ['init', '--git-hook', '--no-audit'],
        cwd: 'packages/my lib',
        env: { DOCATTEST_COMMAND: '{root}/.venv/bin/docattest' },
      },
      { args: ['init', '--git-hook', '--no-audit', '--root', 'packages/b'] },
      ['init', '--git-hook', '--no-audit'],
    ],
  },
  {
    name: 'hooks install --git: a project below the top',
    files: { 'packages/b/x': '' },
    prepare: (root, git) => {
      repo(git);
    },
    steps: [
      ['init', '--git-hook', '--no-audit', '--root', 'packages/b'],
      ['init', '--git-hook', '--no-audit'],
    ],
  },
  ...Object.entries({
    'a hook of the project': '#!/bin/sh\nnpm test\n',
    'an empty hook': '',
    'a hook that runs docattest check in other words':
      '#!/bin/sh\nnpm test\n  npx docattest check docs/a.md\n',
    'a hook that holds the line':
      '#!/bin/sh\nnpm test\r\nnpx --no docattest check --quiet || exit 1\necho done',
    'a hook that holds the line in a longer one':
      '#!/bin/sh\n npx --no docattest check --quiet || exit 1\n',
  }).map(([name, text]): ImpactCase => ({
    name: `hooks install --git: over ${name}`,
    files: { x: '' },
    prepare: (root, git, put) => {
      repo(git);
      git('config', 'core.hooksPath', 'my hooks');
      put({ 'my hooks/pre-commit': text });
    },
    steps: [
      ['init', '--git-hook', '--no-audit'],
      ['init', '--git-hook'],
    ],
  })),
  {
    name: 'hooks install --git: outside a repository',
    files: { x: '' },
    steps: [
      ['init', '--git-hook', '--no-audit'],
      ['init', '--git-hook', '--no-audit', '--root', 'gone'],
    ],
  },
  {
    name: 'hooks install --git: where the hook or its directory cannot be written',
    files: { x: '' },
    prepare: (root, git, put) => {
      repo(git);
      git('config', 'core.hooksPath', 'h');
      put({ 'h/pre-commit/x': '' });
    },
    steps: [
      ['init', '--git-hook', '--no-audit'],
      {
        args: ['init', '--git-hook'],
        env: {
          GIT_CONFIG_COUNT: '1',
          GIT_CONFIG_KEY_0: 'core.hooksPath',
          GIT_CONFIG_VALUE_0: 'x/deeper',
        },
      },
    ],
  },
];
