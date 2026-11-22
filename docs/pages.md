---
title: Pages, living and frozen
---

# Pages, living and frozen

`docattest.config.json` names the pages to check. Pages matched by `pages` and not by
`frozen` are *living* and are parsed for attestations. Frozen pages are listed in the
report and never parsed. A living page with nothing to run is *uncovered*: reported, not
failed, so adoption can be incremental, unless it is marked as *prose*, a page with nothing
to prove.

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

## `frozen` leaves pages out, not a `!` glob

Every list of globs in the config adds files: a file is a page when any glob of `pages`
matches it. A glob that starts with `!` would match every file it does not name, so the
config refuses one and says to list the pages to leave out in `frozen`.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/*.md', '!docs/old.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/a.md': 'N is `3`. <!-- fact: N -->',
    'docs/old.md': 'N was `2`. <!-- fact: N -->',
  },
});

await assert.rejects(project.check(), /"pages" has "!docs\/old\.md": .*"frozen" leaves pages out of the check/);
```

## What git ignores is never a page

In a git work tree, the files git ignores and does not track are left out of the project:
generated reference pages, build output, installed packages. They are never pages, and
never sources either. A file git tracks is the project's own whatever `.gitignore` says, and
outside a work tree every file is, as is every file of a project in a directory git ignores
as a whole.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-gitignore-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('src/index.ts', 'export const N = 3;');
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('docs/guide.md', 'N is `3`. <!-- fact: N -->');
put('docs/api/generated.md', '# Generated reference');
put('.gitignore', 'docs/api/\n');
const living = () => JSON.parse(spawnSync(process.execPath, [cli, 'check', '--json', '--root', root], { encoding: 'utf8' }).stdout).coverage;
// ---cut---
assert.deepEqual(living().uncovered, ['docs/api/generated.md']);

spawnSync('git', ['init', '--quiet'], { cwd: root });
assert.equal(living().living, 1);
assert.deepEqual(living().uncovered, []);
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

## A page with nothing to prove is prose

Some living pages hold only guidance: an index of the docs, a contributing guide. Calling
one uncovered says a proof is missing when none could be written, and freezing it says
"never check this", which is not what is meant either. `prose` in the config, globs of
pages as `frozen` takes, or `docattest: prose` in a page's frontmatter, marks such a page.
It stays living: a directive on it still runs, and `audit` still reads it. It is never
reported as uncovered: the report object lists it under `prose`, which it has only when some
page is prose, and the coverage line counts it apart from the uncovered and the frozen.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], prose: ['docs/README.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;\nexport const REGIONS = ['eu', 'us', 'uk'];`,
    'docs/README.md': '# Docs\n\nWe ship to:\n\n- `eu`\n- `us`\n- `uk`\n',
    'docs/contributing.md': '---\ndocattest: prose\n---\n# Contributing\n\nOpen an issue first.\n',
    'docs/guide.md': 'N is `3`. <!-- fact: N -->',
    'docs/todo.md': 'Not written yet.',
  },
});

const report = await project.check();
assert.deepEqual(report.coverage.prose, ['docs/README.md', 'docs/contributing.md']);
assert.deepEqual(report.coverage.uncovered, ['docs/todo.md']);
assert.equal(report.coverage.living, 4);
assert.equal(report.coverage.covered, 1);

const { suggestions } = await project.audit();
assert.deepEqual(suggestions.map((s) => [s.page, s.line]), [['docs/README.md', 5]]);
```

A page marked as prose that gains a proof is covered like any other page, and its marker
becomes a warning, since it now says something false about the page. A warning never fails
the check.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], prose: ['docs/guide.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/guide.md': 'N is `4`. <!-- fact: N -->',
  },
}).check();

assert.equal(report.results[0].status, 'fail');
assert.equal(report.coverage.covered, 1);
assert.equal(report.coverage.prose, undefined);
assert.deepEqual(report.warnings, ['docs/guide.md is marked as prose but has 1 attestation: take it out of "prose" in the config']);
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

## MDX pages use `{/* … */}` directives

Pages ending in `.mdx` use JSX comments (`{/* fact: ... */}`, `{/* cases: ... */}`,
`{/* test: ... */}`) instead of HTML comments, which MDX rejects. Top-level `import` /
`export` lines and standalone JSX wrapper tags (`<Tabs>`, `<TabItem>`, `<Callout>`) are
ignored while preserving exact line numbers.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({
      pages: ['docs/**/*.mdx'],
      surfaces: { default: './src/index.ts' },
      adapter: 'typescript',
    }),
    'src/index.ts': "export const RETRIES = 3;\nexport const ENV_KEYS = ['API_TOKEN', 'REGION'];",
    'docs/retries.mdx': [
      "import { Tabs, TabItem } from '@astrojs/starlight/components';",
      '',
      '# Retries',
      '',
      '<Tabs>',
      '<TabItem label="Defaults">',
      'A call is tried at most `3` times. {/* fact: RETRIES */}',
      '</TabItem>',
      '</Tabs>',
      '',
      '{/* fact: ENV_KEYS */}',
      '- `API_TOKEN`',
      '- `REGION`',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual(
  report.results.map((r) => [r.line, r.status]),
  [[7, 'pass'], [11, 'pass']],
);
```

## Core-only mode (`adapter: "none"`) and surface-optional configs

Both `adapter` and `surfaces` are optional in `docattest.config.json`. Omitting `adapter`
(or setting `"adapter": "none"`) runs entirely in the Rust core with no language runtime:
`matched()`, `manifestField()`, `file()`, `manifest()`, `snapshot()`, and `console check`
all work out of the box. A language adapter can also omit `surfaces` when pages only check
manifests, files, console blocks, citations, or standalone examples.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-core-only-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('Dockerfile', 'FROM scratch\nUSER 10001\n');
put('package.json', JSON.stringify({ name: 'ops', version: '2.4.0' }));
put('docattest.config.json', JSON.stringify({ pages: ['README.md'] }));
put('README.md', [
  'Version `2.4.0`. <!-- fact: manifest("package.json").version -->',
  'Runs as user `10001`. <!-- fact: matched("Dockerfile", "^USER (\\\\d+)$") -->',
  '',
  '```console check',
  '$ echo ready',
  'ready',
  '```',
].join('\n'));
// ---cut---
const coreOnly = spawnSync(process.execPath, [cli, 'check', '--json', '--root', root], { encoding: 'utf8' });
assert.equal(coreOnly.status, 0);
assert.deepEqual(JSON.parse(coreOnly.stdout).results.map((r: { status: string }) => r.status), ['pass', 'pass', 'pass']);

// With a language adapter and no surfaces, language expressions over manifests work too.
put('docattest.config.json', JSON.stringify({ pages: ['README.md'], adapter: 'typescript' }));
put('README.md', 'Major version `2`. <!-- fact: manifest("package.json").version.split(".")[0] -->\n');
const surfaceOptional = spawnSync(process.execPath, [cli, 'check', '--json', '--root', root], { encoding: 'utf8' });
assert.equal(surfaceOptional.status, 0);
assert.equal(JSON.parse(surfaceOptional.stdout).results[0].status, 'pass');
```
