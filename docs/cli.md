---
title: The command line
---

# The command line

`docattest` is one command with a few subcommands. Every example on this page runs the
built command line against a small project in a temporary directory, the way CI would.

## `check` takes pages or globs

`docattest check` runs every living page. Given paths or globs relative to the project
root, it runs only those pages, which is what an agent does after editing one page; the
coverage line still describes the whole project. `--json` prints the report object instead
of text. The exit code is `1` when any result is `fail` or `error`, and `0` otherwise.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-cli-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('src/index.ts', 'export const N = 3;');
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('docs/good.md', 'N is `3`. <!-- fact: N -->');
put('docs/stale.md', 'N is `4`. <!-- fact: N -->');
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8' });
// ---cut---
const all = docattest('check');
assert.equal(all.status, 1);
assert.match(all.stdout, /docs\/stale\.md\n {2}✗ L1 fact: expected 4, got 3 from N/);

const one = docattest('check', 'docs/good.md');
assert.equal(one.status, 0);
assert.match(one.stdout, /results: 1 pass, 0 fail/);

const typo = docattest('check', 'docs/goood.md');
assert.equal(typo.status, 1);
assert.match(typo.stderr, /"docs\/goood\.md" matches no living page/);
```

## `--quiet` prints only what needs attention

`docattest check --quiet` is for whoever has to act on the result: a coding agent, a git
hook, a busy reader. It prints the results that `fail` or `error`, each under its page; the
warnings of a surface; the snapshots that are stale; and with `--base`, what the change did
to the proofs. Then the verdict, `ok` or `FAILED`. Passes, skips, coverage and the
surfaces' summaries are left out, so a project where everything holds prints one line. The
exit code is the same as without it.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-cli-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('src/index.ts', 'export const N = 3;');
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('docs/good.md', 'N is `3`. <!-- fact: N -->');
put('docs/stale.md', 'N is `4`. <!-- fact: N -->');
put('docs/plain.md', 'A page with no proofs.');
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8' });
// ---cut---
const failing = docattest('check', '--quiet');
assert.equal(failing.status, 1);
assert.equal(failing.stdout.trimEnd(), 'docs/stale.md\n  ✗ L1 fact: expected 4, got 3 from N\nFAILED');

const holding = docattest('check', '--quiet', 'docs/good.md');
assert.equal(holding.status, 0);
assert.equal(holding.stdout.trimEnd(), 'ok');
```

## The project is found from the working directory

Without `--root`, the project root is the nearest directory at or above the working
directory that holds a `docattest.config.json`, so a command run anywhere inside a project
checks that project. `--root` names the root exactly, and nothing is looked for above it.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-cli-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('src/deep/index.ts', 'export const N = 3;');
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/deep/index.ts' }, adapter: 'typescript' }));
put('docs/good.md', 'N is `3`. <!-- fact: N -->');
const docattest = (cwd: string, ...args: string[]) => spawnSync(process.execPath, [cli, ...args], { cwd: join(root, cwd), encoding: 'utf8' });
// ---cut---
const below = docattest('src/deep', 'check');
assert.equal(below.status, 0);
assert.match(below.stdout, /results: 1 pass, 0 fail/);

const named = docattest('src/deep', 'check', '--root', '.');
assert.equal(named.status, 1);
assert.match(named.stderr, /no docattest\.config\.json in /);
```

## One check for a repository of several projects

A repository can hold several projects, each with its own config, and each in its own
language. `members` in the config at the top lists the directories below it that hold one,
and `docattest check` there checks the top project and then each member, each with its own
config and adapter, and prints the reports one after another, ending with one verdict: the
exit code is `1` when any project fails. `--json` prints the top project's report with the
members' reports under `members`. Run from inside a member, the same command checks only that
member, as it always did.

A launcher knows how to start only its own language's adapter, so a member in another
language needs its adapter found some other way. A member written as `{ "path": …,
"adapterCommand": [program, arguments…] }` names the command that starts it; the words are
read from the directory of the config that lists it. Otherwise the member's adapter is found
as every adapter is: from `DOCATTEST_ADAPTER_<NAME>`, else `docattest-adapter-<name>` on
the `PATH`.

Members are checked only by `check` without pages. Named pages are the top project's, and
`where`, `index`, `impact` and the other commands answer for one project: the one they run in.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const adapter = fileURLToPath(new URL('../adapters/typescript/dist/protocol/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-members-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
const project = (dir: string, page: string, members?: unknown[]) => {
  put(`${dir}/package.json`, JSON.stringify({ name: 'demo', type: 'module' }));
  put(`${dir}/src/index.ts`, 'export const N = 3;');
  put(`${dir}/docattest.config.json`, JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', members }));
  put(`${dir}/docs/n.md`, page);
};
const docattest = (cwd: string, ...args: string[]) => spawnSync(process.execPath, [cli, ...args], { cwd: join(root, cwd), encoding: 'utf8' });
// ---cut---
project('.', 'N is `3`. <!-- fact: N -->', ['packages/good', { path: 'packages/stale', adapterCommand: [process.execPath, adapter] }]);
project('packages/good', 'N is `3`. <!-- fact: N -->');
project('packages/stale', 'N is `4`. <!-- fact: N -->');

const all = docattest('.', 'check');
assert.equal(all.status, 1);
const lines = all.stdout.split('\n');
assert.ok(lines.includes('member packages/good (typescript)'));
assert.match(all.stdout, /member packages\/stale \(typescript\)\ndocs\/n\.md\n {2}✗ L1 fact: expected 4, got 3 from N/);
assert.deepEqual(lines.slice(-3, -1), ['projects: 3 checked, 1 failed (packages/stale)', 'FAILED']);

// With --quiet, a member that needs nothing is left out.
const quiet = docattest('.', 'check', '--quiet');
assert.equal(quiet.stdout.trimEnd(), 'member packages/stale (typescript)\ndocs/n.md\n  ✗ L1 fact: expected 4, got 3 from N\n\nFAILED');

const json = JSON.parse(docattest('.', 'check', '--json').stdout);
assert.equal(json.exitCode, 1);
assert.deepEqual(json.members.map((m: { path: string; report: { exitCode: number } }) => [m.path, m.report.exitCode]), [['packages/good', 0], ['packages/stale', 1]]);

const inside = docattest('packages/good', 'check');
assert.equal(inside.status, 0);
assert.doesNotMatch(inside.stdout, /member/);
```

## The agent skill installs into the project

`docattest skill install` copies the docattest skill, a condensed version of the authoring
pattern for coding agents, to where the agent looks for skills. That place is the agent's
[descriptor](agents.md)'s to say: for Claude Code, the agent meant when `--agent` names no
other, it is `.claude/skills/docattest/SKILL.md` in the project. With `--global` it goes
to `~/.claude/skills/docattest/SKILL.md` instead. Installing again is a
no-op; a different file already in that place is left alone unless `--force` is given.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const packaged = readFileSync(new URL('../protocol/agents/SKILL.md', import.meta.url), 'utf8');
const root = mkdtempSync(join(tmpdir(), 'docattest-skill-'));
const home = mkdtempSync(join(tmpdir(), 'docattest-home-'));
const docattest = (...args: string[]) =>
  spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8', env: { ...process.env, HOME: home, USERPROFILE: home } });
// ---cut---
const installed = join(root, '.claude/skills/docattest/SKILL.md');

assert.equal(docattest('skill', 'install').status, 0);
assert.equal(readFileSync(installed, 'utf8'), packaged);
assert.equal(docattest('skill', 'install').status, 0);

writeFileSync(installed, 'my own notes');
const refused = docattest('skill', 'install');
assert.equal(refused.status, 1);
assert.match(refused.stderr, /--force/);
assert.equal(readFileSync(installed, 'utf8'), 'my own notes');

assert.equal(docattest('skill', 'install', '--force').status, 0);
assert.equal(readFileSync(installed, 'utf8'), packaged);

assert.equal(docattest('skill', 'install', '--global').status, 0);
assert.equal(readFileSync(join(home, '.claude/skills/docattest/SKILL.md'), 'utf8'), packaged);

// The same text, where another agent looks for it.
assert.equal(docattest('skill', 'install', '--agent', 'gemini').status, 0);
assert.equal(readFileSync(join(root, '.gemini/skills/docattest/SKILL.md'), 'utf8'), packaged);
```

## `conformance` holds a language adapter to the protocol

`docattest conformance run <kit.json> -- <adapter command>` is for whoever writes a language
adapter. It starts the adapter as the core would and holds it to the
[adapter protocol](protocol.md) and its schemas, given the adapter's kit: a JSON file of a
small project and labelled pieces of code in the language, whose shape is
`protocol/schema/kit.json`. It prints a line for each check and exits `1` when any does not
hold. `conformance record <log> -- <adapter
command>` stands between a core and an adapter and notes every line both ways in the log,
and `conformance validate <log>...` holds what was noted to the schemas.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const at = (path: string) => fileURLToPath(new URL(`../${path}`, import.meta.url));
const cli = at('adapters/typescript/dist/launcher/main.js');
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' });
// ---cut---
const adapter = [process.execPath, at('adapters/typescript/dist/protocol/main.js')];
const run = docattest('conformance', 'run', at('adapters/typescript/conformance.json'), '--', ...adapter);
assert.equal(run.status, 0);
assert.match(run.stdout, /conformance \(typescript\): 13 of 13 checks hold/);
```
