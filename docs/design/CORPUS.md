# Drift corpus

The shapes documentation drift takes, written down so that a candidate solution can be
judged against them: it must fail at each listed event and stay silent on ordinary
commits. The events are illustrative; each is reproduced as a test in this repository.
Add your own.

## Format

For each event: the kind of project, the page, what was wrong, and the attestation kind
that catches it.

## Seed events

| # | Project shape | Page | What was wrong | Caught by |
|---|---------------|------|----------------|-----------|
| 1 | small service, README | env var list | the server read variables the list did not name | `fact` over the config keys, set compare |
| 2 | same | endpoint table | routes missing from the table | `fact` over a route registry, `column=` compare |
| 3 | same | sentence about what the test suite covers | the suite had grown; the sentence described its first version | none: no observable. Rewrite as glue |
| 4 | public SDK, docs site | error-handling guide | a hand-copied error code enum missing one code; shown as `enum`, the source was a const object | `fact` text compare over a code block, or `example` asserting the key set |
| 5 | same | configuration guide | "supports two environments"; the enum had three | `fact` over `Object.values(Enum)`, set compare against the list |
| 6 | same | permissions guide | descriptions differed from a live endpoint's wording | `fact` over a `snapshot`, column compare |
| 7 | same | service guide | interfaces retyped by hand; one omitted the base type it extends | `example … compile` against the real types, or `fact` text compare |
| 8 | same | CI policy | type-checked samples ran only inside a job allowed to fail, and some were never checked | criterion 13: `example` blocks in a blocking job |

## What the corpus shows

- Events 1, 2, 4 and 5 are copied lists or values, the most common drift in
  code-adjacent prose. Facts with set compare catch all four.
- Events 1 and 2 can happen *while the page is being edited*: an author in the file does
  not notice that nearby sections are stale. A check that runs on every doc edit catches
  this; one that only watches code changes does not.
- Events 4 and 5 can sit on a public site for a long time, because the facts are plain
  text that no pipeline reads.
- Event 3 is a stale *behavioural* sentence, untestable as written. That supports the
  pattern's "glue" category and argues against spending effort on change-tracking
  behavioural prose.
- Change detection (flagging a page whenever nearby code or tests change) fires on
  refactors far more often than on real drift. It is excluded by criterion 6.

## Adding an event

1. Find the introducing change: `git log -S'<the string that changed>' -- <source file>`.
2. Find the page's last edit: `git log -1 -- <page>`.
3. Record the row. If no attestation kind would have caught it, say so; those rows are
   the most informative.
