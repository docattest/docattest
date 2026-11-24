---
name: docattest
description: Use when writing or editing documentation, or when changing, renaming or removing an exported symbol, in a repository that has a docattest.config.json.
---

# docattest: attested documentation

In this repository the docs are tests. State behaviour and prove it in the same section;
`docattest check` runs every proof and fails at the page and line that stopped being true.
Run `docattest` the way `AGENTS.md` specifies (from any directory in the project).

Two commands matter whatever else you do:
- After editing source code: `docattest impact --file <path>` (read every line it prints).
- Before finishing: `docattest check --quiet` (or `docattest check <page>` / `--member <dir>` while iterating).

## Choose the proof

| Claim | Proof |
|---|---|
| "Calling X with Y does Z" | Fenced block tagged `check` with an assertion or `Output:` |
| "X is 3", "defaults to `eu`", list/table/field from code | `<!-- fact: expr -->` (modifiers: `@surface`, `ordered`, `column=<name>`, `field=<path>`) |
| TypeScript interface keys or literal union values | `<!-- fact: typeKeys('Options') -->` or `<!-- fact: typeValues('LogLevel') -->` |
| "Rule 1… Rule 2…" over one operation | `<!-- cases: runner -->` above a Markdown table |
| Manifest or config file value | `manifest('file')` or `manifestField('file', 'a.b')` |
| Arbitrary project file value | `matched('Dockerfile', '^USER ([0-9]+)$')` or `file('path')` |
| External pinned data | `snapshot('name')` |
| Command output | Fenced `console check` block (`$ cmd`, expected output, optional `[exit N]`) |
| Existing test proves behaviour | `<!-- test: file > "title" -->` (sets: `> {"a", "b"}`; page alias: `<!-- tests: a = path -->`) |
| Sample doc page | Fenced `md check fixture=<dir>` block |
| Pure orientation / no claims | `docattest: prose` in frontmatter or `prose` in `docattest.config.json` |

## Directives & workflow

- Place scalar facts right after the value (`` `3` <!-- fact: N --> ``), at sentence end, or alone on the line directly above it.
- Place list/column/block/field facts on the line immediately before the list, table or fenced block.
- Inspect bindings without guessing: `docattest explain <page>[:<line>]`.
- Find missing facts, broken untagged examples, or drift: `docattest audit` (auto-insert matching directives and `compile` tags with `docattest audit --apply`).
- Find docs for a symbol: `docattest where <symbol>`; find unattested exports: `docattest uncovered`.
- Prefer `<!-- test: ... -->` citations for heavy integration scenarios instead of putting setup scaffolding inside a `check` block; reserve `check` blocks for short examples readers benefit from seeing. Run `docattest check --run-cited` in CI so citations run their tests.
- Never export internals just for docs — mark internal packages `"internal": true` in `surfaces` or use a docs helper.
- Never make CI green by deleting a proof, weakening an assertion, or freezing a living page (`check --base` catches all three).

## Adopting on an existing repo

- Check for Markdown docs outside `init`'s default globs (nested package guides, skills, API inventories) and mark narrative or external-tool pages under `prose` in `docattest.config.json`.
- Run `docattest audit` first to catch broken untagged code examples and matching facts/types, then convert high-value SDK/API examples to `check` or `compile`.
- Attest documented package/dependency/engine versions with `manifestField(...)` and cite existing unit/integration tests with `<!-- test: ... -->`.
