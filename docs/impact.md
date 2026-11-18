---
title: Impact: from a diff to the prose it touches
---

# Impact: from a diff to the prose it touches

`where` answers "which pages describe this export?" for an agent that knows to ask, and
asks by exported name. `impact` answers the question an agent actually has after an edit:
"what documentation does my current diff touch?". It compares the working tree with a
base revision, declaration by declaration, finds the surface exports whose code changed or
uses code that changed, and the constants facts read from source that changed, and lists
every page and line that names them: the attestations
`where` would list, and the pages that only mention a symbol in a code span, which prove
nothing and are the likeliest to go stale. Nothing runs and nothing is written. A changed
manifest, or another file a fact reads, reaches the facts that read it
([manifests](manifests.md)).

From code, `impact({ base })` takes a git revision for a project on disk, and an
in-memory project takes its base as files, `impact({ baseFiles })`, which are committed
with git in the project's temporary directory; the answer's `base` is then `files`.

## A change two calls below an export reaches the prose that proves the export

`retry` calls `backoff`, which calls `jitter`; neither helper is exported. Changing `jitter`
changes what `retry` does, so `retry` is affected, and the answer carries the claim its
example defends.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': "export { retry } from './retry.js';\nexport { parse } from './parse.js';",
  'src/retry.ts': "import { backoff } from './backoff.js';\nexport async function retry<T>(fn: () => Promise<T>): Promise<T> {\n  try { return await fn(); } catch { await new Promise((r) => setTimeout(r, backoff(1))); return fn(); }\n}",
  'src/backoff.ts': "import { jitter } from './jitter.js';\nexport const backoff = (attempt: number) => jitter(attempt * 100);",
  'src/jitter.ts': 'export const jitter = (ms: number) => ms;',
  'src/parse.ts': 'export const parse = (text: string) => Number(text);',
  'docs/retry.md': ['# Retry', '', 'A successful call returns its value unchanged.', '', '```ts check', "import { retry } from 'docattest-fixture';", "assert.equal(await retry(async () => 'ok'), 'ok');", '```'].join('\n'),
};
const project = createProject({ files: { ...base, 'src/jitter.ts': 'export const jitter = (ms: number) => ms * 2;' } });

const impact = await project.impact({ baseFiles: base });
assert.deepEqual(impact.symbols, ['retry']);
assert.deepEqual(impact.references, [
  { symbol: 'retry', page: 'docs/retry.md', line: 5, kind: 'example', claim: 'A successful call returns its value unchanged.', proven: true },
]);
assert.deepEqual(project.writes(), []);
```

## Comments, formatting and code no export reaches are not changes

Declarations are compared on the tokens the compiler reads, so a comment, a reformat, a
dropped semicolon or a different quote style changes nothing. A declaration no export
reaches affects nothing either. The answer is then empty.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': "export { parse } from './parse.js';",
  'src/parse.ts': "export const parse = (text: string) => Number(text);",
  'src/unused.ts': 'export const unused = () => 1;',
  'docs/parse.md': 'Use `parse` to read numbers.',
};
const project = createProject({
  files: {
    ...base,
    'src/parse.ts': '/** Text to number. */\nexport const parse = (\n  text: string,\n) => Number(text)\n',
    'src/unused.ts': 'export const unused = () => 2;',
  },
});

assert.deepEqual(await project.impact({ baseFiles: base }), { base: 'files', symbols: [], references: [] });
```

## Types count: an interface that gained a field affects its users

An exported interface that gained a field is affected, and so is every export whose
signature uses it. An affected export that no page names is still listed in `symbols`.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': "export type { Options } from './options.js';\nexport { connect } from './connect.js';",
  'src/options.ts': 'export interface Options { host: string }',
  'src/connect.ts': "import type { Options } from './options.js';\nexport const connect = (o: Options) => o.host;",
};
const project = createProject({ files: { ...base, 'src/options.ts': 'export interface Options { host: string; port?: number }' } });

const impact = await project.impact({ baseFiles: base });
assert.deepEqual(impact.symbols, ['Options', 'connect']);
assert.deepEqual(impact.references, []);
```

## A page that only names a symbol is listed as an unproven mention

A living page that names an affected export in an inline code span, on a line that is not
part of an attestation of it, is listed with `kind: "mention"`, `proven: false` and the
sentence around the span as its claim. The claim an attestation defends is not listed
twice.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const parse = (text: string) => Number(text);',
  'docs/parse.md': ['# Parse', '', '`parse` turns text into a number.', '', '```ts check', "import { parse } from 'docattest-fixture';", "assert.equal(parse('4'), 4);", '```'].join('\n'),
  'docs/guide.md': '# Guide\n\nStart here. Read the input with `parse` before you retry anything.',
};
const project = createProject({ files: { ...base, 'src/index.ts': 'export const parse = (text: string) => Number.parseInt(text, 10);' } });

const { references } = await project.impact({ baseFiles: base });
assert.deepEqual(references, [
  { symbol: 'parse', page: 'docs/guide.md', line: 3, kind: 'mention', claim: 'Read the input with `parse` before you retry anything.', proven: false },
  { symbol: 'parse', page: 'docs/parse.md', line: 5, kind: 'example', claim: '`parse` turns text into a number.', proven: true },
]);
```

## A cited test is listed as cited, not proven

A sentence defended by a [citation](citations.md) of a test that uses an affected export is
listed with `kind: "test"` and `proven: false`, and the text report calls it a cited test:
docattest checks that the test is still there, never that it passes, so the change may well
have made the sentence untrue.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const parse = (text: string) => Number(text);',
  'test/parse.test.ts': "import { it } from 'vitest';\nimport { parse } from '../src/index.js';\nit('reads digits', () => { parse('4'); });",
  'docs/parse.md': '# Parse\n\nDigits are read as a number. <!-- test: test/parse.test.ts > "reads digits" -->',
};
const project = createProject({ files: { ...base, 'src/index.ts': 'export const parse = (text: string) => Number.parseInt(text, 10);' } });

const { references } = await project.impact({ baseFiles: base });
assert.deepEqual(references, [
  { symbol: 'parse', page: 'docs/parse.md', line: 3, kind: 'test', claim: 'Digits are read as a number.', proven: false },
]);
```

## A console block is reached through the command it runs

A [console block](console.md) runs a program, not an export, and no language can trace what
a built program runs back to its source. The config's `commands` says it instead: each
command the blocks call, by name, with globs of the files it is built from. A change to one
of those files lists the command under `commands`, and every console block with a command
line that calls it as a reference whose `symbol` is the call, `$ mytool`. A line calls a
command when one of its words is the name or a path ending in it (`./bin/mytool`). An
[`md check`](console.md) block runs `docattest`, so it is listed under that command.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({
    pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript',
    commands: { mytool: ['src/cli/**/*.ts'] },
  }),
  'src/index.ts': 'export const parse = (text: string) => Number(text);',
  'src/cli/main.ts': "console.log('usage: mytool <file>');",
  'docs/cli.md': ['# The tool', '', 'With no file, it prints its usage.', '', '```console check', '$ mytool', 'usage: mytool <file>', '```'].join('\n'),
};
const project = createProject({ files: { ...base, 'src/cli/main.ts': "console.log('usage: mytool [file]');" } });

const impact = await project.impact({ baseFiles: base });
assert.deepEqual(impact.symbols, []);
assert.deepEqual(impact.commands, ['mytool']);
assert.deepEqual(impact.references, [
  { symbol: '$ mytool', page: 'docs/cli.md', line: 5, kind: 'example', claim: 'With no file, it prints its usage.', proven: true },
]);
```

## `docattest impact` reads the diff from git

On the command line the base is a git revision, `HEAD` unless `--base` names another, and
the working tree is compared with it, untracked files included. A directory that is not a
git repository, or a revision that does not exist, is an error, never an empty answer.

```ts check
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const env = { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null', GIT_AUTHOR_NAME: 'docs', GIT_AUTHOR_EMAIL: 'docs@example.invalid', GIT_COMMITTER_NAME: 'docs', GIT_COMMITTER_EMAIL: 'docs@example.invalid' };
const project = (root: string) => {
  const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
  put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
  put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
  put('src/index.ts', 'export const LIMIT = 3;');
  put('docs/limit.md', 'At most `3` calls are made. <!-- fact: LIMIT -->');
  return put;
};
const docattest = (root: string, ...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8', env });
const root = mkdtempSync(join(tmpdir(), 'docattest-impact-'));
const put = project(root);
for (const args of [['init', '-q'], ['add', '-A'], ['commit', '-q', '-m', 'base']]) execFileSync('git', args, { cwd: root, env });
// ---cut---
put('src/index.ts', 'export const LIMIT = 5;');
const run = docattest(root, 'impact', '--json');
assert.equal(run.status, 0);
assert.deepEqual(JSON.parse(run.stdout), {
  base: 'HEAD',
  symbols: ['LIMIT'],
  references: [{ symbol: 'LIMIT', page: 'docs/limit.md', line: 1, kind: 'fact', claim: 'At most `3` calls are made.', proven: true }],
});
assert.match(docattest(root, 'impact').stdout, /docs\/limit\.md:1 proven fact {2}At most `3` calls are made\./);

const unknown = docattest(root, 'impact', '--base', 'no-such-branch');
assert.equal(unknown.status, 1);
assert.match(unknown.stderr, /"no-such-branch" is not a commit/);

const plain = mkdtempSync(join(tmpdir(), 'docattest-nogit-'));
project(plain);
const outside = docattest(plain, 'impact');
assert.equal(outside.status, 1);
assert.match(outside.stderr, /impact needs a git repository/);
```

## `docattest impact --file` is the question after an edit

`docattest impact --file <path>` is what an agent, a script or an editor runs after a file
was edited. A relative path is read from the working directory, as the shell gave it, and
the project is the one the file is in. It answers for that file's change alone, as though
no other file had changed, so the rest of the uncommitted work does not crowd out what the
edit touched. It prints the same report, and only when there is something to read: nothing
when the file is not a source file of the project, and nothing when its change affects no
export. It exits `0` in each case.

A path that names nothing is a mistake, never a quiet "nothing affected": a path outside
the project, or one that neither exists nor is in the diff, exits `1` and says which file
was looked for. A file the change deleted is in the diff. `impact --hook` reads the same
path from an agent's event, but an agent edits files everywhere, so an edit outside the
project is simply not the project's: the hook stays silent.

```ts check
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const env = { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null', GIT_AUTHOR_NAME: 'docs', GIT_AUTHOR_EMAIL: 'docs@example.invalid', GIT_COMMITTER_NAME: 'docs', GIT_COMMITTER_EMAIL: 'docs@example.invalid' };
const root = mkdtempSync(join(tmpdir(), 'docattest-impact-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', "export const LIMIT = 3;\nexport { RETRIES } from './retries.js';");
put('src/retries.ts', 'export const RETRIES = 2;');
put('docs/limit.md', 'At most `3` calls are made. <!-- fact: LIMIT -->');
put('src/old.ts', 'export const unused = 1;');
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8', env });
for (const args of [['init', '-q'], ['add', '-A'], ['commit', '-q', '-m', 'base']]) execFileSync('git', args, { cwd: root, env });
// ---cut---
// A comment changes no declaration: the edit affects nothing, and nothing is printed.
put('src/index.ts', "// the limit\nexport const LIMIT = 3;\nexport { RETRIES } from './retries.js';");
const nothing = docattest('impact', '--file', join(root, 'src/index.ts'));
assert.deepEqual([nothing.status, nothing.stdout], [0, '']);

// Two files changed: each answers for its own change.
put('src/index.ts', "export const LIMIT = 5;\nexport { RETRIES } from './retries.js';");
put('src/retries.ts', 'export const RETRIES = 4;');
const affected = docattest('impact', '--file', join(root, 'src/index.ts'), '--json');
assert.deepEqual(JSON.parse(affected.stdout).symbols, ['LIMIT']);
const retries = docattest('impact', '--file', join(root, 'src/retries.ts'), '--json');
assert.deepEqual(JSON.parse(retries.stdout).symbols, ['RETRIES']);

// A page was edited, not source code: nothing, though the diff still affects LIMIT.
const page = docattest('impact', '--file', join(root, 'docs/limit.md'));
assert.deepEqual([page.status, page.stdout], [0, '']);

// Written relative to the project root, but run from docs/: there is no such file.
const fromDocs = spawnSync(process.execPath, [cli, 'impact', '--file', 'src/index.ts'], { cwd: join(root, 'docs'), encoding: 'utf8', env });
assert.equal(fromDocs.status, 1);
assert.match(fromDocs.stderr, /no file .*docs\/src\/index\.ts/);

const outside = docattest('impact', '--file', join(root, '..', 'elsewhere.ts'));
assert.equal(outside.status, 1);
assert.match(outside.stderr, /outside the project/);

// Deleted, so not on disk, but in the diff: a real answer.
rmSync(join(root, 'src/old.ts'));
assert.equal(docattest('impact', '--file', join(root, 'src/old.ts')).status, 0);
```

## `docattest hooks install` makes an agent run it without being asked

`docattest hooks install` adds two hooks to the settings of a coding agent that has hooks:
after the agent edits or writes a file, `docattest impact --hook` puts a brief of the
edit's impact next to it, and before it stops, `docattest check --hook` sends it back to work
once if a page no longer matches the code. Whatever the settings file already holds is kept,
and a second run adds nothing. `--agent` names the agent; without it the hooks are Claude
Code's, in `.claude/settings.json`. Which agents there are, how one is described, and what
to do for an agent with no hooks is on the [coding agents](agents.md) page.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-hooks-'));
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8' });
const settings = join(root, '.claude', 'settings.json');
mkdirSync(join(root, '.claude'));
writeFileSync(settings, JSON.stringify({ permissions: { allow: ['Bash(npm test)'] } }));
// ---cut---
const first = docattest('hooks', 'install');
assert.equal(first.status, 0);
assert.match(first.stdout, /PostToolUse \(Edit\|Write\): npx --no docattest impact --hook/);

const installed = JSON.parse(readFileSync(settings, 'utf8'));
assert.deepEqual(installed.permissions, { allow: ['Bash(npm test)'] });
assert.deepEqual(installed.hooks, {
  PostToolUse: [{ matcher: 'Edit|Write', hooks: [{ type: 'command', command: 'npx --no docattest impact --hook' }] }],
  Stop: [{ hooks: [{ type: 'command', command: 'npx --no docattest check --hook' }] }],
});

assert.match(docattest('hooks', 'install').stdout, /already installed/);
assert.deepEqual(JSON.parse(readFileSync(settings, 'utf8')), installed);
```

## After an edit the agent gets a brief, not the report

The hook comes after every edit, so what it adds to the agent's context is kept small
whatever the change. It lists each place that names what the edit affects and that nothing
checks, once with every symbol it names and at most five, then how many more there are and
the command that lists them; the places a proof or a cited test defends it only counts,
since `docattest check` covers them. When no page names what the edit affects, it says
nothing. `docattest impact --file` still prints the whole report.

```ts check
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const env = { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null', GIT_AUTHOR_NAME: 'docs', GIT_AUTHOR_EMAIL: 'docs@example.invalid', GIT_COMMITTER_NAME: 'docs', GIT_COMMITTER_EMAIL: 'docs@example.invalid' };
const root = mkdtempSync(join(tmpdir(), 'docattest-brief-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', 'export const LIMIT = 3;\nexport function retry() {}');
put('docs/limit.md', 'At most `3` calls are made. <!-- fact: LIMIT -->\n\nCall `retry` after `LIMIT` calls fail.');
for (const args of [['init', '-q'], ['add', '-A'], ['commit', '-q', '-m', 'base']]) execFileSync('git', args, { cwd: root, env });
const hook = (file: string) => spawnSync(process.execPath, [cli, 'impact', '--hook', '--root', root], { encoding: 'utf8', env, input: JSON.stringify({ tool_input: { file_path: join(root, file) } }) });
// ---cut---
put('src/index.ts', 'export const LIMIT = 5;\nexport function retry(times = 1) {}');
const brief = JSON.parse(hook('src/index.ts').stdout).hookSpecificOutput.additionalContext;
assert.equal(
  brief,
  'docattest: this edit affects 2 symbols. Nothing checks these places; re-read them:\n' +
    '  docs/limit.md:3 LIMIT, retry "Call `retry` after `LIMIT` calls fail."\n' +
    'docattest check covers 1 other place.',
);
```
