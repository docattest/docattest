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
nothing when your change touches nothing a page documents); before you finish, run
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
| "Runs as user 10001" (another file says so)     | a fact over `file('Dockerfile')`, with the language's string functions |
| "Running `tool x` prints…" (a command's output) | a `console check` block: `$ tool x`, then its output |
| a sample page the docs show a user writing      | an `md check fixture=<dir>` block                 |
| a behaviour an existing test already proves     | `<!-- test: file > "title" -->` after the sentence, or alone on the line above it |
| "We chose this because…"                        | prose under `## Why`, no proof                    |

Directives and commands are the same in every language. What is written in the project's
language is the code: examples, fact expressions and the setup module. `"adapter"` in
`docattest.config.json` names the language.

| | `"adapter": "typescript"` | `"adapter": "python"` | `"adapter": "go"` | `"adapter": "rust"` |
|---|---|---|---|---|
| a block that runs | ```` ```ts check ```` | ```` ```python check ````, or ```` ```pycon check ```` for a `>>>` session | ```` ```go check ````: statements, declarations, or a whole `package main` | ```` ```rust check ````: the body of `main`, or a whole program with `fn main` |
| a block that is only checked, not run | ```` ```ts compile ```` (type-checked) | ```` ```python compile ```` (syntax, imports, names) | ```` ```go compile ```` (compiled) | ```` ```rust compile ```` (compiled) |
| an assertion | `assert.equal(a, b)`; `assert` is in scope | `assert a == b`; in a session, the output shown | a last comment `// Output: …` that what it prints must match, or `if a != b { panic(…) }` | `assert_eq!(a, b)`, `assert!(…)`, or an `unwrap()` |
| a fact expression | `RETRY.max`, `errors.map((e) => e.field)`: the surface's exports by their own names, and the constants of its module, exported or not | `RETRY["max"]`, `[e.field for e in errors]`: the surface's exports, and the constants of its module, exported or not | `Retry.Max`, `len(Regions)`: the surface's exports by their own names, the constants of its package and its literal variables (a `var` given a composite literal of constants that the package only reads), exported or not, and what its `//go:build docattest` files export | `RETRY.max`, `REGIONS.len()`: the exports of the surface's module by their own names, and its constants, exported or not |
| how a value is written on the page | as JavaScript prints it: `true`, `3` | as Python prints it: `True`, `3` | as `fmt.Sprint` prints it: `true`, `3`, `[a b]` | as `Display` prints it, else `Debug`: `true`, `3`, `Some(3)` |
| setup above the reader's view | lines above `// ---cut---` | none: put it in an earlier block and tag the next `continue` | none: a block is a whole program | lines that begin `# `, as in a doc test |
| examples in source files | `@example` in a doc comment | `>>>` prompts in a docstring | `Example…` functions in a `_test.go` file | fenced blocks in `///` doc comments |
| a test to cite | `it("title")`, inside `describe` blocks, or a row of `it.each` by the title its runner prints | a `test_…` function, inside classes, or a `parametrize` case or a `subTest` by the id its runner prints (`"test_total" > "big discount"`) | a `Test…` function, or a `t.Run("title", …)` inside one, a row of a literal table included | a `#[test]` function inside its `mod tests`, or a named case of an rstest or test-case test (`"tests" > "discount" > "quarter"`) |
| a cell's code spans, in a cases runner | `codeSpans(cell)`, from `docattest` | `code_spans(cell)`, from `docattest.cells` | `cells.CodeSpans(cell)`, from `github.com/docattest/docattest/adapters/go/cells` | `docattest_harness::cells::code_spans(cell)` |
| the setup module | `export const cases`, `export const facts` | `cases = {…}`, `facts = {…}` | a package with `var Cases = map[string]func(map[string]string) error{…}`; facts use its exports, and a fact that calls one attests what it uses | a file with `pub const CASES: &[(&str, fn(&HashMap<String, String>) -> Result<(), String>)]`; facts use its `pub` items |

Syntax:

- Example: a fenced block in the project's language tagged `check` (it runs) or `compile`
  (it is checked without running). Import what a user would import. One behaviour per
  block, at least one assertion. Setup the reader need not see must never perform the
  behaviour the section describes.
- Fact: `<!-- fact[ @surface][ ordered][ column=<name> | field=<path>]: <expression> -->`,
  the modifiers in any order before the colon. `@surface` reads a configured surface other
  than `default`; `ordered` compares a list in order, and is an error on a single value or
  a fenced block; `column=` and `field=` never go together.
- Scalar fact: put the comment right after the literal: ``At most `3` tries. <!-- fact: RETRY.max -->``
  It binds the code span or bare number before it on the same line. With none there, it
  binds the code spans of the sentence it ends, which may wrap over lines: put the comment
  at the end of the sentence's last line. Or put it alone on the line directly above the
  sentence, as `// @ts-expect-error` sits: it binds as it would at the sentence's end. A
  comment alone on a line with a blank line before the prose below it fails.
- Citation: `<!-- test: file > "title" -->` at the end of the sentence a test proves, or
  alone on the line directly above it; several may share that line. Next to a fact, before
  or after it, a citation never changes what the fact binds. `where` shows the sentence as
  the claim.
- List fact: end the line before the list with the comment: `The server reads: <!-- fact: ENV_KEYS -->`.
  Or end a sentence of code spans with it, ``Keys are `A` and `B`. <!-- fact: KEYS -->``:
  every code span of the sentence is compared as a set, so a sentence that also names a
  file or a function in code fails with `unexpected:`. Give the list a sentence of its own.
- Table column: `<!-- fact column=Field: <expression giving the column> -->` before the table.
- Fenced block: the comment ends the line before the block; whitespace is ignored.
- One value of a `json`, `toml` or `yaml` block: `<!-- fact field=ports.0.port: <expression> -->`
  before the block. Keys are parted by dots, a list item is its index, and a key with a dot
  or space is quoted (`limits."max retries"`). Several such facts may sit above one block.
- Named facts: when an expression is awkward inline, add a helper to the setup module's
  `facts` and call it by bare name. The helper reads the surface; the expectation stays on
  the page. A fact that calls a helper attests what the helper uses, as a cases runner does.
- Docs helpers: never export something only for the docs. To state a private value that a
  helper must work out, write the helper inside the surface where only docattest builds it
  (Go: an exported name in a `//go:build docattest` file of the surface's package; Rust: a
  `pub` item of the surface's module under `#[cfg(docattest)]`) and call it from the fact by
  its bare name. A page's examples cannot call one.
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
   with a proof. When it is plumbing no reader calls, add a glob that names it to the
   config's `undocumented` for its surface, which `uncovered` then leaves out, and say why
   in the PR.
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
