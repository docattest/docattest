---
title: Mutate: how strong each proof is
---

# Mutate: how strong each proof is

A proof that passes says the page and the code agree today. Whether it would fail if they
stopped agreeing is another question, and the only sure way to answer it is to break what
the proof reads and run it again. `docattest mutate [pages…]` does that for every proof that
passes: it changes one thing the proof reads at a time (a literal of a constant, a line of
a file a fact reads, a cell of its table), runs the proof against the change, and puts the
thing back. A proof some change makes fail is *killed*: it has teeth. A proof no change
makes fail *survived*: it reads the value without pinning it. A proof nothing could be
changed for is *not mutated*, with why. Here it is on a small project,
[docs/.fixtures/mutate](.fixtures/mutate), whose page pins a constant, a list and a file's
text, checks only the type of a value, and reads what a function returns:

```console check cwd=docs/.fixtures/mutate
$ docattest mutate
docs/limits.md
  ✓ L3 fact: killed by src/index.ts:1 RETRIES: 3 → 4 (1 mutant)
  ✗ L5 fact: survived 1 mutant
      src/index.ts:1 RETRIES: 3 → 4
  ✓ L7 fact: killed by src/index.ts:2 REGIONS: 'us' removed (3 mutants)
  - L9 fact: not mutated: timeoutMs is a function, class or type: what it gives is computed when it runs
  ✓ L11 fact: killed by VERSION:1: 1.2.0 → 1.2.1 (1 mutant)
mutants: 5; proofs: 3 killed, 1 survived, 1 not mutated
```

The survivor is the finding: `typeof RETRIES` is `number` whatever the limit is, so the
sentence "the limit is a number" would stay green if the limit changed. A page that is
new is worth one run of `mutate`; a survivor is a proof to make stricter, or a sentence that
claims less than it seems to.

## What is changed

What a proof reads of the code is the symbols the index has for it (`docattest where`
lists them), and changing a symbol's value is its language's business: the adapter answers
edits to the literals of the symbol's declaration, and the core makes each one alone, in a
copy of the project. A number gains one, a string gains an `x` before its closing quote, a
boolean is flipped, and a list loses its last item before each of its items is changed.
When a scalar constant is initialized from another name or expression with no literal of its
own (such as `const Period = time.Second` or `export const COPY = PORT`), the adapter offers
a type-guided expression edit (`2 * time.Second`, `PORT + 1`, `PREFIX + 'x'`, `!FLAG`).
[The protocol](../protocol/PROTOCOL.md) has the rule every adapter follows, and each
language's guide lists its own: [Python](../adapters/python/README.md),
[Go](../adapters/go/README.md), [Rust](../adapters/rust/README.md) and
[TypeScript](../adapters/typescript/README.md). At most 8 edits of one symbol are run, the
first in source order:

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const LETTERS = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];`,
    'docs/a.md': 'The first letter is `a`. <!-- fact: LETTERS[0] -->\n',
  },
});

const { proofs } = await project.mutate();
assert.deepEqual(proofs.map((p) => [p.status, p.status === 'killed' && p.killedBy, p.status !== 'notMutated' && p.mutants]), [
  ['killed', "src/index.ts:1 LETTERS: 'a' → 'ax'", 8],
]);
assert.deepEqual(project.writes(), []);
```

The files facts read, the core reads itself (`file`, `matched`, `manifest` and
`manifestField`, as [manifests](manifests.md) describes), so the core changes what those
calls give, the same in every language: each line of a file's text that is not blank, what a
regex matched (its last number counted up, or an `x` added), and each value of a manifest
(a number plus one, a string as text is, a boolean flipped, a list without its last item).
At most 64 changes of one call are run; each is one more evaluation in the project's own
session, so they cost little.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const N = 1;',
    Dockerfile: 'FROM scratch\nUSER 10001\n',
    'ci.json': JSON.stringify({ deploy: { replicas: 3 } }),
    'docs/a.md': [
      'It runs as user `10001`. <!-- fact: matched("Dockerfile", "^USER (\\\\d+)$") -->',
      '',
      'On `3` replicas. <!-- fact: manifestField("ci.json", "deploy.replicas") -->',
      '',
    ].join('\n'),
  },
});

const { proofs } = await project.mutate();
assert.deepEqual(proofs.map((p) => p.status === 'killed' && p.killedBy), [
  'Dockerfile: what "^USER (\\d+)$" matches: 10001 → 10002',
  'ci.json: deploy.replicas: 3 → 4',
]);
```

A row of a [cases table](cases.md) is a proof of its own, and the core changes one of its
cells at a time on a copy of the row: the content of the cell's last code span, else its
text, the last number in it counted up, else an `x` added. The first cell names the row and
is never changed. A row is killed when some change fails it, and a cell no change of which
fails its row is named (`ignoredCells`): the runner does not read that column, and the page
promises something no proof checks.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, setup: 'docs/docattest.setup.ts', adapter: 'typescript' }),
    'src/index.ts': 'export const double = (n: number) => n * 2;',
    'docs/docattest.setup.ts': [
      "import assert from 'node:assert/strict';",
      "import { double } from 'docattest-fixture';",
      'export const cases = { double: (row: Record<string, string>) => assert.equal(String(double(Number(row.n))), row.result) };',
    ].join('\n'),
    'docs/double.md': [
      '<!-- cases: double -->',
      '| case | n | result | note |',
      '|---|---|---|---|',
      '| one | 1 | 2 | the smallest |',
    ].join('\n'),
  },
});

const [row] = (await project.mutate()).proofs;
assert.ok(row?.status === 'killed');
assert.equal(row.row, 'one');
assert.equal(row.killedBy, 'cell n: 1 → 2');
assert.deepEqual(row.ignoredCells, ['note']);
```

Some proofs are never mutated, and the report says why: a proof that does not pass as it is
(a change could tell nothing), a [citation](citations.md) (docattest reads the test file and
does not run the test), a sample page (`md check`, whose fixture holds its own project), and
a proof whose symbols the adapter has no edit for: a function, class or type, whose value is
computed when it runs; a value declared with no mutable literal or scalar expression; a name
no file of the project's own declares, or one declared in a file that is not the project's
own (a build, a dependency).

A [`console check`](console.md) block is run once as its baseline, and each non-blank line
of its captured output is then mutated in memory against the block's expected transcript. A
vacuous block whose expected output matches everything (such as a lone `...`) survives; a
block with literal expected lines is killed, and any output lines hidden by `...` are
counted informatively (`hiddenLines` in `--json`) without failing `--fail-on-survivors`.

When a whole-file `file()` or whole-manifest `manifest()` fact survives (for example, when
it only checks a property unaffected by line edits or reads past the 64-change limit), the
report includes a `hint` suggesting `matched()` or `manifestField()`.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': 'export const N = 1;',
    Dockerfile: 'FROM scratch\nUSER 10001\n',
    'docs/a.md': [
      'Dockerfile is non-empty: `true`. <!-- fact: file("Dockerfile").length > 0 -->',
      '',
      '```console check',
      '$ printf "start\\nignored\\nend\\n"',
      'start',
      '...',
      'end',
      '```',
      '',
      '```console check',
      '$ printf "anything\\n"',
      '...',
      '```',
      '',
    ].join('\n'),
  },
});

const { proofs } = await project.mutate();
assert.equal(proofs[0]?.status, 'survived');
assert.match(proofs[0]?.status === 'survived' ? (proofs[0].hint ?? '') : '', /matched\("Dockerfile"/);
assert.equal(proofs[1]?.status, 'killed');
assert.equal(proofs[1]?.status === 'killed' ? proofs[1].hiddenLines : 0, 1);
assert.equal(proofs[2]?.status, 'survived');
```

## Mutating only what a diff changed (`--base`)

`docattest mutate --base <ref>` (and `project.mutate({ base })`) mutates only the proofs
that [`impact --base <ref>`](impact.md) says the diff affects plus any proof added or
modified on Markdown pages changed since `<ref>`. This keeps `--fail-on-survivors` cheap
on merge requests as a documentation suite grows.

```ts check
import { createProject } from 'docattest';

const baseFiles = {
  'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
  'src/index.ts': 'export const A = 1;\nexport const B = 2;\n',
  'docs/a.md': 'A is `1`. <!-- fact: A -->\n\nB is `2`. <!-- fact: B -->\n',
};

const headFiles = {
  ...baseFiles,
  'src/index.ts': 'export const A = 1;\nexport const B = 3;\n',
  'docs/a.md': 'A is `1`. <!-- fact: A -->\n\nB is `3`. <!-- fact: B -->\n',
};

const mutation = await createProject({ files: headFiles }).mutate({ base: { files: baseFiles } });
assert.deepEqual(mutation.proofs.map((p) => [p.line, p.status]), [[3, 'killed']]);
```

A change that leaves the code unable to build or load tells nothing about a proof: any
proof would fail on it. Such a mutant is *stillborn*, and kills nothing. The core tells it
apart without knowing the language, from what the adapter reports: an error, or an
example's compile diagnostics, that points at a line of the changed file, as a compiler or
a loader writes a place (`src/lib.rs:14:5`, `./shop.go:13:9`, `src/index.ts(3,5)`), or a
project that cannot be loaded at all. A value that differs, an assertion or a test that
fails, a panic while an example runs, and an error about something else (a key that is
gone) are how the changed code behaves, and kill. The adapters offer only changes they
expect to build, so stillborn mutants are few; the report counts them (`(3 mutants, 1
stillborn)`, `stillborn` in `--json`), and a proof whose every mutant was stillborn is not
mutated, with that reason. The [core's guide](../core/README.md#what-the-core-knows-of-a-language)
has the rule with its tests.

## Reading the report

`mutate` exits `0` whatever it finds: it measures, and `check` judges. With
`--fail-on-survivors` it exits `1` when a proof survived, for a CI job that holds a project
to proofs with teeth:

```console check cwd=docs/.fixtures/mutate
$ docattest mutate --fail-on-survivors docs/limits.md
docs/limits.md
  ✓ L3 fact: killed by src/index.ts:1 RETRIES: 3 → 4 (1 mutant)
  ✗ L5 fact: survived 1 mutant
      src/index.ts:1 RETRIES: 3 → 4
  ✓ L7 fact: killed by src/index.ts:2 REGIONS: 'us' removed (3 mutants)
  - L9 fact: not mutated: timeoutMs is a function, class or type: what it gives is computed when it runs
  ✓ L11 fact: killed by VERSION:1: 1.2.0 → 1.2.1 (1 mutant)
mutants: 5; proofs: 3 killed, 1 survived, 1 not mutated
[exit 1]
```

`mutate --json`, and `project.mutate({ pages })` in the programmatic API, give the same as an
object: `proofs`, each with its `page`, `line`, `kind`, `claim` (a row's `row`, its first
cell) and `status`; a killed proof's `killedBy`, the number of `mutants` run and its
`ignoredCells`; a survivor's `survivors`, every change it survived; for both, how many of
their mutants were `stillborn`; a proof not mutated's `why`; and a `summary` of the counts,
`stillborn` among them.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const RETRIES = 3;`,
    'docs/a.md': 'Tried `3` times. <!-- fact: RETRIES -->\n\nA `number`. <!-- fact: typeof RETRIES -->\n',
  },
});

const mutation = await project.mutate({ pages: ['docs/a.md'] });
assert.deepEqual(mutation.summary, { proofs: 2, killed: 1, survived: 1, notMutated: 0, mutants: 1, stillborn: 0 });
const [, survivor] = mutation.proofs;
assert.ok(survivor?.status === 'survived');
assert.deepEqual(survivor.survivors, ['src/index.ts:1 RETRIES: 3 → 4']);
```

## Where code is changed, and what it costs

The project itself is never written. Code is changed in copies of the project, made beside
its directory, in the directory above it: hidden, named after it
(`.shop.docattest-mutate-0-0` beside `shop/`), each with a lock file of the same name and
`.lock`. They are beside the project, not in a temporary directory, so that a path that
leads out of the project (`../shared`, a `node_modules` or a workspace in a directory above)
leads to the same place from a copy. For a project at the root of a repository, that is
beside the repository. A copy holds the project's own files; everything else (installed
dependencies, builds, what git ignores, hidden directories) is a link to the original,
through which no change is made. Each version of a file a copy holds gets a modification
time a second later than the last, so that a cache keyed by a source's time never takes one
version for another. As many copies work at once as `runtime.workers` allows, each with an
adapter of its own.

The copies are removed when `mutate` ends: when it is done, when it fails, and when it is
stopped with Ctrl-C or SIGTERM, before it exits. A run killed outright (SIGKILL) cannot
clean up; the next run finds its copies by their free locks and removes them, and leaves
alone those another run still holds. The [core's guide](../core/README.md#what-the-core-knows-of-a-language)
has the tests.

Two kinds of project cannot be copied beside themselves, and say so: their proofs are
reported as not passing in a copy of the project. A `go.work` in a directory above the
project that does not list the copy makes every Go command fail in it, and a Rust crate that
is a member of a workspace declared above it is not a member from beside it. A project that
holds its own `go.work`, or its own `[workspace]`, is copied whole and works.

A change to a file a fact reads, or to a cell, costs one more evaluation or run in the
project's own session. A change to code costs an adapter started on a copy and one run of
the proofs that read the changed symbol, and no others: a project whose adapter builds
programs (Go, Rust) pays a build for each, and the first in each copy pays for the
dependencies too, which the copies keep between runs, since a copy's name is the same from
run to run.
