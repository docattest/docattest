---
title: Inventory: what a fact could read
---

# Inventory: what a fact could read

Finding a copied fact on a page is a reading task; knowing which expression reads it from
the code is a lookup. `docattest audit --inventory` (`project.inventory()` in code) is the
lookup: every value the surfaces offer, with the expression a fact would use, optionally
only those whose expression or items contain a piece of text. An agent binding existing
pages searches it instead of guessing.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const Fields = { close: { job: 0, project: 1, projectSettings: 2 } };\nexport const REGIONS = ['eu', 'us'];`,
  },
});

const found = await project.inventory('projectSettings');
const keys = found.find((entry) => entry.expression === 'Object.keys(Fields.close)');
assert.deepEqual(keys?.items, ['job', 'project', 'projectSettings']);
assert.ok(found.every((entry) => JSON.stringify(entry).includes('projectSettings')));

const all = await project.inventory();
assert.ok(all.some((entry) => entry.expression === 'REGIONS'));
assert.deepEqual(await project.inventory('no such thing'), []);
```

## The setup module's named facts are called; nothing else is

`audit --inventory` and `audit` never call a function of a surface: a function may do anything, and
what it returns is not known until it runs. The setup module's named facts are the
exception, since they exist for pages to read. Each one that takes no arguments is called,
and what it returns is read as an export is, with the expression a fact would use:
`platformNames()`, or `(await versions()).node` for one that returns a promise. They are
offered with the default surface, which a fact without `@` reads. A named fact that takes
arguments is not called, and neither is one that shares its name with an export, which a
fact could not use.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': [
      "export function platforms() { return { 'linux-x64': 'manylinux', 'darwin-arm64': 'macosx' }; }",
      "export function deploy(): never { throw new Error('a surface function is never called'); }",
    ].join('\n'),
    'docs/docattest.setup.ts': [
      "import { platforms } from 'docattest-fixture';",
      'export const facts = {',
      '  platformNames: () => Object.keys(platforms()),',
      "  versions: async () => ({ node: '22.18' }),",
      '  wheel: (name: string) => platforms()[name],',
      '};',
    ].join('\n'),
    'docs/install.md': 'Wheels are built for `linux-x64` and `darwin-arm64`. Node 22.18 or later.\n',
  },
});

const all = await project.inventory();
assert.deepEqual(all.map((e) => [e.expression, e.items ?? e.value]), [
  ['platformNames()', ['linux-x64', 'darwin-arm64']],
  ['Object.keys((await versions()))', ['node']],
  ['Object.values((await versions()))', ['22.18']],
  ['(await versions()).node', '22.18'],
]);

const { suggestions } = await project.audit();
assert.deepEqual(suggestions.map((s) => [s.line, s.target, s.directive]), [
  [1, 'inline', '<!-- fact: platformNames() -->'],
  [1, 'scalar', '<!-- fact: (await versions()).node -->'],
]);
```
