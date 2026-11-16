---
title: The adapter protocol
---

# The adapter protocol

A language adapter can run as its own program, driven by the core through the protocol in
[PROTOCOL.md](../protocol/PROTOCOL.md): one JSON request per line on its stdin, one JSON response per line on its
stdout. The TypeScript adapter ships as such a program, `docattest-adapter-typescript`,
which the `docattest` command starts for the core. Every example on this page
starts the built server, sends it requests and reads what it wrote, the way a core would.

## `hello` comes first and says what the adapter speaks

The answer to `hello` names the protocol version, the adapter, the info-string languages
whose fenced blocks it runs, the project files that are its source files, the names that
are never the project's own, and the optional methods it implements. The TypeScript adapter implements all three: `tests`, `observables`
and `affected`. A protocol version it does not speak is a `user` error, and the server keeps
running.

The source files are given as globs, `include` minus `exclude`, which the core applies to
project-relative paths the way it applies the config's own globs. So the core never holds a
language's file extensions: for TypeScript they are the scripts of every flavour, without
the declaration files. The names it ignores are given the same way, as data: a path with a
segment called `node_modules` is not a project file. Each is a glob for one name, so
another adapter can say `*.egg-info`. The language's own words come as data too: the
TypeScript adapter calls an enclosing title that is not a literal `(computed title)`.

The adapter answers `hello` before it loads the TypeScript compiler, so a core that needs
nothing else, as an edit hook does for a file that is not source code, is answered at once.
A core that knows more requests will follow says so with `warm`, and the adapter starts
loading the compiler as soon as it has answered; the answer itself is the same.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const server = fileURLToPath(new URL('../adapters/typescript/dist/protocol/main.js', import.meta.url));
const talk = (...lines: unknown[]) => {
  const input = lines.map((l) => (typeof l === 'string' ? l : JSON.stringify(l))).join('\n') + '\n';
  const out = spawnSync(process.execPath, [server], { input, encoding: 'utf8' });
  const responses = out.stdout.split('\n').filter(Boolean).map((l): { id: unknown; result?: any; error?: any } => JSON.parse(l));
  return { status: out.status, stdout: out.stdout, stderr: out.stderr, responses, of: (id: unknown) => responses.find((r) => r.id === id) };
};
// ---cut---
const run = talk(
  { id: 1, method: 'hello', params: { protocol: 1 } },
  { id: 2, method: 'hello', params: { protocol: 2 } },
);
const warm = talk({ id: 1, method: 'hello', params: { protocol: 1, warm: true } });

const hello = run.of(1)?.result;
assert.equal(hello.protocol, 1);
assert.equal(hello.adapter.name, 'typescript');
assert.ok(hello.blockLanguages.includes('ts'));
assert.deepEqual(hello.sourceFiles, {
  include: ['**/*.{ts,tsx,mts,mtsx,cts,ctsx,js,jsx,mjs,mjsx,cjs,cjsx}'],
  exclude: ['**/*.d.{ts,mts,cts}'],
});
assert.deepEqual(hello.ignoredNames, ['node_modules']);
assert.equal(hello.vocabulary.untitled, '(computed title)');
assert.equal(hello.infoWords, undefined);
assert.deepEqual(hello.capabilities, ['tests', 'observables', 'affected']);
assert.deepEqual(warm.of(1)?.result, hello);

assert.deepEqual(run.of(2)?.error, {
  kind: 'user',
  message: 'docattest: the typescript adapter speaks protocol 1, not 2',
});
assert.equal(run.status, 0);
```

## Questions about text need no project

The core never reads source code, so whatever depends on the language's syntax it asks.
These methods need no session, take many items in one request and answer one for each, in
order. `assertions` says whether each example asserts anything: one that does not is weak.
`docProse` reads the doc comments of source files as prose, each with the lines it spans.
`mentions` says which of some symbols each inline code span names: `retry(fn)` names
`retry`, and `retry.ts` is a file. `expressions` gives each fact expression's shape and its
names, so the core can tell a rename (the same shape, other names) from another expression.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const server = fileURLToPath(new URL('../adapters/typescript/dist/protocol/main.js', import.meta.url));
const talk = (...lines: unknown[]) => {
  const input = lines.map((l) => (typeof l === 'string' ? l : JSON.stringify(l))).join('\n') + '\n';
  const out = spawnSync(process.execPath, [server], { input, encoding: 'utf8' });
  const responses = out.stdout.split('\n').filter(Boolean).map((l): { id: unknown; result?: any; error?: any } => JSON.parse(l));
  return { status: out.status, stdout: out.stdout, stderr: out.stderr, responses, of: (id: unknown) => responses.find((r) => r.id === id) };
};
// ---cut---
const source = '/**\n * Doubles a number.\n * @example\n * // check\n */\nexport const double = 1;';
const run = talk(
  { id: 1, method: 'hello', params: { protocol: 1 } },
  { id: 2, method: 'assertions', params: { examples: [{ lang: 'ts', code: 'assert.equal(1, 1);' }, { lang: 'ts', code: 'console.log(1);' }] } },
  { id: 3, method: 'docProse', params: { files: [{ page: 'src/a.ts', text: source }, { page: 'docs/a.md', text: source }] } },
  { id: 4, method: 'mentions', params: { spans: ['retry(fn)', 'new Client()', 'retry.ts'], symbols: ['retry', 'Client'] } },
  { id: 5, method: 'expressions', params: { expressions: ['Math.min(LIMIT, 3)', 'Math.min(MAX_TRIES, 3)', 'LIMIT + 3'] } },
);

assert.deepEqual(run.of(2)?.result, [true, false]);
assert.deepEqual(run.of(3)?.result, [[{ text: 'Doubles a number.', start: 1, end: 5 }], []]);
assert.deepEqual(run.of(4)?.result, ['retry', 'Client', null]);
const [before, renamed, other] = run.of(5)?.result;
assert.deepEqual(before.names, ['Math', 'min', 'LIMIT']);
assert.equal(renamed.shape, before.shape);
assert.notEqual(other.shape, before.shape);
assert.equal(run.status, 0);
```

## A session answers in the shapes the API uses

`open` creates the one session of the process. With `files` the project is held in memory;
without, it is read from `root`. A surface is given as the config states it, a name and an
entry: here a path, which the TypeScript adapter imports as `docattest-fixture` in an
in-memory project. The session's answers are the JSON forms of the API's own
types: a fact's `Evaluation`, an example's `Diagnostic`, a surface's export names. `inCode`
says which of some names the project's code still holds, in the files it is given and in
the files of the surfaces. When
stdin ends, the server waits for its answers, closes the session and exits with code `0`.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const server = fileURLToPath(new URL('../adapters/typescript/dist/protocol/main.js', import.meta.url));
const talk = (...lines: unknown[]) => {
  const input = lines.map((l) => (typeof l === 'string' ? l : JSON.stringify(l))).join('\n') + '\n';
  const out = spawnSync(process.execPath, [server], { input, encoding: 'utf8' });
  const responses = out.stdout.split('\n').filter(Boolean).map((l): { id: unknown; result?: any; error?: any } => JSON.parse(l));
  return { status: out.status, stdout: out.stdout, stderr: out.stderr, responses, of: (id: unknown) => responses.find((r) => r.id === id) };
};
const base = { kind: 'example', page: 'docs/a.md', line: 3, claim: '', lang: 'ts', mode: 'compile', network: false, weak: false };
const fact = { id: 'f', kind: 'fact', page: 'docs/a.md', line: 1, claim: '', expression: 'N + 1', surface: 'default', ordered: false };
// ---cut---
const typo = { ...base, id: 'e', code: "import { M } from 'docattest-fixture';" };
const run = talk(
  { id: 1, method: 'hello', params: { protocol: 1 } },
  { id: 2, method: 'open', params: {
    root: '/not/read', files: { 'src/index.ts': 'export const N = 3;' },
    surfaces: [{ name: 'default', entry: './src/index.ts' }],
    timeoutMs: 10000, workers: 2, network: false, snapshots: { data: {} }, manifests: {}, examples: [typo],
  } },
  { id: 3, method: 'evaluate', params: { facts: [fact] } },
  { id: 4, method: 'compile', params: { examples: [typo] } },
  { id: 5, method: 'exports', params: { surface: 'default' } },
  { id: 6, method: 'exports', params: { surface: 'other' } },
  { id: 7, method: 'inCode', params: { names: ['N', 'M'], files: [] } },
);

assert.deepEqual(run.of(2)?.result, {});
assert.deepEqual(run.of(3)?.result, { f: { ok: true, value: { text: '4', description: 'number 4' } } });
assert.equal(run.of(4)?.result[0].id, 'e');
assert.match(run.of(4)?.result[0].message, /has no exported member 'M'/);
assert.deepEqual(run.of(5)?.result, ['N']);
assert.equal(run.of(6)?.result, null);
assert.deepEqual(run.of(7)?.result, ['N']);
assert.equal(run.status, 0);
```

## Answers arrive as requests finish

The core may send several requests before reading any answer, and the server starts each
one as soon as its line arrives. A request that finishes first is answered first, whatever
order the requests were sent in; the id says which answer is which.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const server = fileURLToPath(new URL('../adapters/typescript/dist/protocol/main.js', import.meta.url));
const talk = (...lines: unknown[]) => {
  const input = lines.map((l) => (typeof l === 'string' ? l : JSON.stringify(l))).join('\n') + '\n';
  const out = spawnSync(process.execPath, [server], { input, encoding: 'utf8' });
  const responses = out.stdout.split('\n').filter(Boolean).map((l): { id: unknown; result?: any; error?: any } => JSON.parse(l));
  return { status: out.status, stdout: out.stdout, stderr: out.stderr, responses, of: (id: unknown) => responses.find((r) => r.id === id) };
};
const surfaces = [{ name: 'default', entry: './src/index.ts' }];
const open = { root: '/not/read', files: { 'src/index.ts': 'export const N = 3;' }, surfaces, timeoutMs: 10000, workers: 2, network: false };
// ---cut---
const slow = {
  id: 'slow', kind: 'example', page: 'docs/a.md', line: 1, claim: '', lang: 'ts', mode: 'check',
  network: false, weak: false, code: 'await new Promise((done) => setTimeout(done, 300));',
};
const run = talk(
  { id: 1, method: 'hello', params: { protocol: 1 } },
  { id: 2, method: 'open', params: { ...open, examples: [slow] } },
  { id: 3, method: 'run', params: { examples: [slow], cases: [] } },
  { id: 4, method: 'exports', params: { surface: 'default' } },
);

assert.deepEqual(run.responses.map((r) => r.id).slice(-2), [4, 3]);
assert.equal(run.of(3)?.result[0].status, 'pass');
```

## Project mistakes are `user` errors; protocol mistakes are `internal`

A surface that cannot be loaded is a mistake in the project, so `observables` answers it
with a `user` error, whose message the core prints as it is. A line that is not JSON, an
unknown method, a session method before `open` and a second `open` are mistakes in the
core: each gets an `internal` error, with a `null` id when the line had no readable one,
and the server goes on answering.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const server = fileURLToPath(new URL('../adapters/typescript/dist/protocol/main.js', import.meta.url));
const talk = (...lines: unknown[]) => {
  const input = lines.map((l) => (typeof l === 'string' ? l : JSON.stringify(l))).join('\n') + '\n';
  const out = spawnSync(process.execPath, [server], { input, encoding: 'utf8' });
  const responses = out.stdout.split('\n').filter(Boolean).map((l): { id: unknown; result?: any; error?: any } => JSON.parse(l));
  return { status: out.status, stdout: out.stdout, stderr: out.stderr, responses, of: (id: unknown) => responses.find((r) => r.id === id) };
};
const surfaces = [{ name: 'default', entry: './src/index.ts' }];
const open = { root: '/not/read', files: { 'src/index.ts': "throw new Error('boom');" }, surfaces, timeoutMs: 10000, workers: 2, network: false };
// ---cut---
const run = talk(
  { id: 1, method: 'hello', params: { protocol: 1 } },
  'this is not JSON',
  { id: 2, method: 'teleport', params: {} },
  { id: 3, method: 'exports', params: { surface: 'default' } },
  { id: 4, method: 'open', params: open },
  { id: 5, method: 'open', params: open },
  { id: 6, method: 'observables', params: { surface: 'default' } },
);

assert.deepEqual(run.of(null)?.error, { kind: 'internal', message: 'a request line is not JSON: this is not JSON' });
assert.deepEqual(run.of(2)?.error, { kind: 'internal', message: 'unknown method "teleport"' });
assert.deepEqual(run.of(3)?.error, { kind: 'internal', message: '"exports" before "open"' });
assert.deepEqual(run.of(4)?.result, {});
assert.equal(run.of(5)?.error.kind, 'internal');
assert.deepEqual(run.of(6)?.error, {
  kind: 'user',
  message: 'docattest: audit: surface "default" failed to load: Error: boom',
});
assert.equal(run.status, 0);
```

## Stdout carries nothing but responses

Attested code may print, and so may a surface module when it is loaded. Neither reaches
stdout. Examples, facts and surfaces run on worker threads whose `console` and
`process.stdout` the adapter drops. Code
that writes to descriptor 1 directly lands on stderr: `docattest-adapter-typescript` runs
the server as a child whose descriptor 1 is the command's stderr, and hands the server the
command's stdout as descriptor 3, which only responses are written to. Every line on stdout
is a response.

```ts check
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const server = fileURLToPath(new URL('../adapters/typescript/dist/protocol/main.js', import.meta.url));
const talk = (...lines: unknown[]) => {
  const input = lines.map((l) => (typeof l === 'string' ? l : JSON.stringify(l))).join('\n') + '\n';
  const out = spawnSync(process.execPath, [server], { input, encoding: 'utf8' });
  const responses = out.stdout.split('\n').filter(Boolean).map((l): { id: unknown; result?: any; error?: any } => JSON.parse(l));
  return { status: out.status, stdout: out.stdout, stderr: out.stderr, responses, of: (id: unknown) => responses.find((r) => r.id === id) };
};
const surfaces = [{ name: 'default', entry: './src/index.ts' }];
const fact = { id: 'f', kind: 'fact', page: 'docs/a.md', line: 1, claim: '', expression: 'N', surface: 'default', ordered: false };
const base = { id: 'e', kind: 'example', page: 'docs/a.md', line: 3, claim: '', lang: 'ts', mode: 'check', network: false, weak: false };
// ---cut---
const surface = "console.log('loading the surface'); process.stdout.write('raw surface output\\n'); export const N = 3;";
const loud = { ...base, code: "import { writeSync } from 'node:fs'; console.log('hello from an example'); process.stdout.write('raw example output\\n'); writeSync(1, 'straight to descriptor 1\\n');" };
const run = talk(
  { id: 1, method: 'hello', params: { protocol: 1 } },
  { id: 2, method: 'open', params: { root: '/not/read', files: { 'src/index.ts': surface }, surfaces, timeoutMs: 10000, workers: 2, network: false, examples: [loud] } },
  { id: 3, method: 'evaluate', params: { facts: [fact] } },
  { id: 4, method: 'run', params: { examples: [loud], cases: [] } },
  { id: 5, method: 'close', params: {} },
);

assert.equal(run.of(3)?.result.f.value.text, '3');
assert.equal(run.of(4)?.result[0].status, 'pass');
assert.deepEqual(run.of(5)?.result, {});
for (const line of run.stdout.split('\n').filter(Boolean)) assert.ok('id' in JSON.parse(line), line);
assert.doesNotMatch(run.stdout, /loading the surface|raw surface output|raw example output|hello from|straight to/);
assert.match(run.stderr, /straight to descriptor 1/);
```

## Why

The core is written in Rust, once, for every language, and an adapter has to be written in
the language it serves. A process boundary with a small line protocol lets each side be
written in its own language and tested on its own, and lets a core drive an adapter for a
language it has never heard of.
