---
title: Surfaces
---

# Surfaces

A surface is what examples and facts are allowed to import: normally the published
package. `surfaces` in the config names each one. The report says where each surface
resolved, so a page is never checked against code nobody meant to check.

## A surface is a package name, a path, or both

A bare specifier resolves the way Node would from the project root. A `./path` is a file in
the project, imported under the package name from `package.json`. The object form names
both: examples import `import`, and it resolves to `path`. That lets a monorepo keep one
config at its root and still point a package's pages at that package's build.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({
      pages: ['docs/**/*.md'],
      surfaces: { default: { import: '@acme/client', path: 'packages/client/src/index.ts' } },
      adapter: 'typescript',
    }),
    'packages/client/src/index.ts': `export const DEFAULT_QUEUE = 'batch-large';`,
    'docs/client.md': [
      'Jobs go to `batch-large` unless told otherwise. <!-- fact: DEFAULT_QUEUE -->',
      '',
      '```ts check',
      "import { DEFAULT_QUEUE } from '@acme/client';",
      "assert.equal(DEFAULT_QUEUE, 'batch-large');",
      '```',
    ].join('\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['pass', 'pass']);
assert.equal(report.surfaces.default.resolution?.import, '@acme/client');
assert.equal(report.surfaces.default.resolution?.entry, 'packages/client/src/index.ts');
```

## The report says where each surface resolved

Each surface lists the entry file it resolved to and the package that file belongs to: the
name and version from the nearest `package.json`, and `display`, the two as the language
writes them, which is what the text report prints (`demo@1.2.3` from the TypeScript adapter,
`demo 1.2.3` from the Python one).

```ts check
import { createProject } from 'docattest';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = mkdtempSync(join(tmpdir(), 'docattest-surface-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', version: '1.2.3', type: 'module' }));
put('src/index.ts', 'export const N = 3;');
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('docs/n.md', 'N is `3`. <!-- fact: N -->');
// ---cut---
const report = await createProject({ root }).check();

assert.deepEqual(report.surfaces.default.resolution, {
  import: 'demo',
  entry: 'src/index.ts',
  package: { name: 'demo', version: '1.2.3', display: 'demo@1.2.3' },
});
const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const text = spawnSync(process.execPath, [cli, 'check'], { cwd: root, encoding: 'utf8' }).stdout;
assert.match(text, /demo → src\/index\.ts \(demo@1\.2\.3\)/);
```

## A stale installed copy of a workspace package is a warning

In a workspace, a bare-specifier surface usually resolves through a symlink to the
workspace package. When it resolves instead to a real directory under `node_modules` whose
version differs from the workspace package of the same name, the pages are being checked
against an old copy. That is reported as a warning: it never changes the exit code, but it
says which copy was checked.

```ts check
import { createProject } from 'docattest';
import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = mkdtempSync(join(tmpdir(), 'docattest-workspace-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
const pkg = (version: string) => JSON.stringify({ name: '@x/client', version, type: 'module', main: 'index.js' });
put('package.json', JSON.stringify({ name: 'repo', private: true, workspaces: ['packages/*'] }));
put('packages/client/package.json', pkg('3.2.0'));
put('packages/client/index.js', "export const VERSION = '3.2.0';");
put('node_modules/@x/client/package.json', pkg('3.0.1'));
put('node_modules/@x/client/index.js', "export const VERSION = '3.0.1';");
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: '@x/client' }, adapter: 'typescript' }));
put('docs/client.md', 'This is client `3.0.1`. <!-- fact: VERSION -->');
// ---cut---
const stale = await createProject({ root }).check();
assert.equal(stale.results[0].status, 'pass');
assert.equal(stale.exitCode, 0);
assert.equal(stale.surfaces.default.resolution?.entry, 'node_modules/@x/client/index.js');
assert.equal(stale.surfaces.default.warnings.length, 1);
assert.match(stale.surfaces.default.warnings[0], /3\.0\.1/);
assert.match(stale.surfaces.default.warnings[0], /packages\/client \(3\.2\.0\)/);

rmSync(join(root, 'node_modules/@x/client'), { recursive: true });
symlinkSync(join(root, 'packages/client'), join(root, 'node_modules/@x/client'), 'junction');
const linked = await createProject({ root }).check();
assert.equal(linked.results[0].status, 'fail');
assert.deepEqual(linked.surfaces.default.warnings, []);
```

## Coverage is counted per surface

Two surfaces may export the same name. A page that attests it on one surface says nothing
about the other, so each surface's coverage counts only the attestations made against it.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts', server: './src/server.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const LIMIT = 3;`,
    'src/server.ts': `export const LIMIT = 30;`,
    'docs/limits.md': 'Clients stop at `3`. <!-- fact: LIMIT -->',
  },
}).check();

assert.deepEqual(report.surfaces.default.unattested, []);
assert.deepEqual(report.surfaces.server.unattested, ['LIMIT']);
```
