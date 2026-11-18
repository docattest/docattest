---
title: docattest
---

# docattest

Documentation that CI can fail. A page carries its own proof: examples that run, facts
checked against the code, tables whose rows execute. When the prose stops being true,
the build fails at the page and line. The same parse links every public symbol back to
the pages that describe it. These pages are docattest's own documentation, written in the
pattern docattest checks, so they are also its acceptance tests.

## A project is checked from its files

`createProject` takes either a directory or an in-memory map of files, reads
`docattest.config.json`, and `check()` returns a report with one result per attestation.
Every operation runs the docattest core, the program the `docattest` command runs, and
returns what it prints with `--json`; an in-memory project is written to a temporary
directory for the length of the call.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const RETRY_DEFAULTS = { maxAttempts: 3 };`,
    'docs/retry.md': 'A call is retried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->',
  },
});
const report = await project.check();

assert.equal(report.results.length, 1);
assert.equal(report.results[0].status, 'pass');
assert.equal(report.results[0].page, 'docs/retry.md');
assert.equal(report.results[0].line, 1);
assert.equal(report.exitCode, 0);
```

## Five kinds of attestation

Every result carries one of these kinds: <!-- fact: KINDS -->
- `example`
- `fact`
- `cases`
- `snapshot`
- `test`

and one of these statuses: <!-- fact: STATUSES -->
- `pass`
- `fail`
- `error`
- `skipped`

Each kind has its own page: [examples](examples.md), [facts](facts.md), [cases](cases.md),
[snapshots](snapshots.md), [citations](citations.md). [Manifests](manifests.md) covers facts
over project files such as `package.json` or a `Dockerfile`; [commands and sample pages](console.md) covers
the examples docattest runs itself, whatever the language. [Pages](pages.md) covers what is
checked at all, [surfaces](surfaces.md) what examples and facts may import, [duality](duality.md) the
code-to-docs index, [report](report.md) the output and exit codes, [the command
line](cli.md) the commands, [adapters](adapters.md) the language boundary, and
[protocol](protocol.md) how the core talks to an adapter. [Audit](audit.md)
finds claims nobody has attested yet, [inventory](inventory.md) lists what a fact could
read, [changes](changes.md) judges what a change did to the
attestations, and [impact](impact.md) goes from a code diff to the prose it touches.
[Coding agents](agents.md) covers what an agent runs, and how one with hooks is described.
[Why](why.md) gives the reasoning behind the design.

## Why

Prose drifts because its expectations live somewhere else. Mechanisms that watch for
change cannot tell a refactor from a behaviour change and drown in noise. Execution can.
So the expectation lives in the page and the page runs. See [SPEC.md](design/SPEC.md) for
the long form.
