import type { Edit, Switch } from '../src/playground/recordingFormat.ts';

/** A command run in the project. */
export interface Command {
  /** What the playground's stop (../src/playground/stops.ts) calls it. */
  readonly id: string;
  readonly program: 'docattest' | 'git';
  readonly args: readonly string[];
  /** JSON sent on standard input, as an agent sends its event to a hook. */
  readonly stdin?: Readonly<Record<string, unknown>>;
}

/** A stop of the playground: its commands are recorded for every setting of its switches. */
export interface Stop {
  readonly id: string;
  /** Edits already made when the visitor arrives. */
  readonly given: readonly Edit[];
  readonly switches: readonly Switch[];
  readonly commands: readonly Command[];
  /** Files to read back after the commands, because a command wrote them. */
  readonly capture: readonly string[];
}

const docattest = (id: string, ...args: string[]): Command => ({ id, program: 'docattest', args });

const CHECK = docattest('check', 'check');
const LIMIT_RAISED: Edit = {
  file: 'src/index.ts',
  find: 'maxAttempts: 3',
  replace: 'maxAttempts: 5',
};
const LIMIT_IN_CODE: Switch = {
  file: 'src/index.ts',
  options: [{ text: 'maxAttempts: 3' }, { text: 'maxAttempts: 5' }],
};
const LIMIT_ON_PAGE: Switch = {
  file: 'docs/retries.md',
  options: [{ text: 'at most `3` times' }, { text: 'at most `5` times' }],
};

export const STOPS: readonly Stop[] = [
  { id: 'meet', given: [], switches: [], commands: [CHECK], capture: [] },
  {
    id: 'example',
    given: [],
    switches: [
      {
        file: 'src/index.ts',
        options: [
          { text: 'return await call();' },
          { text: 'await call(); return undefined as T;' },
        ],
      },
    ],
    commands: [CHECK],
    capture: [],
  },
  {
    id: 'fact',
    given: [],
    switches: [LIMIT_IN_CODE, LIMIT_ON_PAGE],
    commands: [CHECK],
    capture: [],
  },
  {
    id: 'cases',
    given: [],
    switches: [
      {
        file: 'src/index.ts',
        options: [
          { text: 'RETRY.backoffMs * 2 ** (attempt - 1)' },
          { text: 'RETRY.backoffMs * attempt' },
        ],
      },
      { file: 'docs/retries.md', options: [{ text: '| 800    |' }, { text: '| 600    |' }] },
    ],
    commands: [CHECK],
    capture: [],
  },
  {
    id: 'citation',
    given: [],
    switches: [
      {
        file: 'test/retry.test.ts',
        options: [{ text: "it('throws the last error'" }, { text: "it('rethrows'" }],
      },
      {
        file: 'docs/retries.md',
        options: [{ text: '> "throws the last error" -->' }, { text: '> "rethrows" -->' }],
      },
    ],
    commands: [CHECK],
    capture: [],
  },
  {
    id: 'cheat',
    given: [],
    switches: [
      LIMIT_IN_CODE,
      {
        file: 'docs/retries.md',
        options: [
          { text: ' <!-- fact: RETRY.maxAttempts -->' },
          { text: '', shown: 'proof deleted' },
        ],
      },
    ],
    commands: [CHECK, docattest('check-base', 'check', '--base', 'HEAD')],
    capture: [],
  },
  {
    id: 'find',
    given: [],
    switches: [],
    commands: [
      docattest('where', 'where', 'RETRY'),
      docattest('index', 'where'),
      docattest('uncovered', 'uncovered'),
      docattest('inventory', 'audit', '--inventory', 'REGION'),
      docattest('audit', 'audit'),
    ],
    capture: [],
  },
  {
    id: 'impact',
    given: [],
    switches: [LIMIT_IN_CODE],
    commands: [
      docattest('impact', 'impact'),
      docattest('impact-file', 'impact', '--file', 'src/index.ts'),
    ],
    capture: [],
  },
  {
    id: 'git-hook',
    given: [LIMIT_RAISED],
    switches: [LIMIT_ON_PAGE],
    commands: [
      docattest('install', 'init', '--git-hook'),
      { id: 'commit', program: 'git', args: ['commit', '-am', 'Retry five times'] },
    ],
    capture: ['.git/hooks/pre-commit'],
  },
  {
    id: 'agent',
    given: [LIMIT_RAISED],
    switches: [LIMIT_ON_PAGE],
    commands: [
      docattest('agents-install', 'init', '--agents'),
      docattest('hooks-install', 'init', '--hooks'),
      docattest('skill-install', 'init', '--skill'),
      {
        id: 'impact-hook',
        program: 'docattest',
        args: ['impact', '--hook'],
        stdin: { tool_input: { file_path: 'src/index.ts' } },
      },
      {
        id: 'check-hook',
        program: 'docattest',
        args: ['check', '--hook'],
        stdin: { stop_hook_active: false },
      },
    ],
    capture: ['AGENTS.md', '.claude/settings.json'],
  },
];
