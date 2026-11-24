---
title: The command line
---

# The command line

`docattest` is one command with a few subcommands. Every example on this page runs the
built command line against a small project in a temporary directory, the way CI would.

## `check` takes pages or globs

`docattest check` runs every living page. Given paths or globs relative to the project
root, it runs only those pages, which is what an agent does after editing one page; the
coverage line still describes the whole project, in counts: the uncovered pages are named
only when every page ran, and frozen pages are counted, never named. `--json` prints the
report object instead of text, with every list in full. The exit code is `1` when any
result is `fail` or `error`, and `0` otherwise.

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
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], frozen: ['docs/old/**'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('docs/good.md', 'N is `3`. <!-- fact: N -->');
put('docs/stale.md', 'N is `4`. <!-- fact: N -->');
put('docs/plain.md', 'A page with no proofs.');
put('docs/old/notes.md', 'N was `2`.');
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8' });
// ---cut---
const all = docattest('check');
assert.equal(all.status, 1);
assert.match(all.stdout, /docs\/stale\.md\n {2}✗ L1 fact: expected 4, got 3 from N/);
assert.match(all.stdout, /, 1 uncovered, 1 frozen\n {2}uncovered: docs\/plain\.md\n/);
assert.doesNotMatch(all.stdout, /docs\/old/);

const one = docattest('check', 'docs/good.md');
assert.equal(one.status, 0);
assert.match(one.stdout, /results: 1 pass, 0 fail/);
assert.match(one.stdout, /coverage: 3 living pages, 2 covered .*, 1 uncovered, 1 frozen\nsurface/);
assert.deepEqual(JSON.parse(docattest('check', '--json', 'docs/good.md').stdout).coverage.frozen, ['docs/old/notes.md']);

const typo = docattest('check', 'docs/goood.md');
assert.equal(typo.status, 1);
assert.match(typo.stderr, /"docs\/goood\.md" matches no living page/);
```

## `--quiet` prints only what needs attention

`docattest check --quiet` is for whoever has to act on the result: a coding agent, a git
hook, a busy reader. It prints the results that `fail` or `error`, each under its page; the
warnings of the config and of a surface; the snapshots that are stale; and with `--base`,
what the change did to the proofs. Then the verdict, `ok` or `FAILED`. Passes, skips,
coverage and the surfaces' summaries are left out, so a project where everything holds
prints one line. The exit code is the same as without it.

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

Members are checked by `check` without pages, or individually from the repository root with
`--member <dir>` (which uses the member's configured `adapterCommand` and accepts optional
page paths inside that member). Named pages without `--member` are the top project's, and
`where`, `impact` and the other commands answer for one project: the one they run in.

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
assert.deepEqual(lines.slice(-5, -1), [
  'failures:',
  '  packages/stale/docs/n.md:1 fact: expected 4, got 3 from N',
  'projects: 3 checked, 1 failed (packages/stale)',
  'FAILED',
]);

// --member checks just one member (using its configured adapterCommand) and accepts page paths.
const onlyStale = docattest('.', 'check', '--member', 'packages/stale', 'docs/n.md');
assert.equal(onlyStale.status, 1);
assert.doesNotMatch(onlyStale.stdout, /packages\/good/);
assert.match(onlyStale.stdout, /expected 4, got 3 from N/);
const onlyGood = docattest('.', 'check', '--member', 'packages/good');
assert.equal(onlyGood.status, 0);
assert.doesNotMatch(onlyGood.stdout, /packages\/stale/);
const missingMember = docattest('.', 'check', '--member', 'packages/missing');
assert.equal(missingMember.status, 1);
assert.match(missingMember.stderr, /unknown member "packages\/missing"/);

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

## `--run-cited` runs the tests that citations name

`docattest check --run-cited` also runs the test each citation names, and a citation then
holds only when its test ran and passed ([citations](citations.md#running-the-cited-tests)
says what counts and what it costs). The language's adapter
runs the tests, so one that cannot makes `--run-cited` an error that says so, before
anything runs, whether or not the project has any citations.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-cli-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: 'lib' }, adapter: 'plain' }));
put('docs/a.md', 'It works. <!-- test: lib/a.test > "works" -->');
// An adapter that runs nothing: it answers hello, listing no optional method.
put('plain.cjs', `require('node:readline').createInterface({ input: process.stdin }).on('line', (line) => {
  const { id } = JSON.parse(line);
  const result = { protocol: 1, adapter: { name: 'plain', version: '1.0.0' }, blockLanguages: [],
    sourceFiles: { include: [], exclude: [] }, ignoredNames: [], capabilities: [] };
  process.stdout.write(JSON.stringify({ id, result }) + '\\n');
});`);
const env = { ...process.env, DOCATTEST_ADAPTER_PLAIN: JSON.stringify([process.execPath, join(root, 'plain.cjs')]) };
// ---cut---
const run = spawnSync(process.execPath, [cli, 'check', '--run-cited'], { cwd: root, env, encoding: 'utf8' });
assert.equal(run.status, 1);
assert.equal(run.stderr.trim(), 'docattest: --run-cited runs the tests that citations name, and the plain adapter does not run tests');
```

## Reports for other CI services

`docattest check --report <format>=<path>` also writes the report to a file in a format a
CI service reads, while the text, or the `--json` report, still prints as it would without
it. Give it once for each file; a relative path is from the working directory, and
directories that are missing are made.

- `junit` is JUnit XML, which GitLab, Jenkins, CircleCI, Azure Pipelines and most other
  services show as test results: a test suite for each page, a test case for each result,
  `<failure>` for a `fail`, `<error>` for an `error` and `<skipped>` for a skip. A change
  of `--base` that fails is a failing test case too; JUnit has no warnings, so a change
  that only warns is left out.
- `codequality` is a GitLab Code Quality report, which a merge request shows on its
  Reports tab, and in GitLab Ultimate beside the diff: one issue for each result that fails
  (`major`) or errors (`critical`), and each change of `--base` (`major` when it fails,
  `minor` when it warns). An issue's `fingerprint` names the same finding in every run,
  even after lines are added above it, so GitLab can tell new findings from old ones.

Members' results are in both files. Every path in them is from the top of the git
repository, also when the project is in a subdirectory or is a member, because that is how
a CI service finds the file; docattest asks git where the top is, so `--report` needs git
and a repository.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-reports-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
const project = (dir: string, page: string, members?: string[]) => {
  put(`${dir}/package.json`, JSON.stringify({ name: 'demo', type: 'module' }));
  put(`${dir}/src/index.ts`, 'export const N = 3;');
  put(`${dir}/docattest.config.json`, JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', members }));
  put(`${dir}/docs/n.md`, page);
};
const docattest = (cwd: string, ...args: string[]) => spawnSync(process.execPath, [cli, ...args], { cwd: join(root, cwd), encoding: 'utf8' });
spawnSync('git', ['init', '--quiet', root]);
// ---cut---
project('site', 'N is `3`. <!-- fact: N -->\n\nN is `4`. <!-- fact: N -->', ['packages/acme']);
project('site/packages/acme', 'N is `4`. <!-- fact: N -->');

const reports = ['--report', 'junit=reports/docattest.xml', '--report=codequality=reports/gl-code-quality.json'];
const run = docattest('site', 'check', '--json', ...reports);
assert.equal(run.status, 1);
assert.deepEqual(JSON.parse(run.stdout), JSON.parse(docattest('site', 'check', '--json').stdout));

const quality = JSON.parse(readFileSync(join(root, 'site/reports/gl-code-quality.json'), 'utf8'));
assert.deepEqual(quality.map((issue: { location: { path: string; lines: { begin: number } }; severity: string }) =>
  [issue.location.path, issue.location.lines.begin, issue.severity]), [
  ['site/docs/n.md', 3, 'major'],
  ['site/packages/acme/docs/n.md', 1, 'major'],
]);
assert.equal(quality[0].check_name, 'docattest-fact');
assert.equal(quality[0].description, 'docattest fact: expected 4, got 3 from N');
assert.notEqual(quality[0].fingerprint, quality[1].fingerprint);

const junit = readFileSync(join(root, 'site/reports/docattest.xml'), 'utf8');
assert.match(junit, /<testsuites name="docattest" tests="3" failures="2" errors="0" skipped="0">/);
const cases = [...junit.matchAll(/<testcase name="([^"]+)" classname="([^"]+)"/g)].map(([, name, page]) => `${page} ${name}`);
assert.deepEqual(cases, ['site/docs/n.md L1 fact', 'site/docs/n.md L3 fact', 'site/packages/acme/docs/n.md L1 fact']);
assert.match(junit, /<failure message="expected 4, got 3 from N" type="fact">/);

// Run again, the same findings have the same fingerprints.
docattest('site', 'check', ...reports);
assert.deepEqual(JSON.parse(readFileSync(join(root, 'site/reports/gl-code-quality.json'), 'utf8')), quality);

const unknown = docattest('site', 'check', '--report', 'xml=out.xml');
assert.equal(unknown.status, 1);
assert.match(unknown.stderr, /--report takes <format>=<path>, where <format> is junit or codequality, not "xml=out.xml"/);
```

## `init --skill` installs a single-language agent skill

`docattest init --skill` writes a lean, language-specific authoring skill tailored to the
project's `adapter` where the agent looks for skills. That place is the agent's
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
const root = mkdtempSync(join(tmpdir(), 'docattest-skill-'));
const home = mkdtempSync(join(tmpdir(), 'docattest-home-'));
writeFileSync(join(root, 'docattest.config.json'), JSON.stringify({ pages: ['README.md'], adapter: 'typescript' }));
const docattest = (...args: string[]) =>
  spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8', env: { ...process.env, HOME: home, USERPROFILE: home } });
// ---cut---
const installed = join(root, '.claude/skills/docattest/SKILL.md');

assert.equal(docattest('init', '--skill').status, 0);
const text = readFileSync(installed, 'utf8');
assert.match(text, /## TypeScript specifics/);
assert.doesNotMatch(text, /## Go specifics/);
assert.equal(docattest('init', '--skill').status, 0);

writeFileSync(installed, 'my own notes');
const refused = docattest('init', '--skill');
assert.equal(refused.status, 1);
assert.match(refused.stderr, /--force/);
assert.equal(readFileSync(installed, 'utf8'), 'my own notes');

assert.equal(docattest('init', '--skill', '--force').status, 0);
assert.equal(readFileSync(installed, 'utf8'), text);

assert.equal(docattest('init', '--skill', '--global').status, 0);
assert.equal(readFileSync(join(home, '.claude/skills/docattest/SKILL.md'), 'utf8'), text);

// The same text, where another agent looks for it.
assert.equal(docattest('init', '--skill', '--agent', 'gemini').status, 0);
assert.equal(readFileSync(join(root, '.gemini/skills/docattest/SKILL.md'), 'utf8'), text);
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
assert.match(run.stdout, /conformance \(typescript\): 14 of 14 checks hold/);
```

## `init` writes `docattest.config.json` and runs an initial gap scan

`docattest init` detects the project's language adapter, its Markdown and MDX pages, and its
default surface when one is present, writes `docattest.config.json` with its `$schema`
URL, and runs an initial `audit` scan to reveal candidate facts and drift right away
(`--no-audit` skips the scan). It refuses to overwrite an existing config unless `--force`
is given (or integration flags such as `--agents`, `--hooks`, `--git-hook`, `--skill` are
passed to wire tools into an existing project); `--dry-run` prints what it would write
without touching the disk.

```ts check
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const makeDir = (prefix: string) => mkdtempSync(join(tmpdir(), prefix));
const put = (root: string, rel: string, text: string) => {
  mkdirSync(join(root, rel, '..'), { recursive: true });
  writeFileSync(join(root, rel), text);
};
const docattest = (cwd: string, ...args: string[]) =>
  spawnSync(process.execPath, [cli, ...args], { cwd, encoding: 'utf8' });
// ---cut---
const tsRoot = makeDir('docattest-init-ts-');
put(tsRoot, 'package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put(tsRoot, 'tsconfig.json', '{}');
put(tsRoot, 'src/index.ts', 'export const PORT = 8080;');
put(tsRoot, 'README.md', '# Demo\n\nListens on port `8080`.\n');
put(tsRoot, 'docs/guide.mdx', '# Guide\n');

const preview = docattest(tsRoot, 'init', '--dry-run');
assert.equal(preview.status, 0);
assert.equal(existsSync(join(tsRoot, 'docattest.config.json')), false);

const created = docattest(tsRoot, 'init');
assert.equal(created.status, 0);
assert.match(created.stdout, /PORT/);
const cfg = JSON.parse(readFileSync(join(tsRoot, 'docattest.config.json'), 'utf8'));
assert.equal(cfg.$schema, 'https://docattest.github.io/docattest/schema/config.json');
assert.equal(cfg.adapter, 'typescript');
assert.deepEqual(cfg.pages, ['README.md', 'docs/**/*.mdx']);
assert.deepEqual(cfg.surfaces, { default: './src/index.ts' });

// Running init again refuses without --force.
assert.equal(docattest(tsRoot, 'init').status, 1);
assert.equal(docattest(tsRoot, 'init', '--force', '--no-audit').status, 0);

// --dry-run previews integrations too, while keeping an existing config untouched.
const dryIntegrations = docattest(tsRoot, 'init', '--agents', '--hooks', '--skill', '--force', '--dry-run');
assert.equal(dryIntegrations.status, 0);
assert.match(dryIntegrations.stdout, /keeping existing/);
assert.match(dryIntegrations.stdout, /would add to .*AGENTS\.md/);
assert.match(dryIntegrations.stdout, /would add to .*settings\.json/);
assert.match(dryIntegrations.stdout, /would install .*SKILL\.md/);
assert.equal(existsSync(join(tsRoot, 'AGENTS.md')), false);

// Per-subcommand --help prints focused help for that command.
const initHelp = docattest(tsRoot, 'init', '--help');
assert.equal(initHelp.status, 0);
assert.match(initHelp.stdout, /^usage: docattest init \[options\]/);
assert.match(initHelp.stdout, /--dry-run/);

// A repo with no language manifest gets the core-only "none" adapter and no surfaces.
const opsRoot = makeDir('docattest-init-ops-');
put(opsRoot, 'README.md', '# Ops\n');
put(opsRoot, 'Dockerfile', 'FROM scratch\nUSER 10001\n');
assert.equal(docattest(opsRoot, 'init', '--no-audit').status, 0);
const opsCfg = JSON.parse(readFileSync(join(opsRoot, 'docattest.config.json'), 'utf8'));
assert.equal(opsCfg.adapter, 'none');
assert.equal(opsCfg.surfaces, undefined);
```

## A merged command says which command took it over

`index`, `inventory`, `agents`, `hooks` and `skill` were merged into other commands. Asked
for one of them, docattest exits `1` and names the command that does its work now; any other
unknown name gets the usage.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' });
// ---cut---
const index = docattest('index');
assert.equal(index.status, 1);
assert.equal(index.stderr, 'docattest: "index" was merged into another command: run docattest where\n');
assert.match(docattest('hooks', 'install').stderr, /run docattest init --hooks/);
assert.match(docattest('bogus').stderr, /^docattest: unknown command "bogus"\n\nusage:/);
```
