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

and, where a fact names the surface `shipping`, against this one:

```ts
export const FREE_FROM = 50;
export const ZONES = ['EU', 'UK', 'US'];
```

## How a fact is written

A fact is `<!-- fact`, then up to three modifiers in any order, then a colon and the
expression, which runs to `-->` and may span lines. `@<surface>` evaluates the expression
against that surface of the config ([surfaces](surfaces.md)), not `default`. `ordered`
compares a list in order, not as a set: the items of a list, a table column, the code spans
of a sentence or a list in a data block. `column=<name>` binds a column of the next table,
and `field=<path>` one value of the next fenced block; a fact names at most one of the two.
So `<!-- fact @shipping ordered column=zone: ZONES -->` compares the column `zone`, in
order, with `ZONES` from the surface `shipping`. Anything else before the colon is an error
that names it as an unknown modifier. A fact reads one surface and selects with one of
`column=` and `field=`, so a second `@surface`, `column=` or `field=` is an error that says
so, and so is a `column=` that names nothing (`column=""`). `column=`
with `field=` is an error, and so is `ordered` with no list to compare: on a single value,
a fenced block or a field that holds one value. A fact with no colon, or nothing after it,
is an error that says it has no expression.

<!-- cases: factBinding -->
| case                                 | page                                                                     | result                        |
|--------------------------------------|--------------------------------------------------------------------------|-------------------------------|
| `@surface`                           | Free shipping from `50`. <!-- fact @shipping: FREE_FROM -->              | pass                          |
| `@surface` and `field=`              | <!-- fact @shipping field=free_from: FREE_FROM -->¶```json¶{ "free_from": 50 }¶``` | pass                |
| `@surface`, `ordered` and `column=`  | <!-- fact @shipping ordered column=zone: ZONES -->¶\| zone \|¶\|---\|¶\| `EU` \|¶\| `UK` \|¶\| `US` \| | pass |
| the same, out of order               | <!-- fact @shipping ordered column=zone: ZONES -->¶\| zone \|¶\|---\|¶\| `UK` \|¶\| `EU` \|¶\| `US` \| | fail: order |
| modifiers in another order           | <!-- fact column=zone ordered @shipping: ZONES -->¶\| zone \|¶\|---\|¶\| `EU` \|¶\| `UK` \|¶\| `US` \| | pass |
| `ordered` and `field=` over a list   | <!-- fact ordered field=zones @shipping: ZONES -->¶```yaml¶zones: [EU, UK, US]¶``` | pass                |
| `column=` with `field=`              | <!-- fact column=zone field=zones @shipping: ZONES -->                   | error: column= and field= cannot both be given |
| a second `column=`                   | <!-- fact column=zone column=code @shipping: ZONES -->¶\| zone \|¶\|---\|¶\| `EU` \| | error: a fact selects with one column= or field=; "column=code" is a second |
| a second `@surface`                  | Free shipping from `50`. <!-- fact @shipping @default: FREE_FROM -->     | error: a fact reads one surface; it names @shipping and @default |
| a `column=` that names nothing       | <!-- fact column="": KEYS -->¶\| key \|¶\|---\|¶\| `A` \|               | error: column= names no column |
| unknown modifier                     | <!-- fact sorted: KEYS -->¶- `A`¶- `B`¶- `C`                             | error: unknown modifier "sorted" |
| `ordered` on a single value          | At most `3` tries. <!-- fact ordered: N -->                              | error: column= and ordered apply to lists and tables, not scalars |
| `column=` on a single value          | At most `3` <!-- fact column=zone: N --> tries.                          | error: column= and ordered apply to lists and tables, not scalars |
| `ordered` on a fenced block          | <!-- fact ordered: SHAPE -->¶```ts¶export interface Point { x: number; y: number }¶``` | error: not fenced blocks |
| `ordered` on a field of one value    | <!-- fact @shipping ordered field=free_from: FREE_FROM -->¶```json¶{ "free_from": 50 }¶``` | error: is a single value; ordered applies to a list |
| no colon                             | <!-- fact KEYS -->¶- `A`¶- `B`¶- `C`                                     | error: has no expression      |
| nothing after the colon              | At most `3` tries. <!-- fact ordered: -->                                | error: has no expression      |

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
the same way, within that cell. Directives side by side are one group, whatever their order:
a fact followed on its line only by other directives, such as a citation, ends that line,
its sentence, its paragraph or its cell as if it were last, and a fact that ends its line
binds what starts after the lines of only directives directly below it. Anything else is
an error, not a silent skip.

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
| citation after, line before a list     | The keys, `A` among them: <!-- fact: KEYS --> <!-- test: keys.test.ts > "lists keys" -->¶- `C`¶- `A`¶- `B` | pass |
| citation before, line before a list    | The keys, `A` among them: <!-- test: keys.test.ts > "lists keys" --> <!-- fact: KEYS -->¶- `C`¶- `A`¶- `B` | pass |
| citation after, end of a sentence      | Keys are `C`, `A` and `B`. <!-- fact: KEYS --> <!-- test: keys.test.ts > "lists keys" --> | pass |
| citation before, end of a sentence     | Keys are `C`, `A` and `B`. <!-- test: keys.test.ts > "lists keys" --> <!-- fact: KEYS --> | pass |
| citation after, line above a list      | <!-- fact: KEYS --> <!-- test: keys.test.ts > "lists keys" -->¶- `C`¶- `A`¶- `B` | pass |
| citation before, line above a list     | <!-- test: keys.test.ts > "lists keys" --> <!-- fact: KEYS -->¶- `C`¶- `A`¶- `B` | pass |
| citation on the line below the fact    | The keys: <!-- fact: KEYS -->¶<!-- test: keys.test.ts > "lists keys" -->¶- `C`¶- `A`¶- `B` | pass |
| citation after, lead-in, blank line    | The keys: <!-- fact: KEYS --> <!-- test: keys.test.ts > "lists keys" -->¶¶- `C`¶- `A`¶- `B` | pass |
| citation before, in a table cell       | \| limit \| value \|¶\|---\|---\|¶\| tries \| `3` <!-- test: keys.test.ts > "lists keys" --> <!-- fact: N --> \| | pass |
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
function, a type or a variable, is an error that says only constants (and in Go literal
variables) can be read there, which is how a page is kept honest about what a user can
observe.

Constants are read from the source, since a program's most documented values, its port or
its default timeout, are often unexported, or in a program that nothing imports. A fact
that reads only constants runs nothing of the surface. So a surface may be a program: a Go
`package main`, a Rust binary crate or a Python `__main__.py`, of which a fact reads only
constants, or a TypeScript command's entry script, which a fact that names anything else
of it loads. A fact attests the constants it reads, as `where` shows, while `uncovered`
lists only exports, and `impact` lists a change to such a constant with the facts that read
it.

A value holds the values of the names its initializer uses, so a fact that reads it attests
them too, at any depth, and so do a named fact and a cases runner: a fact over `ENV`, built
from `COMMAND_PATH`, attests `COMMAND_PATH`, and a private constant used there is credited
for `where` while `uncovered` leaves it out. A function's body is not followed: a fact over
`total()` attests `total`, not the names its body reads. <!-- test: adapters/typescript/test/unit/language/symbols.test.ts > "a fact that reads a value built from other names" > "attests the names its initializer uses, at any depth, private ones too" -->

What a constant is differs by language: a `const` written in literals in TypeScript, a name
assigned once to literals in Python, any top-level `const` in Go, and a `var` its package
declares with a composite literal of constants and only reads (a literal variable), and a
`const` or `static` of primitive types in Rust. Each guide says it exactly:
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

## Docs helpers say what a surface keeps private

A value a page states should not have to be exported for the page's sake. A constant needs
nothing: it is read from source. A value a helper must work out, such as the names in a
private list of records, needs code that can see the private value, and each language has
its way:

- **Go:** a docs helper is a file of the surface's package behind `//go:build docattest`.
  Only docattest builds it, so it sees the package's unexported names, and a fact calls what
  it exports ([Go guide](../adapters/go/docs/facts.md#docs-helpers)).
- **Rust:** a docs helper is a `pub` item of the surface's module under `#[cfg(docattest)]`,
  built with `--cfg docattest` only by docattest, so it sees the module's private items
  ([Rust guide](../adapters/rust/README.md#docs-helpers-cfgdocattest)).
- **Python:** nothing stops a module from importing another's `_private` names, so a setup
  helper reads them as they are ([Python guide](../adapters/python/README.md#cases-tables-and-named-facts-the-setup-module)).
- **TypeScript:** nothing outside a module reaches what it does not export. A value is
  exported by the module that declares it, not re-exported from the package entry, and that
  module is configured as a surface of its own, so the package's API is unchanged
  ([TypeScript guide](../adapters/typescript/README.md#named-facts-the-setup-module)).

A docs helper is no export, so `uncovered` never lists it. A fact that calls one attests it
and what it uses: the private values it reads, which `where` then lists the fact under, and
the surface's functions it calls, by name, without following their bodies. So `impact` lists
the fact when a private value it reads changes. A page's
example runs as users build the code, without docs helpers: in Go a block is built without
the tag, and in Rust a block or doc-comment example that names a docs helper is an error at
the line that names it. A program (a Go
`package main`, a Rust binary crate) is never imported, so a docs helper there cannot be
called; its constants, and in Go its literal variables, are read from source instead.

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
the surface and returns a value; the expectation is still the literal on the page. A fact
that calls a helper attests what the helper uses of the surfaces, through the setup
module's own helpers at any depth, as a cases table attests what its runner uses: `where`
lists the fact under those exports, `uncovered` stops listing them, and `impact` lists the
fact when one of them changes.

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
assert.deepEqual((await project(helpers, table).where('ERRORS')).map((r) => r.line), [1]);
assert.deepEqual(await project(helpers, table).uncovered(), ['N']);
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
