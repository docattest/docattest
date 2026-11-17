---
title: Changes against a base
---

# Changes against a base

`check` judges the pages and the code as they are. That leaves three ways to make a
failing build pass while the docs stay wrong: delete the proof and keep the sentence,
freeze the page, or weaken the proof until it asserts less than the prose says.
`check({ base })`, and `docattest check --base <ref>` on the command line, also judge what
the change did to the attestations that existed at the base, and add `changes` to the
report.

## Truth depends on the current state; a weakening depends on the change

A verdict depends only on the current page and the current code, so the same files give
the same verdict whatever came before. Removing a proof leaves a page that is consistent
with the code, just unproven: nothing in the current state can tell that apart from a
page that was never proven. A weakening is a property of a change, so only a comparison
with the base can see it. Without `base` the report has no `changes`; with it, the same
files can fail.

```ts check
import { createProject } from 'docattest';

const files = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const LIMIT = 5;',
};
const base = { files: { ...files, 'src/index.ts': 'export const LIMIT = 3;', 'docs/retry.md': 'A call is tried at most `3` times. <!-- fact: LIMIT -->\n' } };
const project = createProject({ files: { ...files, 'docs/retry.md': 'A call is tried at most `3` times.\n' } });

const alone = await project.check();
assert.equal(alone.exitCode, 0);
assert.equal(alone.changes, undefined);

const judged = await project.check({ base });
assert.equal(judged.exitCode, 1);
assert.deepEqual(judged.changes?.map((c) => [c.type, c.page, c.line, c.severity]), [['orphaned-claim', 'docs/retry.md', 1, 'fail']]);
```

Nothing is stored to make this work: the base is read from git when it is asked for, and
nothing is written.

## A claim that survives while its proof disappears fails

Deleting a fact directive, an example, a cases directive or a doc-comment example while
the sentence it proved stays on the page is an `orphaned-claim`, with severity `fail`, at
the line where the sentence is now. It sets exit code 1 even though every remaining
attestation passes.

```ts check
import { createProject } from 'docattest';

const files = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const ok = (value: unknown) => value;',
};
const example = "```ts check\nimport { ok } from 'docattest-fixture';\nassert.equal(ok(7), 7);\n```\n";
const base = { files: { ...files, 'docs/ok.md': `# ok\n\nThe value comes back unchanged.\n\n${example}` } };

const report = await createProject({ files: { ...files, 'docs/ok.md': '# ok\n\nSome new prose.\n\nThe value comes back unchanged.\n' } }).check({ base });
assert.equal(report.exitCode, 1);
const [change, ...rest] = report.changes ?? [];
assert.deepEqual(rest, []);
assert.equal(change?.type, 'orphaned-claim');
assert.equal(change.line, 5);
assert.match(change.message, /"The value comes back unchanged\." is still on the page, but the example that proved it/);
```

## Each proof is judged on its own

A paragraph or list item can hold several claims, each with its own proof. Deleting one of
those proofs orphans its claim even when the others stay: another fact on a different
sentence, a block after the paragraph, a `test:` citation on the same item. The message
quotes the sentence that lost its proof, read as a reader sees it, without the directive.
A proof that takes the sentence over in the same change, such as a new example, or another
fact on the same literal, keeps it proven.

```ts check
import { createProject } from 'docattest';

const files = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': "export const CLIENT = 'acme';\nexport const RETRY = 3;",
};
const block = "```ts check\nimport { RETRY } from 'docattest-fixture';\nassert.ok(RETRY > 0);\n```\n";
const changesFor = async (before: string, after: string) =>
  (await createProject({ files: { ...files, 'docs/c.md': after } }).check({ base: { files: { ...files, 'docs/c.md': before } } })).changes ?? [];

const two = await changesFor(
  'The client is `acme` <!-- fact: CLIENT -->, always. A call is tried `3` times. <!-- fact: RETRY -->\n',
  'The client is `acme`, always. A call is tried `3` times. <!-- fact: RETRY -->\n',
);
assert.deepEqual(two.map((c) => [c.type, c.line]), [['orphaned-claim', 1]]);
assert.match(two[0].message, /^"The client is `acme`, always\." is still on the page/);

const underBlock = await changesFor(`At most \`3\` <!-- fact: RETRY --> tries.\n\n${block}`, `At most \`3\` tries.\n\n${block}`);
assert.deepEqual(underBlock.map((c) => c.type), ['orphaned-claim']);

const takenOver = await changesFor(
  'A call is tried `3` times. <!-- fact: RETRY -->\n',
  "A call is tried `3` times.\n\n```ts check\nimport { RETRY } from 'docattest-fixture';\nassert.equal(RETRY, 3);\n```\n",
);
assert.deepEqual(takenOver, []);
```

## Ordinary edits are not changes

Deleting a sentence together with its proof, moving a section, adding prose above it,
lightly rewording an attested sentence while keeping its proof, renaming a page with its
attestations, deleting a page and adding attestations all leave `changes` empty.
Attestations are matched by what they are and what they defend, never by line number.

```ts check
import { createProject } from 'docattest';

const files = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const LIMIT = 3;\nexport const ok = (value: unknown) => value;',
};
const fact = 'A call is tried at most `3` times. <!-- fact: LIMIT -->';
const example = "```ts check\nimport { ok } from 'docattest-fixture';\nassert.equal(ok(7), 7);\n```";
const base = { files: { ...files, 'docs/retry.md': `# Retry\n\n${fact}\n\nThe value comes back unchanged.\n\n${example}\n` } };
const changesFor = async (pages: Record<string, string>) => (await createProject({ files: { ...files, ...pages } }).check({ base })).changes;

assert.deepEqual(await changesFor({ 'docs/retry.md': `# Retry\n\nThe value comes back unchanged.\n\n${example}\n` }), []);
assert.deepEqual(await changesFor({ 'docs/retry.md': `# Retry\n\nNew prose.\n\nWhen it succeeds, the value is returned as is.\n\n${example}\n\n${fact}\n` }), []);
assert.deepEqual(await changesFor({ 'docs/retrying.md': `# Retry\n\n${fact}\n\nThe value comes back unchanged.\n\n${example}\n` }), []);
assert.deepEqual(await changesFor({}), []);
```

## When it is unclear whether the claim survived, nothing is said

A false alarm costs more than a miss: a guard that fires on an honest edit gets switched
off. A sentence counts as surviving when it is still there word for word, or lightly
reworded with the same literals and exactly one candidate. A sentence rewritten
substantially in the same change that removed its proof is not reported.

```ts check
import { createProject } from 'docattest';

const files = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const LIMIT = 3;',
};
const base = { files: { ...files, 'docs/retry.md': 'A call is tried at most `3` times. <!-- fact: LIMIT -->\n' } };
const changesFor = async (page: string) => (await createProject({ files: { ...files, 'docs/retry.md': page } }).check({ base })).changes ?? [];

assert.deepEqual((await changesFor('A call is attempted at most `3` times.\n')).map((c) => c.type), ['orphaned-claim']);
assert.deepEqual(await changesFor('Retrying is configurable; see the options.\n'), []);
```

## Freezing an attested page is a warning

A page that carried attestations at the base and is no longer living, whether through
`frozen` in the config, frontmatter `docattest: false`, no longer matching `pages`, or a
move to somewhere unchecked, is a `frozen-page` change with severity `warning` and line 1.
Warnings never change the exit code.

```ts check
import { createProject } from 'docattest';

const config = { pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' };
const page = 'N is `3`. <!-- fact: N -->\n';
const files = { 'docattest.config.json': JSON.stringify(config), 'src/index.ts': 'export const N = 3;', 'docs/n.md': page };
const base = { files };
const changesFor = async (edits: Record<string, string>) => (await createProject({ files: { ...files, ...edits } }).check({ base })).changes ?? [];

const byConfig = await changesFor({ 'docattest.config.json': JSON.stringify({ ...config, frozen: ['docs/n.md'] }) });
assert.deepEqual(byConfig.map((c) => [c.type, c.page, c.line, c.severity]), [['frozen-page', 'docs/n.md', 1, 'warning']]);

const byFrontmatter = await changesFor({ 'docs/n.md': `---\ndocattest: false\n---\n${page}` });
assert.deepEqual(byFrontmatter.map((c) => c.type), ['frozen-page']);
```

## A downgraded example is a warning

A `check` block that became `compile`, or lost every assertion, is a `downgrade` at the
block's line now.

```ts check
import { createProject } from 'docattest';

const files = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const ok = (value: unknown) => value;',
};
const block = (tag: string, body: string) => `The value comes back unchanged.\n\n\`\`\`ts ${tag}\nimport { ok } from 'docattest-fixture';\n${body}\n\`\`\`\n`;
const base = { files: { ...files, 'docs/ok.md': block('check', 'assert.equal(ok(7), 7);') } };
const changesFor = async (page: string) => (await createProject({ files: { ...files, 'docs/ok.md': page } }).check({ base })).changes ?? [];

const compiled = await changesFor(block('compile', 'assert.equal(ok(7), 7);'));
assert.deepEqual(compiled.map((c) => [c.type, c.line, c.severity]), [['downgrade', 3, 'warning']]);

const unasserted = await changesFor(block('check', 'ok(7);'));
assert.deepEqual(unasserted.map((c) => [c.type, c.line]), [['downgrade', 3]]);
```

## A fact whose expression changed while its literal did not is weakened

The literal on the page is what the reader trusts; an expression rewritten around an
unchanged literal may now assert less. It is a `weakened` change at the fact's line, a
warning. An expression that changed only because the code it named was renamed is not.

```ts check
import { createProject } from 'docattest';

const config = JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' });
const base = { files: { 'docattest.config.json': config, 'src/index.ts': 'export const LIMIT = 3;', 'docs/n.md': 'At most `3` tries. <!-- fact: LIMIT -->\n' } };
const changesFor = async (src: string, page: string) => (await createProject({ files: { 'docattest.config.json': config, 'src/index.ts': src, 'docs/n.md': page } }).check({ base })).changes ?? [];

const clamped = await changesFor('export const LIMIT = 5;', 'At most `3` tries. <!-- fact: Math.min(LIMIT, 3) -->\n');
assert.deepEqual(clamped.map((c) => [c.type, c.line, c.severity]), [['weakened', 1, 'warning']]);

const renamed = await changesFor('export const MAX_TRIES = 3;', 'At most `3` tries. <!-- fact: MAX_TRIES -->\n');
assert.deepEqual(renamed, []);
```

The kinds of change are: <!-- fact: CHANGE_TYPES -->
- `orphaned-claim`
- `frozen-page`
- `downgrade`
- `weakened`

## On the command line the base is a git ref

`docattest check --base <ref>` reads the base with git from the repository the project is
in, at that ref, and compares the working tree with it: `--base origin/main` in a pull
request. With `--json` the report carries `changes`; as text it ends with a "changes since
the base" section. A directory that is not in a git repository, or a ref that does not
exist, is an error with a message and exit code 1, before anything runs.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const env = { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null', GIT_AUTHOR_NAME: 'd', GIT_AUTHOR_EMAIL: 'd@example.invalid', GIT_COMMITTER_NAME: 'd', GIT_COMMITTER_EMAIL: 'd@example.invalid' };
const root = mkdtempSync(join(tmpdir(), 'docattest-base-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
const run = (cmd: string, ...args: string[]) => spawnSync(cmd, args, { cwd: root, env, encoding: 'utf8' });
const docattest = (...args: string[]) => run(process.execPath, cli, ...args);
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('src/index.ts', 'export const N = 3;');
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
// ---cut---
assert.equal(docattest('check', '--base', 'HEAD').status, 1);
assert.match(docattest('check', '--base', 'HEAD').stderr, /is not in a git repository/);

put('docs/n.md', 'N is `3`. <!-- fact: N -->\n');
run('git', 'init', '-q');
run('git', 'add', '-A');
run('git', 'commit', '-q', '-m', 'base');
put('docs/n.md', 'N is `3`.\n');

const json = docattest('check', '--base', 'HEAD', '--json');
assert.equal(json.status, 1);
assert.deepEqual(JSON.parse(json.stdout).changes.map((c: { type: string; line: number }) => [c.type, c.line]), [['orphaned-claim', 1]]);

const text = docattest('check', '--base', 'HEAD');
assert.equal(text.status, 1);
assert.match(text.stdout, /changes since the base: 1 fail, 0 warnings\n {2}✗ docs\/n\.md:1 orphaned-claim/);

const unknown = docattest('check', '--base', 'v9');
assert.equal(unknown.status, 1);
assert.match(unknown.stderr, /^docattest: unknown git ref "v9"/);
```

## An in-memory project takes the base's files

`check({ base: { files } })` compares with a base given as files, which is how every example
on this page works: the base is committed with git in the project's temporary directory,
so git must be installed. A git ref needs a project on disk, and a project on disk takes
only a git ref. A base without a `docattest.config.json` had nothing to protect, so there
are no changes.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const N = 3;',
  },
});

await assert.rejects(project.check({ base: 'main' }), /a git ref as base needs a project on disk/);
assert.deepEqual((await project.check({ base: { files: { 'README.md': 'before docattest' } } })).changes, []);
```
