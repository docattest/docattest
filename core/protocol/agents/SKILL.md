---
name: docattest
description: Use when writing or editing documentation, or when changing, renaming or removing an exported symbol, in a repository that has a docattest.config.json. docattest makes pages carry their own proof (examples that run, facts checked against the code, tables whose rows run) and maps every symbol back to the pages that describe it.
---

# docattest: attested documentation

In this repository the docs are tests. A page states behaviour and proves it in the same
section; `docattest check` runs the proofs and fails at the page and line where the
prose stopped being true. Follow this skill whenever you touch a page or an exported symbol.

Run `docattest` the way this project runs its tools: `npx docattest` where it was installed
from npm, `docattest` from the project's virtualenv where it was installed with pip; when
the project's `AGENTS.md` has a docattest section, it gives the exact command. The commands
below are written bare, and work from any directory inside the project.

Two commands matter whatever else you do: after editing source code, run
`docattest impact --file <the file you edited>` and read every line it prints (it prints
nothing when your change touches no documented export); before you finish, run
`docattest check`.

## The one rule

Follow every sentence that states behaviour with its proof, in the same section, within a
few lines. Never write a number, name, list or shape that comes from the code as plain
text: write it as a fact.

## Choose the proof

| The sentence says…                              | Write…                                            |
|-------------------------------------------------|---------------------------------------------------|
| "Calling X with Y does Z"                       | a fenced block tagged `check` that asserts Z      |
| "X is 3", "defaults to `eu`", "exports A, B, C" | a fact bound to the literal or list               |
| "Rule 1… Rule 2… Rule 3…" over one operation    | a `<!-- cases: runner -->` table, one row per rule|
| "The service accepts these regions"             | a fact over `snapshot('name')`                    |
| "Needs Node 22.12 or later" (a manifest says so) | a fact over `manifest('package.json')`           |
| "Running `tool x` prints…" (a command's output) | a `console check` block: `$ tool x`, then its output |
| a sample page the docs show a user writing      | an `md check fixture=<dir>` block                 |
| a behaviour an existing test already proves     | `<!-- test: file > "title" -->` after the sentence |
| "We chose this because…"                        | prose under `## Why`, no proof                    |

Directives and commands are the same in every language. What is written in the project's
language is the code: examples, fact expressions and the setup module. `"adapter"` in
`docattest.config.json` names the language.

| | `"adapter": "typescript"` | `"adapter": "python"` | `"adapter": "go"` | `"adapter": "rust"` |
|---|---|---|---|---|
| a block that runs | ```` ```ts check ```` | ```` ```python check ````, or ```` ```pycon check ```` for a `>>>` session | ```` ```go check ````: statements, declarations, or a whole `package main` | ```` ```rust check ````: the body of `main`, or a whole program with `fn main` |
| a block that is only checked, not run | ```` ```ts compile ```` (type-checked) | ```` ```python compile ```` (syntax, imports, names) | ```` ```go compile ```` (compiled) | ```` ```rust compile ```` (compiled) |
| an assertion | `assert.equal(a, b)`; `assert` is in scope | `assert a == b`; in a session, the output shown | a last comment `// Output: …` that what it prints must match, or `if a != b { panic(…) }` | `assert_eq!(a, b)`, `assert!(…)`, or an `unwrap()` |
| a fact expression | `RETRY.max`, `errors.map((e) => e.field)` | `RETRY["max"]`, `[e.field for e in errors]` | `Retry.Max`, `len(Regions)`: the surface's exports by their own names | `RETRY.max`, `REGIONS.len()`: the exports of the surface's module by their own names |
| how a value is written on the page | as JavaScript prints it: `true`, `3` | as Python prints it: `True`, `3` | as `fmt.Sprint` prints it: `true`, `3`, `[a b]` | as `Display` prints it, else `Debug`: `true`, `3`, `Some(3)` |
| setup above the reader's view | lines above `// ---cut---` | none: put it in an earlier block and tag the next `continue` | none: a block is a whole program | lines that begin `# `, as in a doc test |
| examples in source files | `@example` in a doc comment | `>>>` prompts in a docstring | `Example…` functions in a `_test.go` file | fenced blocks in `///` doc comments |
| a test to cite | `it("title")`, inside `describe` blocks | a `test_…` function, inside classes | a `Test…` function, or a `t.Run("title", …)` inside one | a `#[test]` function, inside its `mod tests` |
| the setup module | `export const cases`, `export const facts` | `cases = {…}`, `facts = {…}` | a package with `var Cases = map[string]func(map[string]string) error{…}`; facts use its exports | a file with `pub const CASES: &[(&str, fn(&HashMap<String, String>) -> Result<(), String>)]`; facts use its `pub` items |

Syntax:

- Example: a fenced block in the project's language tagged `check` (it runs) or `compile`
  (it is checked without running). Import what a user would import. One behaviour per
  block, at least one assertion. Setup the reader need not see must never perform the
  behaviour the section describes.
- Scalar fact: put the comment right after the literal: ``At most `3` tries. <!-- fact: RETRY.max -->``
  It binds the nearest code span or bare number before it on its own source line, not in
  its sentence. A comment alone on a line, or pushed onto the next line when the paragraph
  is wrapped, fails with "has no target": keep the literal and the comment on one line.
- List fact: end the line before the list with the comment: `The server reads: <!-- fact: ENV_KEYS -->`
- Table column: `<!-- fact column=Field: <expression giving the column> -->` before the table.
- Fenced block: the comment ends the line before the block; whitespace is ignored.
- Named facts: when an expression is awkward inline, add a helper to the setup module's
  `facts` and call it by bare name. The helper reads the surface; the expectation stays on
  the page.
- Commands: a block tagged `console check` holds lines `$ command`, each followed by what it
  prints (stdout, then stderr). `...` on a line matches any lines; a last line `[exit N]`
  says the command fails with `N`. Commands run with `sh -c` in the project root, or in
  `cwd=<dir>`; `docattest` in one is the docattest checking the page.
- Sample pages: a block tagged `md check fixture=<dir>` is checked as a page (at
  `docs/sample.md`, or `as=<path>`) in a copy of the project at `<dir>`; every proof on it
  must pass.
- Cases: `<!-- cases: runnerName -->` before a table. The first column names the case; the
  other columns are what the runner needs. Rows hold the expectations, never the runner.

## Workflow when you change code

1. Before changing an exported symbol, find the prose it affects and read those pages:

   ```sh
   docattest where <symbol>
   ```

2. Make the change.
3. Check each page `where` listed, then everything:

   ```sh
   docattest check docs/the-page.md
   docattest check --json
   ```

4. Every failure is a sentence that is now false, with expected versus actual. Fix the
   prose where the behaviour changed on purpose; fix the code where it did not.
5. If you added a public symbol, `docattest uncovered --surface` lists it. Document it
   with a proof, or say in the PR why it stays undocumented.
6. After any source edit, including to internals, `docattest impact --file <file>` lists
   the prose your diff touches (`docattest impact` does so without asking about one file).
   Entries marked as mentions are unproven: read them, they go stale first.

## Workflow when you write or edit a page

1. Write the claim sentence, then its proof right after it.
2. Run `docattest check <page>` and read every `fail` and `error`.
3. Fix the prose or the proof until the page passes, then run `docattest check`.
4. `docattest audit` reports lists, tables, blocks and values in sentences on the pages that
   look copied from the code and carry no fact. Add the suggested directive; where it reports drift, fix the page.

## Never make the build green this way

These three make the build pass while making the docs wrong:

- weakening an assertion or a fact expression so it asserts less than the prose says;
- deleting an attestation;
- freezing a page (config `frozen` or frontmatter `docattest: false`).

CI may run `docattest check --base <ref>`, which fails when a claim is kept and its proof
removed, and reports the other two.

## Checklist before a page is done (run it literally)

1. `docattest check <page> --json` reports no `fail` and no `error`.
2. Every section after the orientation paragraph has at least one attestation, or is `## Why`.
3. No bare number, name, list or type from the code appears without a fact.
4. Every `check` example contains at least one assertion.
5. No hidden setup (above a `// ---cut---`, or in an earlier block of a session) performs
   the behaviour the section describes.
6. External facts reference a snapshot, and the prose says it is a snapshot and when it was taken.
7. The page's coverage is not worse than before the edit.
