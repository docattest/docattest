# Drift benchmark

Measures what docattest surfaces, before and after a change to the tool. Each scenario
builds a small project in a temporary directory, runs the built command line against it
(`adapters/typescript/dist/launcher/main.js`, the npm package's launcher, which runs the
docattest core) and judges the JSON it prints.

```sh
npm run build && npm run build:core
npm run bench                          # the core built in core/target/release
node scripts/with-core.ts node tests/bench/run.ts --label <name>
                                       # writes tests/bench/results/<name>.json (git ignores it)
npm run bench -- --record-baseline     # a change that detects more: record it in baseline.json
```

[`baseline.json`](baseline.json) is committed and names the development scenarios that
passed when it was recorded. A run exits 1 when one of them fails now, so a change cannot
lose a detection unnoticed, and it lists the scenarios that pass for the first time. CI
runs the bench on every push.

Two numbers per category:

- **detected**: positive scenarios (drift, or a way of hiding drift) the tool surfaced.
- **false alarms**: negative scenarios (ordinary states) the tool spoke up about. Shown as
  `n/a` until the category detects anything, because a missing command is silent about
  everything.

`scenarios.ts` is the development set: it joins, in order, one file per part in
[`scenarios/`](scenarios), one for each category and one for the ways documentation sites
write copied facts. The parts are written with what sits beside `scenarios.ts`, one module
each: the project a scenario builds on, the judges of a command's JSON report, the shape of
that report, and what a scenario is and how its verdict is reached (which the parity harness
and the mirror test share).
[`../parity/bench.ts`](../parity/bench.ts) runs
the same scenarios (and real projects) to compare the core's exact output with recordings
instead of judging it; see [tests/README.md](../README.md). A second, held-out set is kept
outside the repository and passed with `--extra <file>`; it was written before the features and is
not shown to whoever builds them, so a score on it is not a score on the answers.

## The contract the scenarios judge

Everything is deterministic, offline and free of model calls, like the rest of docattest.

### `docattest audit --json`

Finds literals in living pages that look copied from a surface and carry no directive.

```json
{ "suggestions": [ { "page": "README.md", "line": 5, "target": "list",
    "expression": "ENV_KEYS", "status": "drift",
    "missing": ["RATE"], "unexpected": [] } ] }
```

- `target`: `list`, `column`, `scalar`, `block`, or `inline` for an enumeration written in
  a sentence ("`HF`, `S3` or `URL`").
- `line`: the first line of the list, table or block; for a scalar, the literal's line; for
  an inline enumeration, the line its sentence ends on.
- `expression`: a fact expression over the surface that the literal matches or nearly
  matches (`ENV_KEYS`, `Object.keys(ErrorCodes)`, `ROUTES.map((r) => r.path)`).
- `status`: `match` when page and code agree, `drift` when they nearly agree.
- `missing` (in the code, not on the page) and `unexpected` (on the page, not in the
  code), in the vocabulary fact failures already use.
- A suggestion also carries `surface` and `directive`, the comment ready to paste (for a
  table it names the column). The scenarios do not judge them.
- A literal that belongs to a suggested list, column, block or inline enumeration is
  never also suggested on its own.
- Advisory: exit code 0 whatever it finds. A surface that cannot be loaded is not a
  finding but a broken project: a user error, exit code 1, naming the cause. Frozen pages and already-bound literals are
  never suggested. Precision matters more than recall: a suggestion that is wrong costs
  the reader's trust in every other one.

### `docattest check --base <ref> --json`

The usual report plus `changes`: what the working tree did to the attestations that
existed at `<ref>`.

```json
{ "changes": [ { "type": "orphaned-claim", "page": "docs/retry.md", "line": 3,
    "severity": "fail", "message": "…" } ] }
```

| type | severity | meaning |
|------|----------|---------|
| `orphaned-claim` | `fail` | an attested claim is still on the page and its attestation is gone; `line` is the claim's line now |
| `frozen-page` | `warning` | a page that carried attestations at the base is no longer living |
| `downgrade` | `warning` | `check` became `compile`, or an example lost every assertion; `line` is the block's line now |
| `weakened` | `warning` | a fact's expression changed while the literal it defends did not; `line` is the fact's line now |

A `fail` change sets exit code 1. Deleting a claim together with its proof, moving,
rewording, renaming a page and adding attestations are not changes. `changes` is always
present with `--base`, empty when there is nothing to say.

### `docattest impact [--base <ref>] --json`

From the difference between `<ref>` (default `HEAD`) and the working tree to the prose it
touches.

```json
{ "base": "HEAD", "symbols": ["retry"],
  "references": [ { "symbol": "retry", "page": "docs/retry.md", "line": 7,
      "kind": "example", "claim": "…", "proven": true } ] }
```

- `symbols`: surface exports whose declaration changed, or that reach a changed
  declaration through the code they use. Comments and formatting are not changes.
- `references`: everything `where` would list for those symbols (`proven: true`), plus
  pages that name a symbol in a code span without attesting it (`kind: "mention"`,
  `proven: false`).

### `<!-- test: <file> > "<title>" -->`

A citation binds the sentence before it to an existing test. `check` yields a result of
kind `test`: `pass` when the file holds a test with that title, `error` when the file or
the title is missing or the directive is malformed. The test is not run; the project's
own test job does that. A cited page counts as covered.
