---
title: Pages, living and frozen
---

# Pages, living and frozen

`docattest.config.json` names the pages to check. Pages matched by `pages` and not by
`frozen` are *living* and are parsed for attestations. Frozen pages are listed in the
report and never parsed. A living page with nothing to run is *uncovered*: reported, not
failed, so adoption can be incremental.

## A living page with no attestations is uncovered, not failing

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/intro.md': '# Intro\n\nJust prose. Nothing to prove here.\n',
  },
}).check();

assert.deepEqual(report.coverage.uncovered, ['docs/intro.md']);
assert.equal(report.coverage.living, 1);
assert.equal(report.coverage.covered, 0);
assert.equal(report.results.length, 0);
assert.equal(report.exitCode, 0);
```

## Frozen pages are never parsed

A false fact on a frozen page produces no result. The page is listed under `frozen`.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], frozen: ['docs/decisions/**'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const LIMIT = 5;`,
    'docs/decisions/001.md': 'The limit is `3`. <!-- fact: LIMIT -->',
  },
}).check();

assert.deepEqual(report.coverage.frozen, ['docs/decisions/001.md']);
assert.equal(report.coverage.living, 0);
assert.equal(report.results.length, 0);
assert.equal(report.exitCode, 0);
```

## A page can freeze itself with frontmatter

`docattest: false` in the frontmatter freezes one page without touching the config.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const LIMIT = 5;`,
    'docs/history.md': '---\ndocattest: false\n---\n# History\n\nThe limit was `3`. <!-- fact: LIMIT -->\n',
  },
}).check();

assert.deepEqual(report.coverage.frozen, ['docs/history.md']);
assert.equal(report.results.length, 0);
```

## A check can be limited to some pages

`check({ pages })`, and `docattest check <page or glob>...` on the command line, runs only
the attestations on the matching pages. Patterns are paths or globs relative to the project
root. Coverage still describes every living page, so checking one page never makes the
rest look uncovered.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/a.md': 'N is `3`. <!-- fact: N -->',
    'docs/b.md': 'N is `4`. <!-- fact: N -->',
    'docs/c.md': 'Only prose.',
  },
});

const one = await project.check({ pages: ['docs/a.md'] });
assert.deepEqual(one.results.map((r) => r.page), ['docs/a.md']);
assert.equal(one.exitCode, 0);
assert.equal(one.coverage.living, 3);
assert.equal(one.coverage.covered, 2);
assert.deepEqual(one.coverage.uncovered, ['docs/c.md']);

const globbed = await project.check({ pages: ['docs/[ab].md'] });
assert.deepEqual(globbed.results.map((r) => r.page), ['docs/a.md', 'docs/b.md']);
assert.equal(globbed.exitCode, 1);
```

A pattern that matches no living page is an error, so a typo cannot pass as an empty check.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], frozen: ['docs/old/**'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/a.md': 'N is `3`. <!-- fact: N -->',
    'docs/old/b.md': 'N was `2`. <!-- fact: N -->',
  },
});

await assert.rejects(project.check({ pages: ['docs/nope.md'] }), /"docs\/nope\.md" matches no living page/);
await assert.rejects(project.check({ pages: ['docs/old/b.md'] }), /matches only frozen pages/);
```

## Doc comments are pages too

An `@example` in a doc comment on a source file matched by `sources` is an `example`
when its first line is `// check` or `// compile`. The result's page is the source file
and its symbol is the documented export.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: [], sources: ['src/**/*.ts'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': [
      '/**',
      ' * Doubles a number.',
      ' * @example',
      ' * // check',
      " * import { double } from 'docattest-fixture';",
      ' * assert.equal(double(2), 4);',
      ' */',
      'export function double(n: number) { return n * 2; }',
    ].join('\n'),
  },
}).check();

assert.equal(report.results.length, 1);
assert.equal(report.results[0].page, 'src/index.ts');
assert.equal(report.results[0].kind, 'example');
assert.equal(report.results[0].status, 'pass');
assert.deepEqual(report.results[0].symbols, ['double']);
```

In an in-memory project the default surface is importable under the name
`docattest-fixture`; in a real project it is the name in `package.json`, or the `import`
the surface names (see [surfaces](surfaces.md)).
