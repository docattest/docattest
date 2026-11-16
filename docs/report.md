---
title: The report
---

# The report

`check()` returns one object: results in page-then-line order, page coverage, surface
coverage, snapshot ages and an exit code. The CLI prints it as text or, with `--json`,
exactly this object. Nothing is written to the project by a check.

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

## The text report

`docattest check` without `--json` prints the report as text: failures by page and line,
then one summary line each for results, page coverage, surfaces and snapshots. A surface
symbol that no page attests is listed as not referenced by any page, which is normal for
helpers and types; it is a backlog, not a failure. A count is followed by its noun in the
singular for one and the plural otherwise (`1 living page`, `2 symbols`). The text is the
command line's; the programmatic API returns the report object it is printed from. Here it
is for a small project, [docs/.fixtures/text-report](.fixtures/text-report), whose one page
says `N` is 4 where the code says 3:

```console check cwd=docs/.fixtures/text-report
$ docattest check
docs/a.md
  ✗ L3 fact: expected 4, got 3 from N
results: 0 pass, 1 fail, 0 error, 0 skipped
coverage: 1 living page, 1 covered (1 attestation, 0 weak), 0 uncovered, 0 frozen
surface default: 2 symbols, 1 attested, 1 not referenced by any page
  shop → src/index.ts (shop)
snapshots: none
FAILED
[exit 1]
```
