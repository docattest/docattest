---
title: Duality: from code to docs
---

# Duality: from code to docs

Every attestation knows which surface symbols it exercises: the imports in an example,
the identifiers in a fact, the documented export of a doc comment. The same parse that
checks the pages therefore yields the reverse map, and it cannot drift because nothing
stores it. For an agent about to change a symbol, this is how it finds the prose that
change will affect.

## `where` lists the pages, lines and claims for a symbol

The claim is the prose the attestation defends: for a directive that ends a sentence, that
sentence's paragraph, list item or heading up to the directive; for a directive alone on the
line directly above a sentence, that sentence; for a fenced block or any other directive on
its own line, the paragraph or heading immediately before it.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const retry = async <T>(fn: () => Promise<T>) => fn();\nexport const RETRY_DEFAULTS = { maxAttempts: 3 };\nexport const unused = 1;`,
    'docs/retry.md': [
      '# Retry', '',
      'A successful call returns its value unchanged.', '',
      '```ts check',
      "import { retry } from 'docattest-fixture';",
      "assert.equal(await retry(async () => 'ok'), 'ok');",
      '```', '',
      'A call is retried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->', '',
      '<!-- fact: RETRY_DEFAULTS.maxAttempts -->',
      'It gives up after `3` attempts. Then it throws.',
    ].join('\n'),
  },
});

const refs = await project.where('retry');
assert.equal(refs.length, 1);
assert.equal(refs[0].page, 'docs/retry.md');
assert.equal(refs[0].line, 5);
assert.equal(refs[0].kind, 'example');
assert.equal(refs[0].claim, 'A successful call returns its value unchanged.');

const defaults = await project.where('RETRY_DEFAULTS');
assert.equal(defaults[0].kind, 'fact');
assert.equal(defaults[0].line, 10);
assert.equal(defaults[0].claim, 'A call is retried at most `3` times.');
assert.equal(defaults[1].line, 12);
assert.equal(defaults[1].claim, 'It gives up after `3` attempts.');
```

## The claim comes from the attestation's own container

The paragraph before a directive's own paragraph is not its claim; nor is the prose of the
page around a list or a table. A directive or block inside a list item claims the
paragraph before it in that item, and a fact in a table cell claims the paragraph before
the table.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const double = (n: number) => n * 2;\nexport const KEYS = ['a', 'b'];\nexport const LIMITS = { tries: 3 };`,
    'docs/claims.md': [
      'Plain prose.', '',
      'Doubling `4` gives `8`. <!-- fact: double(4) -->', '',
      'The keys are these.', '',
      '<!-- fact: KEYS -->', '- `a`', '- `b`', '',
      'Limits per call:', '',
      '| limit | value |', '|---|---|', '| tries | `3` <!-- fact: LIMITS.tries --> |', '',
      '- Doubling zero gives zero.', '',
      '  ```ts check',
      "  import { double } from 'docattest-fixture';",
      '  assert.equal(double(0), 0);',
      '  ```',
    ].join('\n'),
  },
});

const claims = async (symbol: string) => (await project.where(symbol)).map((r) => [r.line, r.claim]);
assert.deepEqual(await claims('double'), [
  [3, 'Doubling `4` gives `8`.'],
  [19, 'Doubling zero gives zero.'],
]);
assert.deepEqual(await claims('KEYS'), [[7, 'The keys are these.']]);
assert.deepEqual(await claims('LIMITS'), [[15, 'Limits per call:']]);
```

Within its paragraph, a directive claims only the sentence it sits in: the sentences before
it are other claims. A comment on its own line is invisible to a reader, so a table or list
it binds still claims the paragraph before that comment.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const LIMITS = { tries: 3, wait: 100 };\nexport const KEYS = ['a', 'b'];`,
    'docs/claims.md': [
      'A call is tried `3` times. <!-- fact: LIMITS.tries --> It waits `100` ms between. <!-- fact: LIMITS.wait -->', '',
      'The config takes these keys:', '',
      '<!-- fact column=key: KEYS -->',
      '| key | meaning |', '|---|---|', '| `a` | first |', '| `b` | second <!-- fact: KEYS[1] --> |',
    ].join('\n'),
  },
});

const claims = async (symbol: string) => (await project.where(symbol)).map((r) => [r.line, r.claim]);
assert.deepEqual(await claims('LIMITS'), [
  [1, 'A call is tried `3` times.'],
  [1, 'It waits `100` ms between.'],
]);
assert.deepEqual(await claims('KEYS'), [
  [5, 'The config takes these keys:'],
  [9, 'The config takes these keys:'],
]);
```

## `uncovered` lists public symbols no page attests

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const a = 1;\nexport const b = 2;\nexport type Shape = { x: number };`,
    'docs/a.md': 'a is `1`. <!-- fact: a -->',
  },
});

assert.deepEqual(await project.uncovered(), ['Shape', 'b']);
const report = await project.check();
assert.equal(report.surfaces.default.symbols, 3);
assert.equal(report.surfaces.default.attested, 1);
assert.deepEqual(report.surfaces.default.unattested, ['Shape', 'b']);
```

## Symbols not meant to be documented are counted apart

Some exports are plumbing that readers never call. The config's `undocumented` names them,
by surface, as globs over the names `uncovered` prints, and `uncovered` leaves out those no
page attests. The surface line counts them after the ones left to attest, and a glob that
matches no symbol the surface exports is a warning, so the list cannot go stale unseen. A
symbol a glob matches that a page attests anyway is counted as attested, like any other.
Here it is for [docs/.fixtures/undocumented](.fixtures/undocumented), whose config gives
`["internal*", "Program*", "debug*"]` for its `default` surface, and whose page attests
`RATE` and `ProgramName`:

```console check cwd=docs/.fixtures/undocumented
$ docattest uncovered
quote
$ docattest check
docs/shipping.md
  ✓ L3 fact: RATE is 3
  ✓ L5 fact: ProgramName is shop
results: 2 pass, 0 fail, 0 error, 0 skipped
coverage: 1 living page, 1 covered (2 attestations: 2 proofs, 0 of them weak, and 0 citations), 0 uncovered, 0 frozen
warning: "undocumented.default" has "debug*", which matches no symbol
surface default: 4 symbols, 2 attested, 1 not attested by any page, 1 not meant to be documented
  shop → src/index.ts (shop)
snapshots: none
ok
```

When no symbol is left to attest, `uncovered` says so, and, when the config gives the
surface a glob, says that the rest are not meant to be documented:

```ts check
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../adapters/typescript/dist/launcher/main.js', import.meta.url));
const root = mkdtempSync(join(tmpdir(), 'docattest-undocumented-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('src/index.ts', 'export const a = 1;\nexport const internalB = 2;');
put('docs/a.md', 'a is `1`. <!-- fact: a -->');
const uncovered = (undocumented: Record<string, string[]>) => {
  put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', undocumented }));
  return spawnSync(process.execPath, [cli, 'uncovered'], { cwd: root, encoding: 'utf8' }).stdout;
};
// ---cut---
assert.equal(uncovered({ default: ['internal*'] }), 'every public symbol is attested or not meant to be documented\n');
assert.equal(uncovered({}), 'internalB\n');
put('docs/b.md', 'b is `2`. <!-- fact: internalB -->');
assert.equal(uncovered({}), 'every public symbol is attested\n');
```

The report object lists them in the surface's `undocumented`, which is there only when the
config gives that surface a glob; `unattested` holds the rest. A name in `undocumented` that
is not one of the config's surfaces is a config error.

```ts check
import { createProject } from 'docattest';

const files = (undocumented: Record<string, string[]>) => ({
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', undocumented }),
  'src/index.ts': `export const a = 1;\nexport const b = 2;\nexport const internalC = 3;`,
  'docs/a.md': 'a is `1`. <!-- fact: a -->',
});

const report = await createProject({ files: files({ default: ['internal*'] }) }).check();
assert.equal(report.surfaces.default.symbols, 3);
assert.equal(report.surfaces.default.attested, 1);
assert.deepEqual(report.surfaces.default.unattested, ['b']);
assert.deepEqual(report.surfaces.default.undocumented, ['internalC']);
assert.equal(report.warnings, undefined);

const none = await createProject({ files: files({}) }).check();
assert.equal(none.surfaces.default.undocumented, undefined);

await assert.rejects(
  createProject({ files: files({ server: ['internal*'] }) }).check(),
  /"undocumented" has "server", which is not a surface \(configured: default\)/,
);
```

## The index is the whole map and is never written to disk

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const a = 1;\nexport const b = 2;`,
    'docs/ab.md': 'a is `1`. <!-- fact: a -->\n\nb is `2`. <!-- fact: b -->',
  },
});

const index = await project.index();
assert.deepEqual(Object.keys(index).sort(), ['a', 'b']);
assert.equal(index.b[0].line, 3);
assert.deepEqual(project.writes(), []);
```
