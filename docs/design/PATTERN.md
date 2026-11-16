# The attested page — authoring pattern

For writers, and for agents writing or editing documentation or the code it describes.
Load this before touching a page or an exported symbol in a repository that uses
`docattest`. The machinery is in [SPEC.md](SPEC.md); this is the convention it runs.
`npx docattest skill install` puts a condensed version where coding agents look for skills.

## The one rule

**Every sentence that states behaviour is followed by its proof, in the same section,
within a few lines.** The proof is an example that runs, a table that runs, or a fact
that is checked. A sentence without proof is allowed, but the build cannot defend it,
and the page's coverage says so.

Corollary: never write a number, a name, a list or a shape that comes from the code as
plain text. Write it as a fact.

## Page shape

```md
---
title: Retrying requests
---

# Retrying requests                      ← what the page is about, one line

One paragraph of orientation: why this exists, who uses it. No claims yet.

## Failed calls are retried              ← one behaviour per section

Claim sentence(s): plain, present tense, one idea each.

```ts check                              ← the proof, within a few lines of the claim
import { retry } from '@example/client';
…assertions…
```

Notes a reader needs after the example. Still no unproven literals.

## Defaults

A call is retried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->

## Validation rules                      ← a set of rules becomes a table

<!-- cases: validateConfig -->
| case | config | result |
| … |

## Why                                   ← rationale lives here, and only here

Design reasons, history, trade-offs. Unchecked by design. Keep it general enough to
stay true.
```

## Choosing the proof

| The sentence says…                                   | Write…                              |
|------------------------------------------------------|-------------------------------------|
| "Calling X with Y does Z"                            | an `example` asserting Z            |
| "X is 3", "defaults to `eu`", "exports A, B, C"      | a `fact` bound to the literal/list  |
| "Rule 1… Rule 2… Rule 3…" over one operation         | a `cases` table, one row per rule   |
| "The service accepts these regions/kinds/scopes"     | a `fact` over a `snapshot`          |
| "Running `tool x` prints…"                           | a `console check` block             |
| a sample page the docs show a user writing           | an `md check fixture=<dir>` block   |
| a behaviour an existing test already proves          | a `test` citation of that test      |
| "We chose this because…"                            | prose under `## Why`, no proof      |

## Writing examples

- The example demonstrates the sentence above it and asserts the outcome the sentence
  promises. Without an assertion it is an illustration, not a proof.
- Setup goes above `// ---cut---`. Setup creates inputs; it never performs the behaviour
  under test. If the hidden part does the interesting work, the reader is shown a lie
  and the proof proves something else.
- One behaviour per example. Two claims in one block means a failure cannot say which
  sentence is false.
- Import what a user would import. If the example needs an internal, the sentence is
  about an internal and does not belong on this page.
- If the docs site type-checks samples itself (twoslash and similar), `assert` is unknown
  to it. Put `import assert from 'node:assert/strict';` above `// ---cut---`: the site
  hides it, the site's checker accepts it, and docattest runs it like any setup line.
- No network, no clock, no randomness. If the behaviour needs the network, tag the block
  `check network` and accept that it is skipped offline.

## Writing facts

- Bind to the literal the reader sees: `` `3` ``, `` `eu` ``, a bulleted list of names, a
  code block of a type. For a scalar, the comment goes right after the literal. For a
  list, table or block, the comment ends the line just before it; a lead-in sentence on
  that line is fine ("The server reads: `<!-- fact: ENV_KEYS -->`"). A short list may stay
  in the sentence: after "Regions are `eu`, `us` and `ap`." the comment binds all three.
- The expression reads from the surface, not from source files: `RETRY_DEFAULTS.maxAttempts`,
  `Object.keys(ErrorCodes)`, `snapshot('regions').map(r => r.code)`.
- When the expression would be long or repeated (mapping a structure into the list the
  page shows, say), give it a name: export `facts = { errorFields: () => … }` from the
  setup module and write `<!-- fact column=Field: errorFields() -->`. A named fact is glue
  like a cases runner: it reads the surface and shapes a value, and never holds the
  expectation. Do not reuse a name the surface exports; that is an error.
- When a fact fails, fix the page or fix the code. Never weaken the expression to make
  it pass; a fact that asserts less than the prose says is worse than none.

## Writing cases tables

- The first column names the case in words a reader understands. The remaining columns
  are what the runner needs; their names are a contract with the runner.
- Rows are the expectations. Everything in a row is visible to the reader; the runner
  knows how to execute, never what to expect.
- Prefer a table to a paragraph of "if… then… unless…". It is shorter, it scans, and
  every row runs.
- The directive goes at the end of the line before the table; a blank line between them is
  fine, a sentence is not.

## Citing a test

When the behaviour needs real setup and a test already covers it, cite the test instead of
rewriting it on the page: `<!-- test: test/retry.test.ts > "throws the last error" -->`
after the sentence. A citation only proves the test exists. Prefer an example when the
reader would learn from seeing the call, and a `console check` block when the behaviour is
what a command prints.

## External facts

A statement about a system outside the repo (another service's enum values, a
platform's event names, a provider's URL shapes) is an assumption, not a claim. Pin it:
add a snapshot with a `source`, reference it from a fact, and let `docattest refresh` be
the only way it changes. Say in the prose that the list is a snapshot and when it was
taken, so the reader knows what kind of truth they are reading.

## Anti-patterns

- **Tests in English.** "Returns 5 when called with 3 after the cache is warm." That is
  a test name. Write the user-facing sentence; let the example carry the specifics.
- **The orphan list.** Env vars, routes, error codes or options listed with no fact. The
  single most common drift found in real repositories.
- **The decorative example.** Compiles, asserts nothing, sits next to a sentence that
  promises a lot.
- **The heroic hidden setup.** Everything interesting above the cut, one trivial line
  below.
- **Rationale with numbers.** "We chose 3 retries because…" The 3 belongs in a fact in
  the behaviour section; the Why section says "a small fixed number".
- **Proof far from claim.** The example is in a later section or another page. When the
  claim moves, the proof stops defending it.

## For agents editing code, not docs

The link runs both ways. `docattest impact` lists the prose your uncommitted diff touches,
including pages that only mention a symbol; run it after any source edit
(`docattest hooks install` makes that automatic). Before changing an exported symbol:

1. `docattest where <symbol>` lists every page, line and claim sentence that describes it.
   Load those pages. They are the prose your change will affect, and they will not be in
   your context otherwise.
2. Make the change.
3. `docattest check <page>` for each page `where` listed, then `docattest check --json`.
   Every failure is a sentence that is now false, with expected versus actual. Fix the
   prose where the behaviour changed on purpose; fix the code where it did not.
4. If you added a public symbol, `docattest uncovered --surface` will list it. Either
   document it with a proof or state in the PR why it is undocumented.

Never resolve a failure by weakening an assertion, deleting an attestation, or freezing
a page. Those are the three ways to make the build green while making the docs wrong, and
`docattest check --base <ref>` reports all three.

## Checklist before a page is done (agents: run this literally)

1. `docattest check <page> --json` reports no `fail` and no `error`.
2. Every section after the orientation paragraph has at least one attestation, or is
   `## Why`.
3. No bare number, name, list or type from the code appears without a fact.
4. Every `check` example contains at least one assertion.
5. Nothing above a `// ---cut---` performs the behaviour the section describes.
6. External facts reference a snapshot, and the prose says so.
7. The page's coverage is not worse than before the edit.
