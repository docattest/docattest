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

## A workspace package with a missing or stale build is a warning

A surface's code may import another package of the same workspace, which the package manager
links into `node_modules`. The runtime then loads that package's build, not its sources: a
member added to its sources is not there until it is rebuilt, and before its first build the
package does not load at all. So the imports are followed from each surface's entry through
the project's files and into every linked workspace package, and the surface gets a warning
for each one whose build is missing, or older than one of its TypeScript files (any outside
its build directory and `node_modules`). The warning names the package to build.

```ts check
import { createProject } from 'docattest';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = mkdtempSync(join(tmpdir(), 'docattest-builds-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
const age = (rel: string, seconds: number) => utimesSync(join(root, rel), seconds, seconds);
put('package.json', JSON.stringify({ name: 'repo', private: true, workspaces: ['packages/*'] }));
put('packages/types/package.json', JSON.stringify({ name: '@acme/types', type: 'module', main: 'dist/index.js' }));
put('packages/types/src/index.ts', 'export const LEVELS = { low: 1, high: 3 };');
put('packages/kit/package.json', JSON.stringify({ name: '@acme/kit', type: 'module' }));
put('packages/kit/src/index.ts', "export { LEVELS } from '@acme/types';");
mkdirSync(join(root, 'node_modules/@acme'), { recursive: true });
symlinkSync(join(root, 'packages/types'), join(root, 'node_modules/@acme/types'), 'junction');
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: { import: '@acme/kit', path: 'packages/kit/src/index.ts' } }, adapter: 'typescript' }));
put('docs/kit.md', 'The levels are `low, high`. <!-- fact: Object.keys(LEVELS).join(", ") -->');
// ---cut---
const unbuilt = await createProject({ root }).check();
assert.equal(unbuilt.results[0].status, 'error');
assert.match(unbuilt.surfaces.default.warnings[0], /@acme\/types \(packages\/types\) has no build/);

put('packages/types/dist/index.js', 'export const LEVELS = { low: 1, high: 3 };');
put('packages/types/src/index.ts', 'export const LEVELS = { low: 1, mid: 2, high: 3 };');
age('packages/types/dist/index.js', 1_700_000_000);
const stale = await createProject({ root }).check();
assert.equal(stale.results[0].status, 'pass');
assert.equal(stale.exitCode, 0);
assert.match(stale.surfaces.default.warnings[0], /loaded from its build packages\/types\/dist, which is older than packages\/types\/src\/index\.ts: rebuild @acme\/types/);

put('packages/types/dist/index.js', 'export const LEVELS = { low: 1, mid: 2, high: 3 };');
const rebuilt = await createProject({ root }).check();
assert.equal(rebuilt.results[0].status, 'fail');
assert.deepEqual(rebuilt.surfaces.default.warnings, []);
rmSync(root, { recursive: true });
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
