---
title: Manifests
---

# Manifests

Some claims are about how a project is packaged, not about its code: the Node it needs, the
Python it installs on, the Go a module asks for, the packages it depends on. They live in
manifests, which a package manager enforces, and no surface exports them. A fact reads one
with `manifest("<path>")`, which gives the file's parsed data, in every language docattest
supports.

## A fact reads a field of a manifest

The path is relative to the project root. The fact is held to the file as it is now, so it
fails when the manifest changes and the page does not.

```ts check
import { createProject } from 'docattest';

const check = (engines: string) => createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'package.json': JSON.stringify({ name: 'shop', engines: { node: engines } }),
    'docs/install.md': 'Node 22.12 or later. <!-- fact: manifest("package.json").engines.node.replace(">=", "") -->\n',
  },
}).check();

assert.equal((await check('>=22.12')).results[0].status, 'pass');
const moved = (await check('>=24')).results[0];
assert.equal(moved.status, 'fail');
assert.match(moved.message, /expected 22\.12, got 24/);
```

## JSON, TOML and go.mod

The file's name says its format: `.json` is JSON, `.toml` is TOML, and a file named `go.mod`
is read for its `module`, `go` and `toolchain` directives, each the string it was given
(the other directives are not read). Every manifest is an object at the top, so a fact
reads a field of any of them the same way; a JSON file that holds anything else is an
error. Keys keep the order the file writes them in.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'pyproject.toml': '[project]\nname = "shop"\nrequires-python = ">=3.10"\n\n[project.scripts]\nshop = "shop:main"\nshop-admin = "shop:admin"\n',
    'go.mod': 'module example.com/shop\n\ngo 1.23\n\nrequire example.com/money v1.2.0\n',
    'docs/install.md': [
      'Python 3.10 or later. <!-- fact: manifest("pyproject.toml").project["requires-python"].slice(2) -->',
      '',
      'It installs two commands: <!-- fact: Object.keys(manifest("pyproject.toml").project.scripts) -->',
      '- `shop`',
      '- `shop-admin`',
      '',
      'The module is `example.com/shop` <!-- fact: manifest("go.mod").module -->, for Go 1.23 or later. <!-- fact: manifest("go.mod").go -->',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['pass', 'pass', 'pass', 'pass']);
```

## A manifest that cannot be read is an error that names it

A missing file, a file that is not valid in its format, and a path that leaves the project
are each an `error` at the fact, saying which file and why. The fact does not run.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'broken.json': '{ "version": ',
    'docs/install.md': [
      'Version `1.0`. <!-- fact: manifest("missing.json").version -->',
      '',
      'Version `1.0`. <!-- fact: manifest("broken.json").version -->',
      '',
      'Version `1.0`. <!-- fact: manifest("../elsewhere/package.json").version -->',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => [r.line, r.status, r.message]), [
  [1, 'error', 'manifest "missing.json": no such file'],
  [3, 'error', 'manifest "broken.json": not valid JSON: EOF while parsing a value at line 1 column 13'],
  [5, 'error', 'manifest "../elsewhere/package.json" is outside the project'],
]);
```

## The path is written out in the fact

The core reads and parses the manifests before anything runs, and gives them to the
adapter with the session, as it gives it the snapshots: it never evaluates an expression,
and an adapter cannot ask it for a file halfway through one. So it reads exactly the files
that facts name as a string literal, in any quotes, and a path built at run time was never
read. Reading only those keeps a check from reading, or failing on, files no page is about.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': "export const MANIFEST = 'package.json';",
    'package.json': JSON.stringify({ version: '1.0.0' }),
    'docs/version.md': 'Version `1.0.0`. <!-- fact: manifest(MANIFEST).version -->\n',
  },
}).check();

assert.equal(report.results[0].status, 'error');
assert.match(report.results[0].message, /manifest "package\.json" was not read: name it in the fact itself, as manifest\("package\.json"\)/);
```

## A changed manifest reaches `impact`

`impact` lists the facts that read a manifest the change touched, under the manifest's call,
beside the exports the change affects; its answer then has `manifests`, the changed files
that facts read. `impact --file` and an agent's hook answer only for an edit to a source
file, so an edit to a manifest shows in `impact` and `impact --base`, not after each edit.
`check --base` needs nothing more: every fact is evaluated against the manifest as it is.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const x = 1;',
  'package.json': JSON.stringify({ engines: { node: '>=22.12' } }),
  'docs/install.md': 'Node 22.12 or later. <!-- fact: manifest("package.json").engines.node.replace(">=", "") -->\n',
};
const project = createProject({ files: { ...base, 'package.json': JSON.stringify({ engines: { node: '>=24' } }) } });

const impact = await project.impact({ baseFiles: base });
assert.deepEqual(impact.symbols, []);
assert.deepEqual(impact.manifests, ['package.json']);
assert.deepEqual(impact.references, [
  { symbol: 'manifest("package.json")', page: 'docs/install.md', line: 1, kind: 'fact', claim: 'Node 22.12 or later.', proven: true },
]);
```
