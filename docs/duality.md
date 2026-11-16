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
sentence's paragraph, list item or heading up to the directive; for a fenced block or a
directive on its own line, the paragraph or heading immediately before it.

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
      'A call is retried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->',
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
