---
title: Audit: claims nobody attested yet
---

# Audit: claims nobody attested yet

A fact defends a literal only once someone writes the directive. The drift docattest was
designed from sat in plain lists and values nobody had bound: an environment variable list
missing keys, a route table missing routes, a hand-copied enum one member short.
`docattest audit` looks for those on any repository, directives or not. It reads the living
pages, reads what each surface exports, and reports lists, table columns, fenced blocks and
values in sentences that look copied from the surface: those that agree (a fact worth
adding) and those that nearly agree (probable drift now). Each suggestion carries the
directive to paste.

`audit` is advisory and read-only. It exits `0` whatever it finds, writes nothing, and runs
nothing on the pages. A wrong suggestion costs trust in every right one, so when it is not
sure it says nothing. A surface that cannot be loaded is not something it found but a broken
project: an error with exit code `1` that names the cause.

## A list that fell behind the code is drift, and the suggestion is a working fact

The surface values are evaluated exactly as `check` evaluates facts, and every suggested
expression is evaluated again, on its own, before it is reported; one that does not yield
the value compared is dropped. So the suggested directive, pasted above the list, fails
`check` with the same `missing` items.

```ts check
import { createProject } from 'docattest';

const config = JSON.stringify({ pages: ['docs/**/*.md', 'README.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' });
const project = (files: Record<string, string>) => createProject({ files: { 'docattest.config.json': config, ...files } });
// ---cut---
const source = `export const ENV_KEYS = ['API_TOKEN', 'REGION', 'PORT', 'RATE'];`;
const page = '# Service\n\nEnvironment variables override the file:\n\n- `API_TOKEN`\n- `REGION`\n- `PORT`\n';

const { suggestions } = await project({ 'src/index.ts': source, 'README.md': page }).audit();
assert.deepEqual(suggestions, [{
  page: 'README.md', line: 5, target: 'list', expression: 'ENV_KEYS', status: 'drift',
  missing: ['RATE'], unexpected: [], surface: 'default', directive: '<!-- fact: ENV_KEYS -->',
}]);

const pasted = page.replace('- `API_TOKEN`', `${suggestions[0].directive}\n- \`API_TOKEN\``);
const report = await project({ 'src/index.ts': source, 'README.md': pasted }).check();
assert.equal(report.results[0].status, 'fail');
assert.match(report.results[0].message, /missing: RATE/);
```

## A list that agrees is a fact worth adding

```ts check
import { createProject } from 'docattest';

const config = JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' });
const project = (files: Record<string, string>) => createProject({ files: { 'docattest.config.json': config, ...files } });
// ---cut---
const source = `export const REGIONS = ['eu', 'us', 'ap'];`;
const page = 'Jobs can run in:\n\n- `ap`\n- `eu`\n- `us`\n';

const [s] = (await project({ 'src/index.ts': source, 'docs/regions.md': page }).audit()).suggestions;
assert.equal(s.status, 'match');
assert.equal(s.line, 3);
assert.deepEqual([s.missing, s.unexpected], [[], []]);

const pasted = page.replace('\n- `ap`', `\n${s.directive}\n- \`ap\``);
const report = await project({ 'src/index.ts': source, 'docs/regions.md': pasted }).check();
assert.equal(report.results[0].status, 'pass');
```

## What a surface offers: values, elements, keys, values of objects and fields of rows

For each export `audit` reads the value itself (a string, a number, or the elements of an
array or set), `Object.keys` and `Object.values` of a plain object, and each field that
every row of an array of objects has (`ROUTES.map((r) => r.path)`), or every value of an
object of objects (`Object.values(ERRORS).map((r) => r.status)`). Pairs of fields are read
joined by a space, the way a table writes `GET /jobs`
(`` ROUTES.map((r) => `${r.method} ${r.path}`) ``), for rows of at most six fields. A
numeric enum's keys leave out the reverse mapping. Functions and class instances are not
read. [`audit --inventory`](inventory.md) lists every value read, with its expression.

A table compares column by column, with item values taken as facts take them (the first
code span of a cell, else its text). One suggestion is made per table, for the column that
fits best, with `column=` naming it.

```ts check
import { createProject } from 'docattest';

const config = JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' });
const project = (files: Record<string, string>) => createProject({ files: { 'docattest.config.json': config, ...files } });
// ---cut---
const source = [
  "export const ROUTES = [{ method: 'GET', path: '/jobs' }, { method: 'POST', path: '/jobs' }, { method: 'GET', path: '/projects' }, { method: 'DELETE', path: '/jobs/:id' }];",
  "export const ErrorCodes = { RPC_ERROR: 'rpc', NO_SESSION: 'session', TIMEOUT: 'timeout' } as const;",
  'export enum Network { Production, Staging, Local }',
].join('\n');
const page = [
  '| Method | Path | Purpose |', '| --- | --- | --- |',
  '| GET | `/jobs` | list jobs |', '| POST | `/jobs` | create a job |', '| GET | `/projects` | list projects |', '',
  'Error codes:', '', '- `RPC_ERROR`', '- `NO_SESSION`', '- `TIMEOUT`', '',
  'Error values:', '', '- `rpc`', '- `session`', '- `timeout`', '',
  'Networks:', '', '- `Production`', '- `Staging`', '',
].join('\n');

const found = (await project({ 'src/index.ts': source, 'docs/api.md': page }).audit()).suggestions;
const brief = found.map((s) => [s.line, s.target, s.status, s.directive, s.missing]);
assert.deepEqual(brief, [
  [1, 'column', 'drift', '<!-- fact column=Path: ROUTES.map((r) => r.path) -->', ['/jobs/:id']],
  [9, 'list', 'match', '<!-- fact: Object.keys(ErrorCodes) -->', []],
  [15, 'list', 'match', '<!-- fact: Object.values(ErrorCodes) -->', []],
  [21, 'list', 'drift', '<!-- fact: Object.keys(Network).filter((k) => Number.isNaN(Number(k))) -->', ['Local']],
]);
```

## Nested objects and namespaces are read too

Pages copy values from deep inside a surface: the keys of an object a namespace re-exports
(`export * as Jobs from './jobs.js'`), or the property names of a schema four objects down.
`audit` walks plain objects and namespaces breadth first and reads every object it reaches
as it reads an export: its keys, its values, its lists and its scalars. A key that is not
an identifier is read with brackets (`Plans['free-tier']`). Fields of rows are read for an
export and its direct members, pairs of fields only for an export (a namespace adds no
level). The walk is bounded so a large surface stays fast:

- at most six property hops below an export;
- at most 100 members of one object descended into;
- at most 400 values from one export, shallowest first;
- lists, key sets and row sets of more than 500 items are not read;
- an object reached twice is read once.

A list item's value is its first code span, else a bold lead term (`**BASIC**: runs once`
reads as `BASIC`), else its text, the same rule `check` uses, so the directive pasted above
such a list works.

```ts check
import { createProject } from 'docattest';

const config = JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' });
const project = (files: Record<string, string>) => createProject({ files: { 'docattest.config.json': config, ...files } });
// ---cut---
const files = {
  'src/index.ts': [
    "export * as Jobs from './jobs.js';",
    "export const schemas = { components: { S3: { properties: { url: {}, bucket: {}, IAM: {}, allowWrite: {} } } } };",
    "export const Strategy = { BASIC: 'BASIC', 'BASIC-EXTEND': 'BASIC-EXTEND', TIMED: 'TIMED' };",
    "export const Plans = { 'free-tier': ['p1', 'p2', 'p3'] };",
  ].join('\n'),
  'src/jobs.ts': 'export const CLOSE_FIELDS = { job: 0, project: 1, owner: 2, approver: 3, settings: 4 };',
};
const page = [
  '| Name | Description |', '| --- | --- |',
  '| `job` | The job. |', '| `project` | The project. |', '| `owner` | Owns. |', '| `approver` | Approves. |', '',
  'S3 fields:', '', '- `url`', '- `bucket`', '- `IAM`', '',
  'Strategies:', '', '- **BASIC**: runs once.', '- **TIMED**: runs on a schedule.', '',
  'Plans:', '', '- `p1`', '- `p2`', '- `p3`', '',
].join('\n');

const found = (await project({ ...files, 'docs/p.md': page }).audit()).suggestions;
assert.deepEqual(found.map((s) => [s.line, s.target, s.status, s.expression, s.missing]), [
  [1, 'column', 'drift', 'Object.keys(Jobs.CLOSE_FIELDS)', ['settings']],
  [10, 'list', 'drift', 'Object.keys(schemas.components.S3.properties)', ['allowWrite']],
  [16, 'list', 'drift', 'Object.keys(Strategy)', ['BASIC-EXTEND']],
  [21, 'list', 'match', "Plans['free-tier']", []],
]);

// Pasted, the bold list fails `check` with the same missing item.
const pasted = page.replace('- **BASIC**', `${found[2].directive}\n- **BASIC**`);
const report = await project({ ...files, 'docs/p.md': pasted }).check();
assert.match(report.results[0].message, /missing: BASIC-EXTEND/);
```

## How close is close enough

A list, a column or a block's lines are compared with a surface list as sets, the way a fact
compares them: `missing` is in the code and not on the page, `unexpected` is on the page and
not in the code. Only surface lists that share two items with the page are considered, so
the cost of a page does not grow with the size of the surface.

- Both sides have at least two distinct items.
- At least two shared items are not trivial: one character, a single digit, or a word like
  `true`, `false`, `null`, `yes`, `no`, `on`, `off`.
- Shared items are at least half of all distinct items, page and code together (Jaccard
  0.5). With exactly two shared, at most one item may differ.
- The best fit wins: a deeper object whose keys fit better beats a small enum that shares
  a word or two. When two surface values fit equally well and disagree about what is
  missing, nothing is said, however deep they are. Of equal values, the default surface and
  the earlier export win.
- A list or column needs two distinct items to be compared at all.

Deeper reading multiplies the surface lists a page can meet by chance, so none of these
thresholds was loosened with it. A list sharing two items of five distinct ones stays
silent: two shared items with three differences look the same whether the list was copied
or merely uses two common words, and nothing in the set alone tells them apart.

A single value in a sentence is held to its own, stricter rules (next section): one value
equals unrelated exports by chance far more often than several do.

A fenced block that is not an example matches a string export of at least 16 characters
when the two are equal after collapsing whitespace, and is drift when they share lines by
the set rules above with at least three lines each.

```ts check
import { createProject } from 'docattest';

const config = JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' });
const audit = async (source: string, page: string) =>
  (await createProject({ files: { 'docattest.config.json': config, 'src/index.ts': source, 'docs/p.md': page } }).audit()).suggestions;
const list = (...items: string[]) => items.map((i) => `- \`${i}\``).join('\n') + '\n';
// ---cut---
// Three shared of six is half: drift. Three shared of seven is not.
assert.equal((await audit(`export const K = ['a1', 'a2', 'a3', 'a4', 'a5', 'a6'];`, list('a1', 'a2', 'a3')))[0].status, 'drift');
assert.deepEqual(await audit(`export const K = ['a1', 'a2', 'a3', 'a4', 'a5', 'a6', 'a7'];`, list('a1', 'a2', 'a3')), []);

// Two shared: one difference is drift, two differences are silence.
assert.deepEqual((await audit(`export const K = ['read', 'write', 'admin'];`, list('read', 'write')))[0].missing, ['admin']);
assert.deepEqual(await audit(`export const K = ['read', 'write', 'admin', 'owner'];`, list('read', 'write')), []);

// Trivial items are not evidence.
assert.deepEqual(await audit(`export const K = ['1', '2', 'x'];`, list('1', '2', 'x')), []);

// A deeper set that fits better wins over a small enum sharing two words.
const deep = `export const EventType = { JOB: 'job', QUEUE: 'queue', RUN: 'run' } as const;\nexport const Close = { fields: { job: 0, project: 1, owner: 2, archive: 3 } };`;
const [best] = await audit(deep, list('job', 'project', 'owner'));
assert.equal(best.expression, 'Object.keys(Close.fields)');

// Two unrelated sets that fit equally and disagree: silence.
const twins = `export const A = { x: { job: 0, project: 1, owner: 2, archive: 3 } };\nexport const B = { y: { job: 0, project: 1, owner: 2, run: 3 } };`;
assert.deepEqual(await audit(twins, list('job', 'project', 'owner')), []);

// A fenced block that copies a string export.
const template = 'export const TEMPLATE = `name: demo\\nregion: eu\\nport: 8080\\n`;';
const [block] = await audit(template, '```yaml\nname: demo\nregion: eu\nport: 8080\n```\n');
assert.deepEqual([block.target, block.status, block.line], ['block', 'match', 1]);
```

## Values copied into sentences

Pages copy values into prose as often as into lists: "tag a block `python`, `py` or
`pycon`", "set `DOCATTEST_CORE`". `audit` reads every sentence no directive binds (a
sentence ends at `.`, `!` or `?` before whitespace), in paragraphs anywhere but inside a list
or table that has a suggestion of its own.

A sentence whose code spans name two or more items is an enumeration written inline. It is
compared with the surface's lists by the rules a list is held to (above), and the suggestion
(target `inline`, at the line the sentence ends on) is a fact at the end of the sentence,
which binds the sentence's code spans as a set.

A single value is evidence only when it could hardly be equal by chance. A code span, or a
bare number in the text, is suggested when it is distinctive and exactly one value of the
surfaces equals it:

- a number is distinctive with at least three digits (`8080`, `0.125`), never `3` or `90`,
  which equal a count or an index;
- text is distinctive with at least four characters, when its core (ignoring leading or
  trailing punctuation) is not a plain word or short number: it holds internal punctuation
  or digits (`DOCATTEST_CORE`, `batch-large`, `docattest.config.json`, `/etc/ld.so.preload`,
  `:8080`), or starts with a lowercase letter and has camelCase (`maxRetries`). `queue`,
  `Production`, `PoolAccount`, `JSON`, `/proc` and `.git` are plain words or type/enum
  names and match unrelated prose too often;
- a value two surface values share (`eur-cents` as both `CURRENCY` and `Money.unit`) says
  nothing about which one the page copied, so it is never suggested.

The directive goes right after the value, where it is compared with that value alone, and
the suggestion (target `scalar`) says in `after` how the page writes it. A page mentions a
value more often than it states it, so each surface value is suggested once per page, where
it first appears, and not at all on a page where a fact already reads it. The pasted
directive is a working fact.

```ts check
import { createProject } from 'docattest';

const config = JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' });
const project = (files: Record<string, string>) => createProject({ files: { 'docattest.config.json': config, ...files } });
// ---cut---
const source = [
  "export const LANGUAGES = ['python', 'py', 'pycon'];",
  "export const CORE_VARIABLE = 'DOCATTEST_CORE';",
  'export const DEFAULT_PORT = 8080;',
  'export const RETRIES = 3;',
  "export const EventType = { QUEUE: 'queue' };",
  "export const CURRENCY = 'eur-cents';",
  "export const Money = { unit: 'eur-cents' };",
].join('\n');
const page = [
  "Blocks tagged `python`, `py` or `pycon` are the adapter's. Set `DOCATTEST_CORE` first.",
  '',
  'The server listens on port 8080 and tries 3 times.',
  '',
  'Events go to `queue`. Prices are in `eur-cents`. Without `DOCATTEST_CORE`, the launcher looks elsewhere.',
  '',
].join('\n');

const { suggestions } = await project({ 'src/index.ts': source, 'docs/p.md': page }).audit();
assert.deepEqual(suggestions.map((s) => [s.line, s.target, s.status, s.directive]), [
  [1, 'inline', 'match', '<!-- fact: LANGUAGES -->'],
  [1, 'scalar', 'match', '<!-- fact: CORE_VARIABLE -->'],
  [3, 'scalar', 'match', '<!-- fact: DEFAULT_PORT -->'],
]);
assert.deepEqual(suggestions.flatMap((s) => (s.target === 'scalar' ? [s.after] : [])), ['`DOCATTEST_CORE`', '8080']);

// Pasted right after the value, the directive is a fact that holds.
const pasted = page.replace('8080', `8080 ${suggestions[2].directive}`);
const report = await project({ 'src/index.ts': source, 'docs/p.md': pasted }).check();
assert.deepEqual(report.results.map((r) => [r.line, r.status]), [[3, 'pass']]);

// A sentence that names most of a list is drift, by the rules of a list.
const drift = await project({ 'src/index.ts': source, 'docs/p.md': 'Tag a block `python` or `pycon`.\n' }).audit();
assert.deepEqual(drift.suggestions.map((s) => [s.target, s.status, s.missing]), [['inline', 'drift', ['py']]]);
```

## What is never suggested

Anything a value-binding directive (`fact` or `cases`) already binds, a sentence included
(`<!-- test: ... -->` citations attest behaviour rather than literal values, so they leave
distinctive values open to audit), anything on a frozen page, and anything inside fenced
code, where a list is code rather than a claim.

```ts check
import { createProject } from 'docattest';

const audit = async (files: Record<string, string>, frozen: string[] = []) => {
  const config = JSON.stringify({ pages: ['docs/**/*.md'], frozen, surfaces: { default: './src/index.ts' }, adapter: 'typescript' });
  return (await createProject({ files: { 'docattest.config.json': config, ...files } }).audit()).suggestions;
};
const source = `export const REGIONS = ['eu', 'us', 'ap'];\nexport const DEFAULT_QUEUE = 'batch-large';`;
// ---cut---
const bound = 'Jobs can run in: <!-- fact: REGIONS -->\n- `eu`\n- `us`\n\nJobs go to `batch-large`. <!-- fact: DEFAULT_QUEUE -->\n';
assert.deepEqual(await audit({ 'src/index.ts': source, 'docs/p.md': bound }), []);

const copied = 'Jobs can run in:\n\n- `eu`\n- `us`\n- `ap`\n';
assert.deepEqual(await audit({ 'src/index.ts': source, 'docs/old/p.md': copied }, ['docs/old/**']), []);
assert.deepEqual(await audit({ 'src/index.ts': source, 'docs/p.md': `---\ndocattest: false\n---\n\n${copied}` }), []);

const fenced = '```md\n- `eu`\n- `us`\n- `ap`\n\nJobs go to `batch-large`.\n```\n';
assert.deepEqual(await audit({ 'src/index.ts': source, 'docs/p.md': fenced }), []);
```

## The command line prints suggestions by page and exits 0

`docattest audit` prints each page, then one `?` line per suggestion with the directive and
where it goes, then a summary. `--json` prints `{ suggestions }`. The exit code is `0`
whatever it finds, and nothing is written. A surface that cannot be loaded is an error in
every mode (including `--inventory`): exit code `1`, and the cause on standard error after
`docattest: audit:`.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-audit-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('src/index.ts', `export const SCOPES = ['read', 'write', 'admin'];\nexport const DEFAULT_QUEUE = 'batch-large';`);
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('docs/scopes.md', '# Scopes\n\nA token may carry:\n\n- `read`\n- `write`\n- `admin`\n- `billing`\n\nJobs go to `batch-large`.\n');
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8' });
// ---cut---
const text = docattest('audit');
assert.equal(text.status, 0);
assert.equal(text.stdout, [
  'docs/scopes.md',
  '  ? L5 list drifts from SCOPES (unexpected: billing) → <!-- fact: SCOPES --> above the list',
  '  ? L10 scalar matches DEFAULT_QUEUE → <!-- fact: DEFAULT_QUEUE --> after `batch-large`',
  'audit: 2 suggestions on 1 page, 1 match, 1 drift',
  '',
].join('\n'));

const json = docattest('audit', '--json');
assert.equal(json.status, 0);
assert.deepEqual(JSON.parse(json.stdout).suggestions.map((s: { line: number }) => s.line), [5, 10]);
assert.deepEqual(readdirSync(join(root, 'docs')), ['scopes.md']);

put('src/index.ts', `import 'a-package-that-is-not-installed';\nexport const SCOPES = ['read'];`);
for (const args of [['audit'], ['audit', '--json'], ['audit', '--inventory']]) {
  const broken = docattest(...args);
  assert.equal(broken.status, 1);
  assert.equal(broken.stdout, '');
  assert.match(broken.stderr, /^docattest: audit: surface "default" failed to load: .*a-package-that-is-not-installed/);
}
```

## `--apply` inserts directives directly into pages

`docattest audit --apply` writes matching (`match`) suggestions straight into the Markdown
or MDX files (`<!-- fact: ... -->` in `.md`, `{/* fact: ... */}` in `.mdx`), leaving drifted
suggestions untouched so CI stays green while you inspect them. Pass `--include-drift` to
insert directives on drifted claims too (never rewriting the prose itself, so `check`
immediately flags them), or `--dry-run` to preview edits without writing.

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-audit-apply-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('src/index.ts', `export const SCOPES = ['read', 'write', 'admin'];\nexport const DEFAULT_QUEUE = 'batch-large';`);
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md', 'docs/**/*.mdx'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('docs/scopes.md', '# Scopes\n\nA token may carry:\n\n- `read`\n- `write`\n- `admin`\n- `billing`\n\nJobs go to `batch-large`.\n');
put('docs/queue.mdx', '# Queue\n\nJobs go to `batch-large`.\n');
const docattest = (...args: string[]) => spawnSync(process.execPath, [cli, ...args, '--root', root], { encoding: 'utf8' });
// ---cut---
const applied = docattest('audit', '--apply');
assert.equal(applied.status, 0);
assert.match(applied.stdout, /applied 2 directives to 2 pages \(1 drift skipped/);
assert.match(readFileSync(join(root, 'docs/scopes.md'), 'utf8'), /`batch-large` <!-- fact: DEFAULT_QUEUE -->/);
assert.doesNotMatch(readFileSync(join(root, 'docs/scopes.md'), 'utf8'), /<!-- fact: SCOPES -->/);
assert.match(readFileSync(join(root, 'docs/queue.mdx'), 'utf8'), /`batch-large` \{\/\* fact: DEFAULT_QUEUE \*\/\}/);
// Applied matches pass check immediately.
assert.equal(docattest('check').status, 0);

// With --include-drift, the drifted list gets its directive too and check catches it.
assert.equal(docattest('audit', '--apply', '--include-drift').status, 0);
assert.match(readFileSync(join(root, 'docs/scopes.md'), 'utf8'), /<!-- fact: SCOPES -->\n- `read`/);
assert.equal(docattest('check').status, 1);
```
