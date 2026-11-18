---
title: Facts
---

# Facts

A fact pins a literal in the prose to an expression over the public surface. The
comment is invisible when rendered; the literal is what the reader sees. When the two
disagree the build fails with both sides and the line, and someone edits the prose or
the code. A fact never rewrites the page.

All cases on this page run against this fixture surface:

```ts
export const N = 3;
export const KEYS = ['A', 'B', 'C'];
export const ROUTES = [{ method: 'GET', path: '/a' }, { method: 'POST', path: '/b' }];
export const SHAPE = `export interface Point { x: number; y: number }`;
export const VERSION = '1.2.3-rc.1';
export const STRICT = true;
```

## How a fact finds its target

Binding is positional and strict. A fact that ends its line binds the list, table or fenced
block on the next line. An item compares as its first code span, else a bold term that opens
it (`**A**: first` is `A`), else its text; a table cell compares as every code span in it,
as a cell may group several (`` `A`, `B` ``), else as an item does. Otherwise it compares
the code span, bare number or bare version (`1.2.3-rc.1`) before it on the same line. A
number or version may be written with a leading `v`, on the page or in the value: `v1.2.3`
compares as `1.2.3`. A fact that ends a sentence of two or more code spans compares them as
a set when its value is a list, in order with `fact ordered:`, and the last of them when its
value is a single value; the sentence starts after the previous sentence end or comment in
the same paragraph, list item or cell. When nothing on its own line can be compared, as at
the end of a sentence wrapped over lines, a fact binds the code spans of the sentence it
ends: a set for two or more, the one span for one. When its sentence has nothing to compare
either, a fact that ends its paragraph binds the list or fenced block right after that
paragraph, across blank lines, as after a lead-in like `The keys are:`. A fact with
`column=` names a table column, so it binds the next table even across blank lines and the
other directives stacked above it, one fact per column; a column
name with spaces is quoted, as in `column="route path"` or `column='route path'`. A fact
with `field=` binds one value of the next fenced block in the same way
([below](#a-fact-can-check-one-field-of-a-data-block)). A
directive may span lines; it binds from the line where it ends. Inside a table cell it binds
the same way, within that cell. Anything else is an error, not a silent skip.

A fact may also sit alone on the line directly above a sentence, as `// @ts-expect-error`
sits above the line it applies to: a line that holds only directives applies each of them
to the line below it. There a fact binds what it would bind at the end of the sentence that
starts on that line, which may wrap over lines: a code span or number on the sentence's
last line, else the sentence's code spans, else, when the sentence is a lead-in that ends
its paragraph, the list or fenced block after that paragraph. A fact alone on a line with a
blank line, then prose, below it is an error, never a bind to something further away; with
no prose directly above it either, the error says to put it directly above the sentence or
at its end. A fact written after text on its line binds what is before it.

A set failure leads with what differs: `missing:` the values the expression has and the
page does not, `unexpected:` those the page has and the expression does not, an empty one as
`""`. It gives the counts only when they differ, counting distinct values when a column
repeats one. A block compares token by token, a token being a run of letters, digits, `_`
and `$` or any other single character, so whitespace counts only where it parts two runs:
`[1,2]` matches `[1, 2]`, and `letx` does not match `let x`. A block failure names the first
line of the block that differs.

<!-- cases: factBinding -->
| case                                   | page                                                                    | result                        |
|----------------------------------------|-------------------------------------------------------------------------|-------------------------------|
| scalar, code span on the same line     | At most `3` tries. <!-- fact: N -->                                     | pass                          |
| scalar, bare number on the same line   | At most 3 tries. <!-- fact: N -->                                       | pass                          |
| scalar mismatch                        | At most `4` tries. <!-- fact: N -->                                     | fail: expected 4, got 3       |
| scalar, bare version                   | Released as 1.2.3-rc.1. <!-- fact: VERSION -->                          | pass                          |
| scalar, version mismatch               | Released as 1.2.4. <!-- fact: VERSION -->                               | fail: expected 1.2.4          |
| scalar, version with a leading v       | Released as v1.2.3-rc.1. <!-- fact: VERSION -->                         | pass                          |
| code span, version with a leading v    | Released as `v1.2.3-rc.1`. <!-- fact: VERSION -->                       | pass                          |
| sentence of code spans, any order      | Keys are `C`, `A` and `B`. <!-- fact: KEYS -->                          | pass                          |
| sentence missing a code span           | Keys are `A` and `B`. <!-- fact: KEYS -->                               | fail: missing: C              |
| sentence with an extra code span       | Keys are `A`, `B`, `C` and `D`. <!-- fact: KEYS -->                     | fail: unexpected: D           |
| ordered sentence, wrong order          | Keys are `C`, `A` and `B`. <!-- fact ordered: KEYS -->                  | fail: order                   |
| sentence starts after the previous one | See `X`. Keys are `A`, `B` and `C`. <!-- fact: KEYS -->                 | pass                          |
| single value after several code spans  | `A` and `B` allow `3`. <!-- fact: N -->                                 | pass                          |
| sentence wrapped over lines            | Keys are `A`, `B`¶and `C` here. <!-- fact: KEYS -->                     | pass                          |
| wrapped sentence missing a code span   | Keys are `A` and `B`¶here. <!-- fact: KEYS -->                          | fail: missing: C              |
| wrapped sentence of one code span      | The first key is `A`¶here. <!-- fact: KEYS[0] -->                      | pass                          |
| list, any order                        | <!-- fact: KEYS -->¶- `C`¶- `A`¶- `B`                                   | pass                          |
| list missing an item                   | <!-- fact: KEYS -->¶- `A`¶- `B`                                         | fail: missing: C              |
| list with an extra item                | <!-- fact: KEYS -->¶- `A`¶- `B`¶- `C`¶- `D`                             | fail: unexpected: D           |
| list with as many items, one differing | <!-- fact: KEYS -->¶- `A`¶- `B`¶- `D`                                   | fail: missing: C; unexpected: D (comparing the list on the page with KEYS) |
| list after a lead-in and a blank line  | The keys are: <!-- fact: KEYS -->¶¶- `C`¶- `A`¶- `B`                    | pass                          |
| a value on the line keeps it a scalar  | At most `3` tries. <!-- fact: N -->¶¶- `A`¶- `B`                        | pass                          |
| list of bold lead terms                | <!-- fact: KEYS -->¶- **A**: first¶- **B**: second¶- **C**: third       | pass                          |
| ordered list, wrong order              | <!-- fact ordered: KEYS -->¶- `C`¶- `A`¶- `B`                           | fail: order                   |
| table column                           | <!-- fact column=path: ROUTES.map(r => r.path) -->¶\| method \| path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \| `/b` \| | pass |
| table column after a blank line        | <!-- fact column=path: ROUTES.map(r => r.path) -->¶¶\| method \| path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \| `/b` \| | pass |
| lead-in code span cannot steal a table | `ROUTES` lists: <!-- fact column=path: ROUTES.map(r => r.path) -->¶¶\| method \| path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \| `/b` \| | pass |
| column name with spaces, quoted       | <!-- fact column="route path": ROUTES.map(r => r.path) -->¶\| method \| route path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \| `/b` \| | pass |
| column name with spaces, unquoted     | <!-- fact column=route path: ROUTES.map(r => r.path) -->¶\| method \| route path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \| `/b` \| | error: unknown modifier "path"; a column name with spaces is quoted: column="route path" |
| cell with several code spans           | <!-- fact column=path: ROUTES.map(r => r.path) -->¶\| path \|¶\|---\|¶\| `/a`, `/b` \| | pass |
| empty cell in the column               | <!-- fact column=path: ROUTES.map(r => r.path) -->¶\| method \| path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \|  \| | fail: unexpected: "" |
| repeated values count once             | <!-- fact column=method: ROUTES.map(r => r.method) -->¶\| method \|¶\|---\|¶\| GET \|¶\| GET \| | fail: missing: POST (1 distinct value in column method on the page, 2 in |
| column= with no table below            | <!-- fact column=path: ROUTES.map(r => r.path) -->¶¶Just a paragraph. | error: no table |
| fenced block, whitespace ignored       | <!-- fact: SHAPE -->¶```ts¶export interface Point {¶  x: number;¶  y: number¶}¶```  | pass                          |
| fenced block, spaced differently       | <!-- fact: SHAPE -->¶```ts¶export interface Point{x:number;¶y : number }¶```  | pass                          |
| fenced block, words run together       | <!-- fact: SHAPE -->¶```ts¶exportinterface Point { x: number; y: number }¶```  | fail: at line 1 (whitespace ignored): expected "exportinterface Point { x: number; y: number }" |
| fenced block differing on one line     | <!-- fact: SHAPE -->¶```ts¶export interface Point {¶  x: number;¶  z: number¶}¶```  | fail: at line 3 (whitespace ignored): expected "z: number", got "y: number }" |
| line above a sentence                  | <!-- fact: N -->¶At most `3` tries. Then `4` more.                      | pass                          |
| line above a sentence, mismatch        | <!-- fact: N -->¶At most `4` tries.                                     | fail: expected 4, got 3       |
| line above a wrapped sentence          | <!-- fact: KEYS -->¶Keys are `C`, `A`¶and `B`.                          | pass                          |
| line above a lead-in to a list         | <!-- fact: KEYS -->¶The keys are:¶¶- `C`¶- `A`¶- `B`                    | pass                          |
| line above, then a blank line          | <!-- fact: N -->¶¶At most `3` tries.                                    | error: must sit directly above the sentence it attests |
| no target                              | Just a paragraph. <!-- fact: N -->                                      | error: no target              |
| expression throws                      | At most `3` tries. <!-- fact: N.missing.deeper -->                      | error: Cannot read            |
| unknown surface                        | At most `3` tries. <!-- fact @nope: N -->                               | error: unknown surface        |
| directive spanning two lines           | <!-- fact:¶  KEYS -->¶- `A`¶- `B`¶- `C`                                 | pass                          |
| scalar inside a table cell             | \| limit \| value \|¶\|---\|---\|¶\| tries \| `3` <!-- fact: N --> \| | pass        |
| unclosed directive                     | At most `3` tries. <!-- fact: N                                         | error: not closed             |

In the `page` column `¶` stands for a line break and `\|` for a literal pipe.

## A fact can check one field of a data block

A fact with `field=<path>` before a fenced block of JSON, TOML or YAML (`json`, `toml`,
`yaml` or `yml`) compares one value in the block, not the whole block. The block is read as
a manifest of its format is ([manifests](manifests.md)), so it holds an object at the top.
A path is keys parted by dots, and a key of digits picks a list's item by its index, counted
from 0: `field=jobs.0.retries`. A key with a dot or a space in it is quoted:
`field=limits."max retries"`. The block is written as its format writes values, and the
fact's value as the project's language displays it, so the two are compared as the same
value, not as the same spelling: the adapter displays the block's value as its language
displays a fact's. A JSON `1.0` is a TypeScript `1`, and in a Python project the block's
`true` is Python's `True`
([the Python guide](../adapters/python/README.md#facts)).
A string is its text, and a list of scalars compares as a list does, in order with
`fact ordered:`. A field that holds an object, or a list of objects, is an error that says
so. So is a block in another language, a block that does not parse, with the parser's
message and its lines counted from the line after the fence, and a path that is not there,
with the keys found where it stops.

<!-- cases: factBinding -->
| case                                | page                                                                            | result                                         |
|-------------------------------------|---------------------------------------------------------------------------------|------------------------------------------------|
| field of a JSON block               | <!-- fact field=retries: N -->¶```json¶{ "retries": 3, "region": "eu" }¶```     | pass                                           |
| field mismatch                      | <!-- fact field=retries: N -->¶```json¶{ "retries": 4 }¶```                     | fail: expected 4, got 3                        |
| list item by its index              | <!-- fact field=jobs.0.retries: N -->¶```yaml¶jobs:¶  - retries: 3¶```          | pass                                           |
| string field                        | <!-- fact field=release.version: VERSION -->¶```toml¶[release]¶version = "1.2.3-rc.1"¶``` | pass                         |
| boolean field                       | <!-- fact field=strict: STRICT -->¶```yaml¶strict: true¶```                     | pass                                           |
| float field, as the language shows it | <!-- fact field=ratio: RATIO -->¶```json¶{ "ratio": 1.0 }¶```                 | pass                                           |
| float field mismatch                | <!-- fact field=ratio: RATIO -->¶```toml¶ratio = 1.5¶```                        | fail: expected 1.5, got 1                      |
| list of mixed scalars               | <!-- fact field=flags ordered: ['a', RATIO, STRICT] -->¶```yaml¶flags: [a, 1.0, true]¶``` | pass                                 |
| list field, any order               | <!-- fact field=keys: KEYS -->¶```toml¶keys = ["C", "A", "B"]¶```               | pass                                           |
| ordered list field, wrong order     | <!-- fact field=keys ordered: KEYS -->¶```toml¶keys = ["C", "A", "B"]¶```       | fail: order                                    |
| list field missing an item          | <!-- fact field=keys: KEYS -->¶```yml¶keys: [A, B]¶```                          | fail: missing: C                               |
| key with spaces, quoted             | <!-- fact field=limits."max retries": N -->¶```json¶{ "limits": { "max retries": 3 } }¶``` | pass                                |
| key with a dot, quoted              | <!-- fact field="retries.max": N -->¶```yaml¶retries.max: 3¶```                 | pass                                           |
| path that is not there              | <!-- fact field=retry: N -->¶```json¶{ "retries": 3, "region": "eu" }¶```       | error: has no retry: the top level has keys retries, region |
| index past the end of a list        | <!-- fact field=jobs.1.retries: N -->¶```yaml¶jobs:¶  - retries: 3¶```          | error: has no jobs.1.retries: jobs is a list of 1 item |
| field holding an object             | <!-- fact field=limits: N -->¶```json¶{ "limits": { "retries": 3 } }¶```        | error: limits in the json block at L2 has keys retries; name a field inside it |
| field holding a list of objects     | <!-- fact field=jobs: N -->¶```yaml¶jobs:¶  - retries: 3¶```                    | error: jobs in the yaml block at L2 is a list of objects or lists |
| block in another language           | <!-- fact field=retries: N -->¶```ts¶const retries = 3;¶```                     | error: the block after it is ts, not json, toml or yaml |
| block that does not parse           | <!-- fact field=retries: N -->¶```json¶{¶  "retries": 3,¶}¶```                  | error: the json block at L2 cannot be read: not valid JSON: trailing comma at line 3 |
| field= with no block below          | <!-- fact field=retries: N -->¶¶Just a paragraph.                               | error: no fenced block follows it              |

Like `column=`, `field=` binds the next fenced block across blank lines and the other
directives stacked above it, so several facts may each check one field of the same block.

```ts check
import { createProject } from 'docattest';

const check = (port: number) => createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const SSH = { port: ${port}, image: 'app:1' };`,
    'docs/job.md': [
      'The job listens on port 2222: <!-- fact field=ports.0.port: SSH.port -->',
      '<!-- fact field=image: SSH.image -->',
      '',
      '```json',
      '{ "image": "app:1", "ports": [{ "port": 2222 }], "replicas": 3 }',
      '```',
      '',
    ].join('\n'),
  },
}).check();

assert.deepEqual((await check(2222)).results.map((r) => r.status), ['pass', 'pass']);
const [moved] = (await check(2223)).results;
assert.equal(moved.status, 'fail');
assert.match(moved.message, /expected 2222, got 2223/);
```

## A failure names both sides and the line

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const ErrorCodes = { A: 'A', B: 'B', C: 'C' } as const;`,
    'docs/errors.md': '# Errors\n\n`error.code` is one of: <!-- fact: Object.keys(ErrorCodes) -->\n- `A`\n- `B`\n',
  },
}).check();

const [r] = report.results;
assert.equal(r.kind, 'fact');
assert.equal(r.status, 'fail');
assert.equal(r.line, 3);
assert.equal(r.expected, '[A, B]');
assert.equal(r.actual, '[A, B, C]');
assert.match(r.message, /missing: C/);
assert.equal(report.exitCode, 1);
```

## Facts observe the surface, not the source

An expression may use what the surface exports, and the constants its module declares at
the top level, exported or not. A fact over any other name the module does not export, a
function, a type or a variable, is an error that says only constants can be read there,
which is how a page is kept honest about what a user can observe.

Constants are read from the source, since a program's most documented values, its port or
its default timeout, are often unexported, or in a program that nothing imports. A fact
that reads only constants runs nothing of the surface. So a surface may be a program: a Go
`package main`, a Rust binary crate or a Python `__main__.py`, of which a fact reads only
constants, or a TypeScript command's entry script, which a fact that names anything else
of it loads. A fact attests the constants it reads, as `where` shows, while `uncovered`
lists only exports, and `impact` lists a change to such a constant with the facts that read
it.

What a constant is differs by language: a `const` written in literals in TypeScript, a name
assigned once to literals in Python, any top-level `const` in Go, and a `const` or `static`
of primitive types in Rust. Each guide says it exactly:
[TypeScript](../adapters/typescript/README.md#facts-read-constants-from-source),
[Python](../adapters/python/README.md#facts), [Go](../adapters/go/docs/facts.md) and
[Rust](../adapters/rust/README.md#facts).

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `const INTERNAL = 7;\nfunction secret() { return INTERNAL; }\nexport const PUBLIC = secret() + 1;`,
    'docs/x.md': 'Internally `7`. <!-- fact: INTERNAL -->\n\nSecretly `7`. <!-- fact: secret() -->',
  },
});
const report = await project.check();

assert.deepEqual(report.results.map((r) => r.status), ['pass', 'error']);
assert.match(report.results[1].message, /secret is a function that surface "default" does not export; of its unexported names, a fact reads only constants/);
assert.deepEqual(await project.where('INTERNAL'), [{ page: 'docs/x.md', line: 1, kind: 'fact', claim: 'Internally `7`.' }]);
assert.deepEqual(await project.uncovered(), ['PUBLIC']);
```

What a package declares rather than exports, such as the Node it needs, is read from its
manifest with `manifest("package.json")`, and what any other file says with
`file("<path>")` ([manifests](manifests.md)).

## Facts may await

A surface whose values are asynchronous is still observable.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const version = async () => '2.0';`,
    'docs/v.md': 'Reports version `2.0`. <!-- fact: await version() -->',
  },
}).check();

assert.equal(report.results[0].status, 'pass');
```

## Facts can call named helpers from the setup module

Some values are awkward to write inline. The setup module may export `facts`, an object of
named helpers, and a fact expression calls them by bare name. The helper is glue: it reads
the surface and returns a value; the expectation is still the literal on the page.

```ts check
import { createProject } from 'docattest';

const project = (setup: string, page: string) => createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;\nexport const ERRORS = { missing: { field: 'name' }, invalid: { field: 'email' } };`,
    'docs/docattest.setup.ts': setup,
    'docs/errors.md': page,
  },
});
const helpers = [
  "import { ERRORS } from 'docattest-fixture';",
  'export const facts = { errorFields: () => Object.values(ERRORS).map((e) => e.field) };',
].join('\n');
const table = '<!-- fact column=Field: errorFields() -->\n| Field | Meaning |\n|---|---|\n| `name` | required |\n| `email` | malformed |\n';

const report = await project(helpers, table).check();
assert.equal(report.results[0].status, 'pass');
assert.deepEqual(report.results[0].symbols, ['ERRORS']);
```

A helper may not share a name with a surface export: the expression would be ambiguous, so
it is an error rather than a guess.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': `export const N = 3;`,
    'docs/docattest.setup.ts': 'export const facts = { N: () => 4 };',
    'docs/n.md': 'N is `3`. <!-- fact: N -->',
  },
}).check();

assert.equal(report.results[0].status, 'error');
assert.match(report.results[0].message, /"N" is both an export of surface "default" and a setup fact/);
```
