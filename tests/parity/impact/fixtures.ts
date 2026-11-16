// The projects, agent descriptors and helpers the impact cases (tests/parity/impact/) are
// built from.
import { lines } from '../../lib/lines.ts';
import type { ProjectFiles } from '../../lib/project.ts';
import { URL_BASE } from './server.ts';
import type { CaseStep, Git, ImpactCase, Prepare, TextFiles } from './types.ts';

/** Configuration fields, as a config file or an agent descriptor holds them. */
type Fields = Readonly<Record<string, unknown>>;

export const pkg = JSON.stringify({ name: 'fixture', version: '1.0.0', type: 'module' });
/** The config of a fixture project, with `extra` fields. */
export const config = (extra: Fields = {}): string =>
  JSON.stringify({
    pages: ['docs/**/*.md'],
    surfaces: { default: './src/index.ts' },
    adapter: 'typescript',
    ...extra,
  });
/** A fixture project: package.json, the config `cfg`, and `files`. */
export const project = (files: TextFiles, cfg = config()): TextFiles => ({
  'package.json': pkg,
  'docattest.config.json': cfg,
  ...files,
});

export const src: TextFiles = {
  'src/index.ts': lines(
    "export { retry, RETRY_DEFAULTS } from './retry.js';",
    "export { parse } from './parse.js';",
    "export { Client } from './client.js';",
  ),
  'src/retry.ts': lines(
    "import { backoff } from './backoff.js';",
    'export const RETRY_DEFAULTS = { maxAttempts: 3 };',
    'export async function retry<T>(fn: () => Promise<T>): Promise<T> {',
    '  for (let attempt = 1; ; attempt++) {',
    '    try { return await fn(); } catch (err) {',
    '      if (attempt >= RETRY_DEFAULTS.maxAttempts) throw err;',
    '      await new Promise((done) => setTimeout(done, backoff(attempt)));',
    '    }',
    '  }',
    '}',
  ),
  'src/backoff.ts': lines(
    'export function backoff(attempt: number): number {',
    '  return attempt * 2;',
    '}',
  ),
  'src/parse.ts': lines(
    'export function parse(text: string): number {',
    '  return Number(text);',
    '}',
  ),
  'src/client.ts': lines(
    "import { parse } from './parse.js';",
    'export class Client {',
    '  read(text: string) { return parse(text); }',
    '}',
  ),
};
export const pages: TextFiles = {
  'docs/retry.md': lines(
    '# Retry with `retry`',
    '',
    'A call is tried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->',
    '',
    'A successful call through `retry` returns its value unchanged.',
    '',
    '```ts check',
    "import { retry } from 'fixture';",
    "assert.equal(await retry(async () => 'ok'), 'ok');",
    '```',
    '',
    'Wrap it: `await retry(fn)`. Never `retry-after`! See `retry.ts` and `RETRY_DEFAULTS.maxAttempts`.',
  ),
  'docs/parse.md': lines(
    '# Parse',
    '',
    'Text becomes a number.',
    '',
    '```ts check',
    "import { parse } from 'fixture';",
    "assert.equal(parse('4'), 4);",
    '```',
  ),
  'docs/guide.md': lines(
    '# Guide',
    '',
    'Start here. Read the input with `parse` before you retry anything. Then stop.',
    '',
    '- [x] `parse` first',
    '- [ ] then `new Client()` reads; `new Client-x` does not',
    '* **`retry`** wraps',
    '',
    '| name | use |',
    '|------|-----|',
    '| `parse` | text. to number |',
    '| x | see `Client.read` or `await parse(x)`? Yes. |',
    '',
    '> Größe zählt 😀. Erst `parse(a.b)`. Dann é!',
    '',
    '## `Client` and `parse`',
    '',
    'Same line `parse` and `parse` twice.',
    'Next line `parse`',
    'in one paragraph. Done.',
  ),
};
export const base = project({ ...src, ...pages });
/** The text of the file `path` of a fixture's files. */
export function fileOf(files: TextFiles, path: string): string {
  const text = files[path];
  if (text === undefined) throw new Error(`fixture: no ${path}`);
  return text;
}
/** The text of one of `base`'s files. */
export const baseFile = (path: string): string => fileOf(base, path);
/** One file of `base`, with `from` replaced by `to`. */
export const edit = (file: string, from: string, to: string): TextFiles => ({
  [file]: baseFile(file).replace(from, to),
});
export const parseChanged = edit('src/parse.ts', 'Number(text)', 'Number.parseInt(text, 10)');
export const retryChanged = edit('src/backoff.ts', 'attempt * 2', 'attempt * 200');
export const constChanged = edit('src/retry.ts', 'maxAttempts: 3', 'maxAttempts: 5');

/** Commit everything as the base. */
export const commit = (git: Git, message = 'base'): void => {
  git('add', '-A');
  git('commit', '-q', '-m', message, '--allow-empty');
};
/** A repository on its main branch, everything committed. */
export const repo = (git: Git): void => {
  git('init', '-q', '-b', 'main');
  commit(git);
};
/** A case in a repository: `files` committed, then `head` applied to the working tree. */
export const inRepo = (
  name: string,
  files: ProjectFiles,
  head: ProjectFiles,
  steps: readonly CaseStep[],
  more: { after?: Prepare } = {},
): ImpactCase => ({
  name,
  files,
  prepare: (root, git, put) => {
    repo(git);
    put(head);
    more.after?.(root, git, put);
  },
  steps,
});
/** A PostToolUse event for an edit of `file`, as the default agent sends it. */
export const event = (file: string | number, extra: Fields = {}): string =>
  JSON.stringify({
    hook_event_name: 'PostToolUse',
    tool_name: 'Edit',
    tool_input: { file_path: file },
    ...extra,
  });
/** A Stop event, as the default agent sends it. */
export const stop = (extra: Fields = {}): string =>
  JSON.stringify({ hook_event_name: 'Stop', ...extra });

export const failing = project({
  'src/index.ts': 'export const N = 3;\n',
  'docs/a.md': 'At most `4` tries. <!-- fact: N -->\n\nAnd `3`. <!-- fact: N -->\n',
});
export const passing = project({
  'src/index.ts': 'export const N = 3;\n',
  'docs/a.md': 'At most `3` tries. <!-- fact: N -->\n',
});

/** One hook of the default agent's settings. */
export const hook = (command: string): { type: string; command: string } => ({
  type: 'command',
  command,
});
/** The default agent's settings file, holding `value` (as it is, when it is text). */
export const settings = (value: string | Fields): TextFiles => ({
  '.claude/settings.json': typeof value === 'string' ? value : JSON.stringify(value),
});
export const INSTALL: readonly CaseStep[] = [['hooks', 'install']];

// A project inside the project, with a config of its own.
export const nested: TextFiles = {
  'packages/inner/package.json': JSON.stringify({
    name: 'inner',
    version: '1.0.0',
    type: 'module',
  }),
  'packages/inner/docattest.config.json': config(),
  'packages/inner/src/index.ts': 'export const inner = 1;\n',
  'packages/inner/docs/inner.md': 'It is `1`. <!-- fact: inner -->\n\nUse `inner`.\n',
};

// An agent nobody ships, unlike the shipped ones wherever a descriptor can differ: where its
// settings are and how they nest, what its events are called, where an event holds the
// edited file and the sign that it is already continuing, and what an answer looks like.
export const acme = JSON.stringify({
  name: 'acme',
  docs: 'https://example.invalid/acme/automation',
  projectRoot: '"${ACME_WORKSPACE}"',
  skill: 'acme/guides/docattest.md',
  settings: {
    file: 'acme/config.json',
    hooks: 'automation',
    command: 'exec',
    handler: { shell: 'sh', timeoutMs: 60000 },
    defaults: { schema: 3, enabled: true },
  },
  hooks: {
    impact: {
      event: 'file.saved',
      matcher: '**/*',
      file: ['change', 'target', 'path'],
      reply: { fixed: { ok: true, notes: { level: 'info' } }, text: ['notes', 'body'] },
    },
    check: {
      event: 'turn.ending',
      continuing: ['turn', 'retries'],
      reply: { fixed: { action: 'continue' }, text: ['message'] },
    },
  },
});
export const reply = { text: ['say'] };
export const acmeFiles: TextFiles = {
  'tools/acme.json': acme,
  'tools/check-only.json': JSON.stringify({
    name: 'check-only',
    settings: { file: 'c.json', hooks: 'h', command: 'c' },
    hooks: { check: { event: 'end', reply } },
  }),
  'tools/impact-only.json': JSON.stringify({
    name: 'impact-only',
    settings: { file: 'c.json', hooks: 'h', group: 'g', command: 'c' },
    hooks: { impact: { event: 'edit', file: ['f'], reply } },
  }),
  // What the fixed part of an answer holds where the text goes gives way to the text.
  'tools/fixed.json': JSON.stringify({
    name: 'fixed',
    settings: { file: 'c.json', hooks: 'h', command: 'c' },
    hooks: {
      impact: {
        event: 'e',
        file: ['path'],
        reply: { fixed: { a: 'x', b: { c: 1 }, 10: 1, 2: 2 }, text: ['a', 'b', 'c'] },
      },
      check: {
        event: 'e',
        continuing: ['never', 'again'],
        reply: { fixed: { say: { no: 1 }, z: [1] }, text: ['say'] },
      },
    },
  }),
};
/** Stands for a field a broken descriptor lacks. */
const REMOVED = Symbol('removed');

/**
 * `acme` with the field at `path` (dotted) set to `value`, or removed: a descriptor that is
 * wrong in one way. Fields keep their order; a new one comes last.
 */
export function badAgent(path: string, value: unknown): string {
  const agent = JSON.parse(acme) as Record<string, unknown>;
  const keys = path.split('.');
  const last = keys.pop() ?? '';
  let node = agent;
  for (const key of keys) node = node[key] as Record<string, unknown>;
  if (value === REMOVED) Reflect.deleteProperty(node, last);
  else node[last] = value;
  return JSON.stringify(agent);
}
const acmeCheck: unknown = (JSON.parse(acme) as { hooks: { check: unknown } }).hooks.check;
export const badAgents = [
  '{ not json',
  '[]',
  '"claude"',
  '{}',
  '',
  '\ufeff{}',
  badAgent('name', 'Acme'),
  badAgent('name', 7),
  badAgent('title', 'Acme'),
  badAgent('docs', ''),
  badAgent('projectRoot', null),
  badAgent('skill', ['x']),
  badAgent('settings', REMOVED),
  badAgent('settings', []),
  badAgent('settings.file', REMOVED),
  badAgent('settings.hooks', ''),
  badAgent('settings.group', 1),
  badAgent('settings.handler', 'x'),
  badAgent('settings.command', REMOVED),
  badAgent('settings.defaults', null),
  badAgent('settings.path', 'x'),
  badAgent('hooks', REMOVED),
  badAgent('hooks.stop', acmeCheck),
  badAgent('hooks.impact', 'x'),
  badAgent('hooks.impact.event', REMOVED),
  badAgent('hooks.impact.matcher', 1),
  badAgent('hooks.impact.file', REMOVED),
  badAgent('hooks.impact.file', []),
  badAgent('hooks.impact.file', 'change.target.path'),
  badAgent('hooks.impact.file', ['a', '']),
  badAgent('hooks.impact.continuing', ['x']),
  badAgent('hooks.impact.reply', REMOVED),
  badAgent('hooks.impact.reply.fixed', []),
  badAgent('hooks.impact.reply.text', REMOVED),
  badAgent('hooks.impact.reply.template', '{text}'),
  badAgent('hooks.check.file', ['x']),
  badAgent('hooks.check.continuing', [1]),
  badAgent('hooks.check.reply', null),
  badAgent('hooks.check.event', ''),
];
export const BEGIN = '<!-- docattest:begin -->';
export const END = '<!-- docattest:end -->';

/** A snapshot file whose source is `path` on the local server. */
export const snapshot = (path: string, fetchedAt = '2020-01-01', data: unknown = { old: true }) =>
  JSON.stringify({ source: `${URL_BASE}${path}`, fetchedAt, data });
/** A project with snapshots (`cfg` says where), and `files`. */
export const withSnapshots = (
  files: TextFiles,
  cfg: Fields = { snapshots: { dir: 'docs/.snapshots', maxAgeDays: 30 } },
): TextFiles =>
  project({ 'src/index.ts': 'export const N = 3;\n', 'docs/a.md': '# A\n', ...files }, config(cfg));
