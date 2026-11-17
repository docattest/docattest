---
title: Examples
---

# Examples

A fenced code block tagged `check` compiles and runs; tagged `compile` it only compiles;
untagged it is documentation and counts toward nothing. Assertions inside a `check` block
are the proof of the sentence above it. `assert` is the host language's standard
assertion and is in scope without an import.

Blocks of commands (`console check`) and sample pages (`md check`) are examples too, which
docattest runs itself whatever the project's language: [commands and sample
pages](console.md).

All examples on this page build a small in-memory project around one documentation
page and look at the result for that page.

## A passing assertion passes, a failing one fails at the block's line

```ts check
import { createProject } from 'docattest';

const page = (body: string) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const add = (a: number, b: number) => a + b;`,
    'docs/add.md': `# add\n\nAdds two numbers.\n\n\`\`\`ts check\n${body}\n\`\`\`\n`,
  },
});

const good = await createProject(page(`import { add } from 'docattest-fixture';\nassert.equal(add(2, 2), 4);`)).check();
assert.equal(good.results[0].status, 'pass');
assert.equal(good.results[0].kind, 'example');
assert.equal(good.results[0].line, 5);

const bad = await createProject(page(`import { add } from 'docattest-fixture';\nassert.equal(add(2, 2), 5);`)).check();
assert.equal(bad.results[0].status, 'fail');
assert.equal(bad.results[0].line, 5);
assert.match(bad.results[0].message, /block line 2/);
assert.match(bad.results[0].message, /4/);
assert.equal(bad.exitCode, 1);
```

## A block without assertions is weak coverage

It runs, and it passes if it does not throw, but the report counts it as `weak`: an
illustration, not a proof.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const greet = (n: string) => 'hi ' + n;`,
    'docs/greet.md': "```ts check\nimport { greet } from 'docattest-fixture';\nconsole.log(greet('x'));\n```\n",
  },
}).check();

assert.equal(report.results[0].status, 'pass');
assert.equal(report.coverage.weak, 1);
```

## `compile` blocks are type-checked and never executed

```ts check
import { createProject } from 'docattest';

const page = (body: string) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const add = (a: number, b: number) => a + b;`,
    'docs/add.md': `\`\`\`ts compile\n${body}\n\`\`\`\n`,
  },
});

const typeError = await createProject(page(`import { add } from 'docattest-fixture';\nadd('two', 2);`)).check();
assert.equal(typeError.results[0].status, 'fail');
assert.match(typeError.results[0].message, /string/);

const wouldThrow = await createProject(page(`import { add } from 'docattest-fixture';\nthrow new Error('never runs');\nadd(1, 2);`)).check();
assert.equal(wouldThrow.results[0].status, 'pass');
```

## Untagged blocks are not attestations

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/x.md': "```ts\nthis is not even valid\n```\n",
  },
}).check();

assert.equal(report.results.length, 0);
assert.deepEqual(report.coverage.uncovered, ['docs/x.md']);
```

## Lines above `// ---cut---` run but are setup

They are compiled and executed like the rest of the block; renderers that understand
the marker hide them. A failure above the cut is still a failure.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const add = (a: number, b: number) => a + b;`,
    'docs/add.md': "```ts check\nimport { add } from 'docattest-fixture';\nconst base = 40;\n// ---cut---\nassert.equal(add(base, 2), 42);\n```\n",
  },
}).check();

assert.equal(report.results[0].status, 'pass');
```

## Blocks are isolated from each other

Each block is its own module with a fresh import of the surface. Names do not collide
and state does not leak, so the order of blocks on a page never matters.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const counter = { n: 0 };`,
    'docs/c.md': [
      '```ts check',
      "import { counter } from 'docattest-fixture';",
      'const value = 1;',
      'counter.n++;',
      'assert.equal(counter.n, 1);',
      '```',
      '',
      '```ts check',
      "import { counter } from 'docattest-fixture';",
      'const value = 2;',
      'assert.equal(counter.n, 0);',
      'assert.equal(value, 2);',
      '```',
    ].join('\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['pass', 'pass']);
```

## Network is off unless a block asks for it

A `check` block that reaches the network fails. A `check network` block runs only when
the environment allows it and is otherwise `skipped`, which never changes the exit code.

```ts check
import { createProject } from 'docattest';

const page = (info: string) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/n.md': `\`\`\`${info}\nconst res = await fetch('https://example.com');\nassert.ok(res);\n\`\`\`\n`,
  },
  env: { network: false },
});

const blocked = await createProject(page('ts check')).check();
assert.equal(blocked.results[0].status, 'fail');
assert.match(blocked.results[0].message, /network/i);

const skipped = await createProject(page('ts check network')).check();
assert.equal(skipped.results[0].status, 'skipped');
assert.equal(skipped.exitCode, 0);
```

## A block that does not finish in time fails

The default budget is `10000` ms per block <!-- fact: DEFAULT_TIMEOUT_MS --> and
`runtime.timeoutMs` in the config overrides it.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', runtime: { timeoutMs: 50 } }),
    'src/index.ts': `export const x = 1;`,
    'docs/slow.md': "```ts check\nawait new Promise((r) => setTimeout(r, 500));\nassert.ok(true);\n```\n",
  },
}).check();

assert.equal(report.results[0].status, 'fail');
assert.match(report.results[0].message, /timed out/i);
```

## A block that times out is stopped, not abandoned

Blocks run on a pool of worker threads. When a block runs out of time its worker is
terminated, so a block stuck at an `await` or in a busy loop really stops: its timers never
fire and cannot disturb the blocks that run after it. A fresh worker takes its place.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', runtime: { timeoutMs: 200, workers: 1 } }),
    'src/index.ts': `export const x = 1;`,
    'docs/stuck.md': [
      '```ts check',
      'setTimeout(() => { (globalThis as { late?: boolean }).late = true; }, 250);',
      'await new Promise(() => {});',
      '```',
      '',
      '```ts check',
      'while (true) {}',
      '```',
      '',
      '```ts check',
      'await new Promise((resolve) => setTimeout(resolve, 150));',
      'assert.equal((globalThis as { late?: boolean }).late, undefined);',
      '```',
    ].join('\n'),
  },
}).check();

assert.deepEqual(report.results.map((r) => r.status), ['fail', 'fail', 'pass']);
assert.match(report.results[0].message, /timed out after 200 ms/);
assert.match(report.results[1].message, /timed out after 200 ms/);
```

`runtime.workers` sets the size of the pool; by default it is the number of CPUs Node
reports as available. Workers are reused from page to page, and each block still gets a
fresh module registry inside its worker.

## Renderer tags are allowed on proofs only when the config names them

Documentation sites add their own words to an info string, such as `twoslash` for type
hovers. On a block tagged `check` or `compile`, a word docattest does not know is an error
unless it is listed in `ignoreTags`, which accepts it and otherwise ignores it. On a block
with no tag of docattest's, a renderer's word leaves it documentation, counted toward
nothing; but a word that is likely a misspelling of `check`, `compile` or `network` (within
a third of the tag's length in edits, at least one, ignoring case) is an error that names
the tag it resembles, so a typo like `chekc` can never pass silently as prose. A bracketed
title such as `[TypeScript SDK]`, which code-group renderers use as a tab label, is always
ignored.

```ts check
import { createProject } from 'docattest';

const page = (info: string, ignoreTags?: string[]) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', ignoreTags }),
    'src/index.ts': `export const N = 3;`,
    'docs/n.md': `\`\`\`${info}\nimport { N } from 'docattest-fixture';\nassert.equal(N, 3);\n\`\`\`\n`,
  },
});

const typo = await createProject(page('ts chekc')).check();
assert.equal(typo.results[0].status, 'error');
assert.match(typo.results[0].message, /unknown tag "chekc"; did you mean "check"\?/);

const rendered = await createProject(page('ts twoslash')).check();
assert.equal(rendered.results.length, 0);
assert.deepEqual(rendered.coverage.uncovered, ['docs/n.md']);

const unlisted = await createProject(page('ts twoslash check')).check();
assert.equal(unlisted.results[0].status, 'error');

const listed = await createProject(page('ts twoslash check', ['twoslash'])).check();
assert.equal(listed.results[0].status, 'pass');

const titled = await createProject(page('ts check [TypeScript SDK]')).check();
assert.equal(titled.results[0].status, 'pass');

const titledWithSpaces = await createProject(page('ts twoslash check [Production (Default)]', ['twoslash'])).check();
assert.equal(titledWithSpaces.results[0].status, 'pass');
```

## A surface may depend on packages that only publish ES modules

Many packages declare only an `import` condition in their exports map. Examples and
facts can still use a surface that depends on one; docattest resolves through the
`import` condition when the `require` path is not exported.

```ts check
import { createProject } from 'docattest';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = mkdtempSync(join(tmpdir(), 'docattest-esm-'));
const put = (rel: string, text: string) => { mkdirSync(join(root, rel, '..'), { recursive: true }); writeFileSync(join(root, rel), text); };
put('node_modules/esm-only/package.json', JSON.stringify({ name: 'esm-only', type: 'module', exports: { '.': { types: './index.d.ts', import: './index.js' } } }));
put('node_modules/esm-only/index.js', 'export const answer = 42;');
put('node_modules/esm-only/index.d.ts', 'export declare const answer: number;');
put('package.json', JSON.stringify({ name: 'demo', type: 'module' }));
put('src/index.ts', "export { answer } from 'esm-only';");
put('docattest.config.json', JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }));
put('docs/answer.md', "The answer is `42`. <!-- fact: answer -->\n\n```ts check\nimport { answer } from 'demo';\nassert.equal(answer, 42);\n```\n");

const report = await createProject({ root }).check();
assert.deepEqual(report.results.map((r) => r.status), ['pass', 'pass']);
```
