---
title: Manifests
---

# Manifests

Some claims are about how a project is packaged, not about its code: the Node it needs, the
Python it installs on, the Go a module asks for, the packages it depends on. They live in
manifests, which a package manager enforces, and no surface exports them. A fact reads one
with `manifest("<path>")`, which gives the file's parsed data, in every language docattest
supports. A claim about any other file, such as the user a `Dockerfile` runs as, reads it
with `file("<path>")`, which gives the file's text. Two calls pick one value out of a file
in one step, worked out by the core so that every language gets the same value:
`matched("<path>", "<regex>")` and `manifestField("<path>", "<keys>")`.

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

## JSON, TOML, YAML and go.mod

The file's name says its format: `.json` is JSON, `.toml` is TOML, `.yaml` and `.yml` are
YAML, and a file named `go.mod` is read for its `module`, `go` and `toolchain` directives,
each the string it was given (the other directives are not read). Every manifest is an
object at the top, so a fact reads a field of any of them the same way; a JSON or YAML
file that holds anything else is an error. Keys keep the order the file writes them in.
YAML is read as YAML 1.2: an alias is the node its anchor names, a key is its text, and a
file of several documents is an error. A fact with `field=` reads a fenced block of JSON,
TOML or YAML on the page the same way ([facts](facts.md#a-fact-can-check-one-field-of-a-data-block)).

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'pyproject.toml': '[project]\nname = "shop"\nrequires-python = ">=3.10"\n\n[project.scripts]\nshop = "shop:main"\nshop-admin = "shop:admin"\n',
    'go.mod': 'module example.com/shop\n\ngo 1.23\n\nrequire example.com/money v1.2.0\n',
    'pnpm-workspace.yaml': 'packages:\n  - apps/*\n  - libs/*\n',
    'docs/install.md': [
      'Python 3.10 or later. <!-- fact: manifest("pyproject.toml").project["requires-python"].slice(2) -->',
      '',
      'It installs two commands: <!-- fact: Object.keys(manifest("pyproject.toml").project.scripts) -->',
      '- `shop`',
      '- `shop-admin`',
      '',
      'The module is `example.com/shop` <!-- fact: manifest("go.mod").module -->, for Go 1.23 or later. <!-- fact: manifest("go.mod").go -->',
      '',
      'The workspace holds two kinds of package: <!-- fact: manifest("pnpm-workspace.yaml").packages -->',
      '- `apps/*`',
      '- `libs/*`',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['pass', 'pass', 'pass', 'pass', 'pass']);
```

## Any other file, as text

`file("<path>")` gives a project file's text as the language's string, so a fact takes
what it needs with the language's own string and regular-expression functions. It reads
any file, whatever its name, and parses nothing.

```ts check
import { createProject } from 'docattest';

const check = (user: string) => createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'Dockerfile': `FROM node:22-slim\nUSER ${user}\nCMD ["node", "server.js"]\n`,
    'docs/deploy.md': 'The image runs as user `10001`. <!-- fact: file("Dockerfile").match(/^USER (\\d+)$/m)[1] -->\n',
  },
}).check();

assert.equal((await check('10001')).results[0].status, 'pass');
const root = (await check('0')).results[0];
assert.equal(root.status, 'fail');
assert.match(root.message, /expected 10001, got 0/);
```

The text must be UTF-8, because it reaches the adapter inside the protocol's JSON. A file
that is not, such as an image, is an error that names it and the offset of its first byte
that is not UTF-8, rather than text with characters replaced that a fact would compare
without anyone seeing why.

```ts check
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createProject } from 'docattest';

const root = mkdtempSync(join(tmpdir(), 'docattest-file-'));
const put = (rel: string, text: string | Buffer) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('src/index.ts', 'export const x = 1;');
put('logo.png', Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));
put('docs/brand.md', 'The logo is a `PNG`. <!-- fact: file("logo.png").slice(1, 4) -->\n');

const [result] = (await createProject({ root }).check()).results;
assert.deepEqual([result.status, result.message], ['error', 'file "logo.png": not UTF-8 text (invalid byte at offset 0)']);
```

## One value of a file, in one call

`matched("<path>", "<regex>")` is what a regular expression matches in a file's text: the
text of its first group in its first match, or the whole match when it has no group. `^`
and `$` match at the start and end of each line, `.` stops at the end of a line, and a
line that ends in `\r\n` ends as one that ends in `\n` does, so a checkout with either line
ending gives the same value; `(?-m)` and `(?s)` in the regex turn the first two off. The
core matches it with the syntax of Rust's `regex` crate, with look-around and
back-references, the same regex in every language.

```ts check
import { createProject } from 'docattest';

const check = (user: string) => createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'Dockerfile': `FROM node:22-slim\r\nUSER ${user}\r\nCMD ["node", "server.js"]\r\n`,
    'docs/deploy.md': [
      'The image runs as user `10001`. <!-- fact: matched("Dockerfile", "^USER (\\\\d+)$") -->',
      '',
      'It starts from `node:22-slim`. <!-- fact: matched("Dockerfile", "(?<=^FROM )\\\\S+") -->',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual((await check('10001')).results.map((r) => r.status), ['pass', 'pass']);
const root = (await check('0')).results[0];
assert.equal(root.status, 'fail');
assert.match(root.message, /expected 10001, got 0/);
```

`manifestField("<path>", "<keys>")` is the value at a path of keys in a manifest's data,
written as `field=` writes one
([facts](facts.md#a-fact-can-check-one-field-of-a-data-block)): keys parted by dots, a
list's item by its index, a key that holds a dot or a space in quotes. The value is in the
language's own kind of value, as `manifest` gives one: a string, a number (an integer
where the file writes one), a boolean, null, a list or an object. So a deep field needs no
chain of indexes, and in a typed language no type assertions.

```ts check
import { createProject } from 'docattest';

const check = (version: string) => createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    '.gitlab-ci.yml': `docattest:\n  variables:\n    DOCATTEST_VERSION: ${version}\n  parallel: 3\n  "eu.west": { tags: [a, b] }\n`,
    'docs/ci.md': [
      'CI installs docattest `0.2.2`. <!-- fact: manifestField(".gitlab-ci.yml", "docattest.variables.DOCATTEST_VERSION") -->',
      '',
      'It runs `3` jobs at once. <!-- fact: manifestField(".gitlab-ci.yml", "docattest.parallel") + 0 -->',
      '',
      'Its first tag is `a`. <!-- fact: manifestField(".gitlab-ci.yml", \'docattest."eu.west".tags.0\') -->',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual((await check('0.2.2')).results.map((r) => r.status), ['pass', 'pass', 'pass']);
const moved = (await check('0.3.0')).results[0];
assert.equal(moved.status, 'fail');
assert.match(moved.message, /expected 0\.2\.2, got 0\.3\.0/);
```

The user's two facts that asked for these, in Go, before and after:

```markdown
<!-- fact: regexp.MustCompile(`USER (\d+):`).FindStringSubmatch(file("Dockerfile"))[1] -->
<!-- fact: matched("Dockerfile", `USER (\d+):`) -->

<!-- fact: manifest(".gitlab-ci.yml")["docattest"].(map[string]any)["variables"].(map[string]any)["DOCATTEST_VERSION"] -->
<!-- fact: manifestField(".gitlab-ci.yml", "docattest.variables.DOCATTEST_VERSION") -->
```

A regex that matches nothing, one that is not valid, a first group that takes no part in
the match, and a path that is not in the manifest are each an `error` at the fact, saying
which file and what is there instead, as `field=` says it; the fact does not run. So a
value that is gone is never compared as an empty string.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'Dockerfile': 'FROM node:22-slim\nUSER 10001\n',
    'ci.yml': 'deploy:\n  image: app\n  replicas: 3\n',
    'docs/deploy.md': [
      'User `1`. <!-- fact: matched("Dockerfile", "USER (\\\\d+):") -->',
      '',
      'User `1`. <!-- fact: matched("Dockerfile", "USER (\\\\d+") -->',
      '',
      'User `1`. <!-- fact: matched("Dockerfile", "(ROOT)?USER") -->',
      '',
      'Version `1`. <!-- fact: manifestField("ci.yml", "deploy.version") -->',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => [r.line, r.status, r.message]), [
  [1, 'error', 'matched "Dockerfile": "USER (\\d+):" matches nothing'],
  [3, 'error', 'matched "Dockerfile": "USER (\\d+" is not a valid regex: Parsing error at position 9: Opening parenthesis without closing parenthesis'],
  [5, 'error', 'matched "Dockerfile": "(ROOT)?USER" matches without its first group'],
  [7, 'error', 'manifestField "ci.yml" has no deploy.version: deploy has keys image, replicas'],
]);
```

## A manifest that cannot be read is an error that names it

A missing file, a file that is not valid in its format, and a path that leaves the project
are each an `error` at the fact, saying which file and why. The fact does not run. A file
that `file` reads follows the same rules, but for the format, which it does not read.

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
      'Version `1.0`. <!-- fact: file("VERSION").trim() -->',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => [r.line, r.status, r.message]), [
  [1, 'error', 'manifest "missing.json": no such file'],
  [3, 'error', 'manifest "broken.json": not valid JSON: EOF while parsing a value at line 1 column 13'],
  [5, 'error', 'manifest "../elsewhere/package.json" is outside the project'],
  [7, 'error', 'file "VERSION": no such file'],
]);
```

## The path is written out in the fact

The core reads the files facts name, manifests and texts alike, before anything runs, and
gives them to the adapter with the session, as it gives it the snapshots: it never
evaluates an expression, and an adapter cannot ask it for a file halfway through one. So it
reads exactly the files that facts name as a string literal, in any quotes, and a path
built at run time was never read. Reading only those keeps a check from reading, or failing on, files no page is about.
A call is the one docattest gives unless it is a member of something else, so a spread
before it, `[...manifest("tsconfig.json").references]`, still names its file.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': "export const MANIFEST = 'package.json'; export const DOCKERFILE = 'Dockerfile';",
    'package.json': JSON.stringify({ version: '1.0.0' }),
    'Dockerfile': 'USER 10001\n',
    'docs/version.md': 'Version `1.0.0`. <!-- fact: manifest(MANIFEST).version -->\n\nUser `10001`. <!-- fact: file(DOCKERFILE).slice(5, 10) -->\n',
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['error', 'error']);
assert.match(report.results[0].message, /manifest "package\.json" was not read: name it in the fact itself, as manifest\("package\.json"\)/);
assert.match(report.results[1].message, /file "Dockerfile" was not read: name it in the fact itself, as file\("Dockerfile"\)/);

const spread = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'tsconfig.json': JSON.stringify({ references: [{ path: 'apps/web' }, { path: 'libs/money' }] }),
    'docs/build.md': 'The build has `2` projects. <!-- fact: [...manifest("tsconfig.json").references].length -->\n',
  },
}).check();
assert.equal(spread.results[0].status, 'pass');
```

Both arguments of `matched` and of `manifestField` are written out too, and the core reads a
literal only where every language gives it the same value: a raw literal (`r"…"`, `r'…'`,
`r#"…"#`, `` `…` ``) is its text, and a quoted one may escape only a backslash, `\\`, and its
own quote. So a regex is written `"^USER (\\d+)$"`, or raw where the language has raw
strings: `` `^USER (\d+)$` `` in Go, `r"^USER (\d+)$"` in Python and Rust. Any other escape,
such as the `\d` of `"^USER (\d+)$"`, means something different in each language (in
TypeScript it is a plain `d`), so the call names nothing, and the fact fails saying it was
not read.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const x = 1;',
    'Dockerfile': 'USER 10001\n',
    'docs/deploy.md': [
      'User `10001`. <!-- fact: matched("Dockerfile", "^USER (\\\\d+)$") -->',
      '',
      'User `10001`. <!-- fact: matched("Dockerfile", "^USER (\\d+)$") -->',
      '',
    ].join('\n'),
  },
}).check();

assert.equal(report.results[0].status, 'pass');
assert.equal(report.results[1].status, 'error');
assert.match(report.results[1].message, /matched\("Dockerfile", "\^USER \(d\+\)\$"\) was not read: write both arguments out in the fact itself, as string literals/);
```

## A changed manifest or file reaches `impact`

`impact` lists the facts that read a manifest the change touched, under the manifest's call,
beside the exports the change affects; its answer then has `manifests`, the changed files
that facts read. A file that facts read with `file` is listed the same way, under its own
call, and in `files`. `impact --file` and an agent's hook answer only for an edit to a source
file, so an edit to a manifest shows in `impact` and `impact --base`, not after each edit.
`check --base` needs nothing more: every fact is evaluated against the file as it is.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const x = 1;',
  'package.json': JSON.stringify({ engines: { node: '>=22.12' } }),
  'Dockerfile': 'USER 10001\n',
  'docs/install.md': 'Node 22.12 or later. <!-- fact: manifest("package.json").engines.node.replace(">=", "") -->\n\nIt runs as user `10001`. <!-- fact: file("Dockerfile").slice(5, 10) -->\n',
};
const project = createProject({
  files: { ...base, 'package.json': JSON.stringify({ engines: { node: '>=24' } }), 'Dockerfile': 'USER 0\n' },
});

const impact = await project.impact({ baseFiles: base });
assert.deepEqual(impact.symbols, []);
assert.deepEqual(impact.manifests, ['package.json']);
assert.deepEqual(impact.files, ['Dockerfile']);
assert.deepEqual(impact.references, [
  { symbol: 'file("Dockerfile")', page: 'docs/install.md', line: 3, kind: 'fact', claim: 'It runs as user `10001`.', proven: true },
  { symbol: 'manifest("package.json")', page: 'docs/install.md', line: 1, kind: 'fact', claim: 'Node 22.12 or later.', proven: true },
]);
```

A fact that reads a file with `manifestField` is listed as one that reads it with
`manifest`, and one that reads it with `matched` as one that reads it with `file`: under
the file's call.

```ts check
import { createProject } from 'docattest';

const base = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const x = 1;',
  'package.json': JSON.stringify({ engines: { node: '>=22.12' } }),
  'Dockerfile': 'USER 10001\n',
  'docs/install.md': 'Node `>=22.12`. <!-- fact: manifestField("package.json", "engines.node") -->\n\nUser `10001`. <!-- fact: matched("Dockerfile", "^USER (\\\\d+)$") -->\n',
};
const project = createProject({
  files: { ...base, 'package.json': JSON.stringify({ engines: { node: '>=24' } }), 'Dockerfile': 'USER 0\n' },
});

const impact = await project.impact({ baseFiles: base });
assert.deepEqual([impact.manifests, impact.files], [['package.json'], ['Dockerfile']]);
assert.deepEqual(impact.references.map((r) => [r.symbol, r.line]), [['file("Dockerfile")', 3], ['manifest("package.json")', 1]]);
```
