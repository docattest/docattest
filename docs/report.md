---
title: The report
---

# The report

`check()` returns one object: results in page-then-line order, page coverage, surface
coverage, snapshot ages and an exit code. The CLI prints it as text or, with `--json`,
exactly this object; with `--report`, it also writes it to a file in a format another CI
service reads ([the command line](cli.md#reports-for-other-ci-services)). Nothing else is
written to the project by a check.

## Exit code is 1 only for `fail` or `error`

`skipped` results, uncovered pages and snapshot warnings leave it at 0.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': `export const N = 3;`,
};

const clean = await createProject({ files: { ...base, 'docs/a.md': 'N is `3`. <!-- fact: N -->' } }).check();
assert.equal(clean.exitCode, 0);

const failing = await createProject({ files: { ...base, 'docs/a.md': 'N is `4`. <!-- fact: N -->' } }).check();
assert.equal(failing.exitCode, 1);

const erroring = await createProject({ files: { ...base, 'docs/a.md': '<!-- fact: N -->\n\nNo target here.' } }).check();
assert.equal(erroring.exitCode, 1);

const skipping = await createProject({ files: { ...base, 'docs/a.md': "```ts check network\nawait fetch('https://example.com');\n```" }, env: { network: false } }).check();
assert.equal(skipping.exitCode, 0);

const uncovered = await createProject({ files: { ...base, 'docs/a.md': 'Only prose.' } }).check();
assert.equal(uncovered.exitCode, 0);
```

## Results are ordered by page, then line

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/b.md': 'N is `3`. <!-- fact: N -->',
    'docs/a.md': 'Intro.\n\nN is `3`. <!-- fact: N -->\n\nN is `3`. <!-- fact: N -->',
  },
}).check();

assert.deepEqual(report.results.map((r) => `${r.page}:${r.line}`), ['docs/a.md:3', 'docs/a.md:5', 'docs/b.md:1']);
```

## A check writes nothing

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/a.md': 'N is `3`. <!-- fact: N -->\n\n```ts check\nassert.ok(true);\n```',
  },
});
await project.check();
await project.check();
assert.deepEqual(project.writes(), []);
```

## The same inputs give the same report

```ts check
import { createProject } from 'docattest';

const input = {
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/a.md': 'N is `4`. <!-- fact: N -->\n\n```ts check\nassert.equal(1, 2);\n```',
  },
};
const one = await createProject(input).check();
const two = await createProject(input).check();
assert.deepEqual(one, two);
```

The number of workers never changes the report, because results are collected and then
ordered by page and line, not by when they finish.

```ts check
import { createProject } from 'docattest';

const config = (workers: number) => JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', runtime: { workers } });
const pages = {
  'src/index.ts': `export const N = 3;`,
  'docs/a.md': "```ts check\nawait new Promise((r) => setTimeout(r, 30));\nassert.equal(1, 1);\n```\n\nN is `3`. <!-- fact: N -->",
  'docs/b.md': "```ts check\nassert.equal(2, 3);\n```\n\n```ts check\nassert.ok(true);\n```",
};
const serial = await createProject({ files: { 'docattest.config.json': config(1), ...pages } }).check();
const parallel = await createProject({ files: { 'docattest.config.json': config(4), ...pages } }).check();
assert.deepEqual(parallel, serial);
assert.deepEqual(serial.results.map((r) => `${r.page}:${r.line}`), ['docs/a.md:1', 'docs/a.md:6', 'docs/b.md:1', 'docs/b.md:5']);
```

## What the config names to no effect is a warning

A glob of `pages` or `sources` that matches no file, and a `setup` module that is not there,
or that nothing reads because no living page has a cases table or a fact, each do nothing,
and a typo there would check less than was meant without a word. Each is a warning under
`warnings` in the report, a key there only when there is one, and a `warning:` line in the
text after the coverage, even with `--quiet`. Like every warning, it leaves the exit code
as it is.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md', 'guide/*.md'], setup: 'docs/docattest.setup.ts', surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/a.md': 'N is `3`. <!-- fact: N -->',
  },
}).check();

assert.deepEqual(report.warnings, [
  '"pages" has "guide/*.md", which matches no file',
  '"setup" is "docs/docattest.setup.ts", which is not there',
]);
assert.equal(report.exitCode, 0);
```

## The text report

`docattest check` without `--json` prints the report as text: failures by page and line,
then one summary line each for results, page coverage, surfaces and snapshots. A surface
symbol that no page attests is counted as not attested by any page, which is normal for
helpers and types; it is a backlog, not a failure. The ones the config says are not meant to
be documented are counted apart ([duality](duality.md#symbols-not-meant-to-be-documented-are-counted-apart)). A count is followed by its noun in the
singular for one and the plural otherwise (`1 living page`, `2 symbols`). The text is the
command line's; the programmatic API returns the report object it is printed from. The
coverage line counts the frozen pages and names the uncovered ones, as many to a line as fit
in 100 columns; after a check of named pages it names none, since the rest of the project
is not what was asked about. The report object lists both, always. Here it
is for a small project, [docs/.fixtures/text-report](.fixtures/text-report), whose one page
says `N` is 4 where the code says 3:

```console check cwd=docs/.fixtures/text-report
$ docattest check
docs/a.md
  ✗ L3 fact: expected 4, got 3 from N
results: 0 pass, 1 fail, 0 error, 0 skipped
coverage: 1 living page, 1 covered (1 attestation: 1 proof, 0 of them weak, and 0 citations), 0 uncovered, 0 frozen
surface default: 2 symbols, 1 attested, 1 not attested by any page
  shop → src/index.ts (shop)
snapshots: none
FAILED
[exit 1]
```

In a project with [prose pages](pages.md#a-page-with-nothing-to-prove-is-prose), pages
marked as having nothing to prove, the coverage line counts them too, between the
uncovered and the frozen, and never names them.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-prose-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], prose: ['docs/README.md', 'docs/contributing.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', 'export const N = 3;');
put('docs/README.md', '# Docs');
put('docs/contributing.md', '# Contributing');
put('docs/a.md', 'N is `3`. <!-- fact: N -->');
put('docs/todo.md', 'Not written yet.');
// ---cut---
const out = spawnSync(process.execPath, [cli, 'check', '--root', root], { encoding: 'utf8' }).stdout;
assert.match(out, /, 1 covered \(.*\), 1 uncovered, 2 prose, 0 frozen\n {2}uncovered: docs\/todo\.md\n/);
```

## Coverage counts proofs and citations apart

The coverage line gives the number of attestations on the living pages, a cases table
counting one for each row, and then splits it in two. Proofs are what
docattest runs or evaluates itself: examples, facts and table rows. A weak example, a
`check` block that asserts nothing, is still a proof, so the weak ones are counted among
the proofs. Citations only check that the test they name still exists: they say where to
look, not that the sentence holds ([citations](citations.md#what-a-citation-does-not-prove)).
All three counts are always printed, `0` too, and the report object carries them as
`attestations`, `proofs`, `weak` and `cited`, with `proofs` and `cited` adding up to
`attestations`.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'test/n.test.ts': "it('is three', () => {});",
    'docs/a.md': 'N is `3`. <!-- fact: N -->\n\n```ts check\nconst n = 3;\n```\n\nIt is tested. <!-- test: test/n.test.ts > "is three" -->',
  },
}).check();

const { attestations, proofs, weak, cited } = report.coverage;
assert.deepEqual({ attestations, proofs, weak, cited }, { attestations: 3, proofs: 2, weak: 1, cited: 1 });
```
