---
title: Coding agents
---

# Coding agents

docattest wants two things of a coding agent: after it edits code, look at the prose the
edit touches; before it finishes, run the check. Neither needs a particular agent. Two
plain commands do the work, a section of `AGENTS.md` tells any agent to run them, and a git
hook catches what was missed. An agent that has hooks of its own can be made to run the
commands without being told, and everything docattest knows about such an agent is one
small JSON file, a *descriptor*. Nothing else in docattest names an agent.

## Two plain commands

`docattest impact --file <path>` is what to run after editing a file. When the file is a
source file of the project and its difference from `HEAD` affects a symbol the pages can
attest, it prints what [`impact`](impact.md) prints for that file's change; otherwise it
prints nothing.
It exits `0` either way, so it can run after every edit. `docattest check` is what to run
before finishing: it exits `1` when a page no longer matches the code.

The project is found from where the command runs: without `--root`, the root is the nearest
directory at or above the working directory that holds a `docattest.config.json`, and for
`impact --file` the nearest one at or above the edited file comes first. No variable of any
agent is needed to say where the project is.

```ts check
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const env = { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null', GIT_AUTHOR_NAME: 'docs', GIT_AUTHOR_EMAIL: 'docs@example.invalid', GIT_COMMITTER_NAME: 'docs', GIT_COMMITTER_EMAIL: 'docs@example.invalid' };
const root = realpathSync(mkdtempSync(join(tmpdir(), 'docattest-agents-')));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', 'export const LIMIT = 3;');
put('docs/limit.md', 'At most `3` calls are made. <!-- fact: LIMIT -->');
for (const args of [['init', '-q'], ['add', '-A'], ['commit', '-q', '-m', 'base']]) execFileSync('git', args, { cwd: root, env });
const docattest = (cwd: string, ...args: string[]) => spawnSync(process.execPath, [cli, ...args], { cwd: join(root, cwd), encoding: 'utf8', env });
// ---cut---
put('src/index.ts', 'export const LIMIT = 5;');

// From a directory below the root, with no --root: the project is found.
const edited = docattest('src', 'impact', '--file', 'index.ts');
assert.equal(edited.status, 0);
assert.match(edited.stdout, /docs\/limit\.md:1 proven fact {2}At most `3` calls are made\./);

// A page is not a source file: nothing is printed.
const page = docattest('.', 'impact', '--file', 'docs/limit.md');
assert.deepEqual([page.status, page.stdout], [0, '']);

const check = docattest('docs', 'check');
assert.equal(check.status, 1);
assert.match(check.stdout, /✗ L1 fact: expected 3, got 5 from LIMIT/);
```

## `init --agents` tells any agent to run them

`docattest init --agents` writes a section into `AGENTS.md` at the project root, the file
coding agents read their instructions from. The section says to run `impact --file` after
an edit and `check` before finishing, with the command that runs docattest in this project.
It stands between two markers, `<!-- docattest:begin -->` and `<!-- docattest:end -->`.
Running the command again replaces what is between the markers and nothing else: a file
that has no section gets one at its end, and text outside the markers is never changed.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-agents-'));
const { DOCATTEST_COMMAND: _unset, ...env } = process.env;
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8', env });
const file = join(root, 'AGENTS.md');
// ---cut---
writeFileSync(file, '# Our rules\n\nUse tabs.\n');
assert.match(docattest('init', '--agents', '--no-audit').stdout, /docattest agents: added to /);

const written = readFileSync(file, 'utf8');
assert.ok(written.startsWith('# Our rules\n\nUse tabs.\n\n<!-- docattest:begin -->\n'));
assert.ok(written.endsWith('<!-- docattest:end -->\n'));
assert.match(written, /run `npx --no docattest impact --file <path>`/);
assert.match(written, /run `npx --no docattest check --quiet`/);

assert.match(docattest('init', '--agents').stdout, /already up to date/);

// An older section is replaced where it stands; the text around it stays.
writeFileSync(file, '# Our rules\n<!-- docattest:begin -->\nold\n<!-- docattest:end -->\nUse tabs.\n');
assert.match(docattest('init', '--agents').stdout, /^docattest agents: updated in /);
const again = readFileSync(file, 'utf8');
assert.ok(again.startsWith('# Our rules\n<!-- docattest:begin -->\n## '));
assert.ok(again.endsWith('<!-- docattest:end -->\nUse tabs.\n'));

// Markers that are not one section are refused, and the file is left alone.
writeFileSync(file, '<!-- docattest:begin -->\nnever ends\n');
const refused = docattest('init', '--agents');
assert.equal(refused.status, 1);
assert.match(refused.stderr, /fix the markers and run again/);
assert.equal(readFileSync(file, 'utf8'), '<!-- docattest:begin -->\nnever ends\n');
```

## `init --git-hook` checks before each commit

`docattest init --git-hook` writes a git `pre-commit` hook that runs
`docattest check --quiet` and stops the commit when it fails. The hook goes where git looks
for hooks, so `core.hooksPath` is respected, and in a project below the top of its
repository the hook changes to the project's directory first. A `pre-commit` hook that is
already there is never changed: when it holds the line, the hook is installed; when it does
not, the command exits `1` and prints the line to add.

The check reads the working tree, as every docattest command does, and not only what is
staged: that is what CI will see once the work is pushed, and checking the index alone would
mean writing the staged files somewhere else for each commit.

```ts check
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const { DOCATTEST_COMMAND: _unset, ...rest } = process.env;
const env = { ...rest, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null' };
const repository = () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'docattest-git-')));
  execFileSync('git', ['init', '-q'], { cwd: root, env });
  return root;
};
const docattest = (root: string, ...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8', env });
// ---cut---
const root = repository();
const hook = join(root, '.git/hooks/pre-commit');
const line = 'npx --no docattest check --quiet || exit 1';

const first = docattest(root, 'init', '--git-hook', '--no-audit');
assert.ok(first.stdout.includes(`docattest hooks: added ${hook}\n  pre-commit: ${line}\n`));
assert.ok(readFileSync(hook, 'utf8').startsWith('#!/bin/sh\n'));
assert.ok(readFileSync(hook, 'utf8').endsWith(`\n${line}\n`));
assert.equal(statSync(hook).mode & 0o111, 0o111);
assert.match(docattest(root, 'init', '--git-hook').stdout, /already installed/);

// Someone else's hook is left as it is.
const other = repository();
execFileSync('git', ['config', 'core.hooksPath', '.githooks'], { cwd: other, env });
mkdirSync(join(other, '.githooks'));
writeFileSync(join(other, '.githooks/pre-commit'), '#!/bin/sh\nnpm test\n');
const refused = docattest(other, 'init', '--git-hook', '--no-audit');
assert.equal(refused.status, 1);
assert.ok(refused.stderr.endsWith(`add this line to it:\n  ${line}\n`));
assert.equal(readFileSync(join(other, '.githooks/pre-commit'), 'utf8'), '#!/bin/sh\nnpm test\n');
```

## An agent with hooks is described by a descriptor

Some agents run a command of the project on their own events. For such an agent,
`docattest init --hooks --agent <name>` writes two hooks into the agent's settings file:
`docattest impact --hook` after the agent edits a file, and `docattest check --hook` before
it stops. In hook mode the commands read the agent's event from standard input and answer
in the agent's JSON: `impact --hook` puts a brief of the impact into the agent's context, and
`check --hook` keeps the agent working, once, when a page no longer matches the code.

What differs between agents is data. This is the whole descriptor of Claude Code, the agent
meant when `--agent` is not given:

```ts check
import { readFileSync } from 'node:fs';

const descriptor = JSON.parse(readFileSync(new URL('../protocol/agents/claude.json', import.meta.url), 'utf8'));
const index = JSON.parse(readFileSync(new URL('../protocol/agents/index.json', import.meta.url), 'utf8'));
// ---cut---
assert.deepEqual(descriptor, {
  $schema: '../schema/agent.json#/$defs/Agent',
  name: 'claude',
  docs: 'https://code.claude.com/docs/en/hooks',
  projectRoot: '"$CLAUDE_PROJECT_DIR"',
  skill: '.claude/skills/docattest/SKILL.md',
  settings: {
    file: '.claude/settings.json',
    localFile: '.claude/settings.local.json',
    hooks: 'hooks',
    group: 'hooks',
    handler: { type: 'command' },
    command: 'command',
  },
  hooks: {
    impact: {
      event: 'PostToolUse',
      matcher: 'Edit|Write',
      file: ['tool_input', 'file_path'],
      reply: {
        fixed: { hookSpecificOutput: { hookEventName: 'PostToolUse' } },
        text: ['hookSpecificOutput', 'additionalContext'],
      },
    },
    check: {
      event: 'Stop',
      continuing: ['stop_hook_active'],
      reply: { fixed: { decision: 'block' }, text: ['reason'] },
    },
  },
});
assert.deepEqual(index, { default: 'claude' });
```

| field | what it says | why it is there |
|---|---|---|
| `name` | the agent's name, as `--agent` takes it | a shipped descriptor is found by it |
| `docs` | the page of the agent's documentation the descriptor was checked against | hook formats change; this is where to look when one does |
| `projectRoot` | how a hook command of this agent writes the project root | a command such as `{root}/.venv/bin/docattest` must not hold one machine's path; left out when the agent runs hooks in the project root, where `.` will do |
| `skill` | where the agent looks for a skill, relative to the project root or the home directory | `init --skill` writes the language-specific skill there; left out when the agent has no such place |
| `settings.file` | the JSON file the agent reads hooks from, relative to the project root | where `init --hooks` writes |
| `settings.localFile` | the untracked project-local settings file, relative to the project root | where `init --hooks --local` writes, so a teammate without docattest is unaffected; left out when the agent has no such file |
| `settings.hooks` | the key of that file that maps an event name to a list of entries | where a hook goes in the file |
| `settings.group` | the key under which an entry holds its list of handlers | some agents group handlers under a filter; left out when an entry is itself a handler |
| `settings.handler` | fields every handler carries besides its command | an agent may want a type, a shell or a timeout on each |
| `settings.command` | the key of a handler that holds the shell command | to write the command, and to see that a hook is already installed |
| `settings.defaults` | top-level keys the file must have, with their values | some agents need a version in the file; added only where missing |
| `hooks.impact`, `hooks.check` | the agent's event for each action | an action the agent has no event for is left out, and its hook is not installed |
| `event`, `matcher` | the event's name, and the filter that selects the tools that edit files | which list of the settings the entry joins |
| `file` | where the edited file's path is in the event, as a list of keys | `impact --hook` reads it from standard input |
| `continuing` | where the event says the agent is already continuing because a hook sent it back | `check --hook` stays silent when the value there is `true` or a number above `0`, so a failure the agent cannot fix never loops |
| `reply.fixed`, `reply.text` | the JSON object the hook prints, and where in it the text goes | each agent has its own shape for "context for the agent" and for "do not stop, because" |

The format is defined by `protocol/schema/agent.json`. There is no template language in it:
a path is a list of keys, and an answer is a fixed object with one string put into it.

## The shipped descriptors

A descriptor is shipped only when the agent's own documentation says what its hooks look
like. An unknown name is refused with the names that are known.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
// ---cut---
const unknown = spawnSync(process.execPath, [cli, 'init', '--hooks', '--no-audit', '--agent', 'nobody'], { encoding: 'utf8' });
assert.equal(unknown.status, 1);
assert.equal(
  unknown.stderr,
  'docattest: unknown agent "nobody" (known: antigravity, claude, codex, cursor, gemini); for another agent pass its descriptor file, as in --agent ./nobody.json\n',
);
```

| `--agent` | agent | hooks | checked against |
|---|---|---|---|
| `antigravity` | Antigravity | `check` when the execution loop stops | <https://antigravity.google/docs> |
| `claude` | Claude Code | `impact` after `Edit` and `Write`, `check` before it stops | <https://code.claude.com/docs/en/hooks> |
| `codex` | OpenAI Codex | `check` when the agent turn stops | <https://developers.openai.com/codex> |
| `cursor` | Cursor | `check` when the agent loop ends | <https://cursor.com/docs/agent/hooks> |
| `gemini` | Gemini CLI | `impact` after `write_file` and `replace`, `check` after the agent's turn | <https://geminicli.com/docs/hooks/reference/> |

`init --hooks --local` writes into the agent's untracked local settings file
(`settings.localFile`, such as `.claude/settings.local.json` for Claude Code) instead of
the shared settings file, so teammates who do not have docattest installed are not
affected. Inside a git repository, if the written file is not ignored by git, docattest
prints a note reminding you to add it to `.gitignore` or `.git/info/exclude`. An agent
whose descriptor names no `localFile` refuses `--local`.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-agents-local-'));
spawnSync('git', ['init', '-q', root]);
spawnSync('git', ['-C', root, 'config', 'core.excludesFile', '/dev/null']);
const { DOCATTEST_COMMAND: _unset, ...env } = process.env;
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8', env });
// ---cut---
const first = docattest('init', '--hooks', '--local', '--no-audit');
assert.equal(first.status, 0);
assert.match(first.stdout, /note: \.claude\/settings\.local\.json is not ignored by git/);
const local = JSON.parse(readFileSync(join(root, '.claude/settings.local.json'), 'utf8'));
assert.equal(local.hooks.Stop[0].hooks[0].command, 'npx --no docattest check --hook');

writeFileSync(join(root, '.gitignore'), '.claude/settings.local.json\n');
const ignored = docattest('init', '--hooks', '--local');
assert.equal(ignored.status, 0);
assert.doesNotMatch(ignored.stdout, /not ignored by git/);

const unsupported = docattest('init', '--hooks', '--local', '--agent', 'cursor');
assert.equal(unsupported.status, 1);
assert.match(unsupported.stderr, /has no local settings file/);
```

Cursor, Codex and Antigravity bind no event to `impact`: Cursor's post-edit event and
Antigravity's `PostToolUse` accept no context reply for the agent, and Codex's `apply_patch`
passes a multi-file patch rather than a single `file_path`. For those agents, as for any
agent with no hooks at all, the reminder after an edit comes from `AGENTS.md`.

Each agent's hooks take that agent's shape. Gemini CLI groups handlers as Claude Code does,
under other event names, and is told `deny` where Claude Code is told `block`. Cursor's
entries are the handlers themselves, its file needs a `version`, and it is kept working by a
follow-up message.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-agents-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', 'export const LIMIT = 5;');
put('docs/limit.md', 'At most `3` calls are made. <!-- fact: LIMIT -->');
const { DOCATTEST_COMMAND: _unset, ...env } = process.env;
const docattest = (input: unknown, ...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8', env, input: JSON.stringify(input) });
const read = (rel: string) => JSON.parse(readFileSync(join(root, rel), 'utf8'));
// ---cut---
assert.equal(docattest({}, 'init', '--hooks', '--agent', 'gemini').status, 0);
assert.deepEqual(read('.gemini/settings.json'), {
  hooks: {
    AfterTool: [{ matcher: 'write_file|replace', hooks: [{ type: 'command', command: 'npx --no docattest impact --hook --agent gemini' }] }],
    AfterAgent: [{ hooks: [{ type: 'command', command: 'npx --no docattest check --hook --agent gemini' }] }],
  },
});
const denied = JSON.parse(docattest({ stop_hook_active: false }, 'check', '--hook', '--agent', 'gemini').stdout);
assert.equal(denied.decision, 'deny');
assert.match(denied.reason, /^docattest check failed: these pages no longer match the code\.\n/);

assert.equal(docattest({}, 'init', '--hooks', '--agent', 'cursor').status, 0);
assert.deepEqual(read('.cursor/hooks.json'), {
  version: 1,
  hooks: { stop: [{ command: 'npx --no docattest check --hook --agent cursor' }] },
});
const followup = JSON.parse(docattest({ status: 'completed', loop_count: 0 }, 'check', '--hook', '--agent', 'cursor').stdout);
assert.deepEqual(Object.keys(followup), ['followup_message']);
// Once it has been sent back, it is let go.
assert.equal(docattest({ status: 'completed', loop_count: 1 }, 'check', '--hook', '--agent', 'cursor').stdout, '');

assert.equal(docattest({}, 'init', '--hooks', '--agent', 'codex').status, 0);
assert.deepEqual(read('.codex/hooks.json'), {
  hooks: {
    Stop: [{ hooks: [{ type: 'command', command: 'npx --no docattest check --hook --agent codex' }] }],
  },
});
const codexBlock = JSON.parse(docattest({ stop_hook_active: false }, 'check', '--hook', '--agent', 'codex').stdout);
assert.equal(codexBlock.decision, 'block');
assert.equal(docattest({ stop_hook_active: true }, 'check', '--hook', '--agent', 'codex').stdout, '');

assert.equal(docattest({}, 'init', '--hooks', '--agent', 'antigravity').status, 0);
assert.deepEqual(read('.agents/hooks.json'), {
  docattest: {
    Stop: [{ type: 'command', command: 'npx --no docattest check --hook --agent antigravity' }],
  },
});
const agyContinue = JSON.parse(docattest({ continuationActive: false }, 'check', '--hook', '--agent', 'antigravity').stdout);
assert.equal(agyContinue.decision, 'continue');
assert.equal(docattest({ continuationActive: true }, 'check', '--hook', '--agent', 'antigravity').stdout, '');
```

## A descriptor of your own

For an agent docattest does not ship, write a descriptor and pass its file: anything given
to `--agent` that is not a bare name is a file. The hooks that are installed name the file
by a path they can find again, relative to the project root as the agent writes it. A
descriptor the format does not understand is refused with the place of the mistake.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = realpathSync(mkdtempSync(join(tmpdir(), 'docattest-agents-')));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', 'export const LIMIT = 5;');
put('docs/limit.md', 'At most `3` calls are made. <!-- fact: LIMIT -->');
const { DOCATTEST_COMMAND: _unset, ...env } = process.env;
const docattest = (input: unknown, ...args: string[]) => spawnSync(process.execPath, [cli, ...args], { cwd: root, encoding: 'utf8', env, input: JSON.stringify(input) });
// ---cut---
const mine = {
  name: 'acme',
  projectRoot: '"$ACME_WORKSPACE"',
  settings: { file: 'acme/config.json', hooks: 'automation', command: 'exec', defaults: { schema: 3 } },
  hooks: {
    check: { event: 'turn.ending', continuing: ['turn', 'retries'], reply: { fixed: { action: 'continue' }, text: ['message'] } },
  },
};
put('tools/acme.json', JSON.stringify(mine));

assert.equal(docattest({}, 'init', '--hooks', '--agent', './tools/acme.json').status, 0);
assert.deepEqual(JSON.parse(readFileSync(join(root, 'acme/config.json'), 'utf8')), {
  schema: 3,
  automation: { 'turn.ending': [{ exec: 'npx --no docattest check --hook --agent "$ACME_WORKSPACE"/tools/acme.json' }] },
});

const answer = JSON.parse(docattest({ turn: { retries: 0 } }, 'check', '--hook', '--agent', 'tools/acme.json').stdout);
assert.equal(answer.action, 'continue');
assert.match(answer.message, /expected 3, got 5 from LIMIT/);
assert.equal(docattest({ turn: { retries: 1 } }, 'check', '--hook', '--agent', 'tools/acme.json').stdout, '');

put('tools/wrong.json', JSON.stringify({ ...mine, hooks: { check: { ...mine.hooks.check, reply: { template: '{text}' } } } }));
const wrong = docattest({}, 'init', '--hooks', '--agent', 'tools/wrong.json');
assert.equal(wrong.status, 1);
assert.equal(wrong.stderr, `docattest: ${join(root, 'tools/wrong.json')}: unknown key "hooks.check.reply.template"\n`);
```

## Why

Every agent has its own settings file, its own event names and its own JSON for the same two
answers, and they change. Written into the code, each one is a branch that has to be kept
true in two implementations; written as data, a new agent is a file a user can add without
a release, and the format is tested against an agent nobody ships. The plain commands come
first because they outlast every format: an agent that can run a command and read
`AGENTS.md` is supported today, whatever its hooks look like next year.
