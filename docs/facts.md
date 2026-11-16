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
```

## How a fact finds its target

Binding is positional and strict. A fact that ends its line binds the list, table or
fenced block on the next line. An item or cell compares as its first code span, else a bold
term that opens it (`**A**: first` is `A`), else its text. Otherwise it compares the code
span, bare number or bare version (`1.2.3-rc.1`) before it on the same line. A fact that
ends a sentence of two or more code spans compares them as a set when its value is a list,
in order with `fact ordered:`, and the last of them when its value is a single value; the
sentence starts after the previous sentence end or comment in the same paragraph, list item
or cell. When nothing on its own line can be compared, as at the end of a sentence wrapped
over lines, a fact binds the code spans of the sentence it ends: a set for two or more, the
one span for one. A fact with `column=` names a table column, so it binds the next table even across
blank lines. A directive may span lines; it binds from the line where it ends. Inside a
table cell it binds the same way, within that cell. Anything else is an error, not a silent
skip.

A set failure names the missing and unexpected values, an empty one as `""`, and counts
distinct values when a column repeats one. A block failure names the first line of the
block that differs.

<!-- cases: factBinding -->
| case                                   | page                                                                    | result                        |
|----------------------------------------|-------------------------------------------------------------------------|-------------------------------|
| scalar, code span on the same line     | At most `3` tries. <!-- fact: N -->                                     | pass                          |
| scalar, bare number on the same line   | At most 3 tries. <!-- fact: N -->                                       | pass                          |
| scalar mismatch                        | At most `4` tries. <!-- fact: N -->                                     | fail: expected 4, got 3       |
| scalar, bare version                   | Released as 1.2.3-rc.1. <!-- fact: VERSION -->                          | pass                          |
| scalar, version mismatch               | Released as 1.2.4. <!-- fact: VERSION -->                               | fail: expected 1.2.4          |
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
| list of bold lead terms                | <!-- fact: KEYS -->¶- **A**: first¶- **B**: second¶- **C**: third       | pass                          |
| ordered list, wrong order              | <!-- fact ordered: KEYS -->¶- `C`¶- `A`¶- `B`                           | fail: order                   |
| table column                           | <!-- fact column=path: ROUTES.map(r => r.path) -->¶\| method \| path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \| `/b` \| | pass |
| table column after a blank line        | <!-- fact column=path: ROUTES.map(r => r.path) -->¶¶\| method \| path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \| `/b` \| | pass |
| lead-in code span cannot steal a table | `ROUTES` lists: <!-- fact column=path: ROUTES.map(r => r.path) -->¶¶\| method \| path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \| `/b` \| | pass |
| empty cell in the column               | <!-- fact column=path: ROUTES.map(r => r.path) -->¶\| method \| path \|¶\|---\|---\|¶\| GET \| `/a` \|¶\| POST \|  \| | fail: unexpected: "" |
| repeated values count once             | <!-- fact column=method: ROUTES.map(r => r.method) -->¶\| method \|¶\|---\|¶\| GET \|¶\| GET \|¶\| PUT \| | fail: expected 2 distinct values in column method from the page, got 2 |
| column= with no table below            | <!-- fact column=path: ROUTES.map(r => r.path) -->¶¶Just a paragraph. | error: no table |
| fenced block, whitespace ignored       | <!-- fact: SHAPE -->¶```ts¶export interface Point {¶  x: number;¶  y: number¶}¶```  | pass                          |
| fenced block differing on one line     | <!-- fact: SHAPE -->¶```ts¶export interface Point {¶  x: number;¶  z: number¶}¶```  | fail: at line 3 (whitespace ignored): expected "z: number", got "y: number }" |
| no target                              | <!-- fact: N -->¶¶Just a paragraph.                                     | error: no target              |
| expression throws                      | At most `3` tries. <!-- fact: N.missing.deeper -->                      | error: Cannot read            |
| unknown surface                        | At most `3` tries. <!-- fact @nope: N -->                               | error: unknown surface        |
| directive spanning two lines           | <!-- fact:¶  KEYS -->¶- `A`¶- `B`¶- `C`                                 | pass                          |
| scalar inside a table cell             | \| limit \| value \|¶\|---\|---\|¶\| tries \| `3` <!-- fact: N --> \| | pass        |
| unclosed directive                     | At most `3` tries. <!-- fact: N                                         | error: not closed             |

In the `page` column `¶` stands for a line break and `\|` for a literal pipe.

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

An expression may only use what the surface exports. A fact over an unexported name is
an error, which is how a page is kept honest about what a user can observe. What a
package declares rather than exports, such as the Node it needs, is read from its manifest
with `manifest("package.json")` ([manifests](manifests.md)).

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `const INTERNAL = 7;\nexport const PUBLIC = INTERNAL + 1;`,
    'docs/x.md': 'Internally `7`. <!-- fact: INTERNAL -->',
  },
}).check();

assert.equal(report.results[0].status, 'error');
assert.match(report.results[0].message, /INTERNAL/);
```

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
