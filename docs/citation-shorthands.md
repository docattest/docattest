---
title: Citation shorthands: sets and aliases
---

# Citation shorthands: sets and aliases

A [citation](citations.md) names a file and the titles of a test. Citing four tests of one
place, or many tests of one file, would write the same file and titles again and again;
these two shorthands write them once. Each citation they make is judged exactly as one
written out in full.

## Several tests in one place are cited as a set

The last segment of a citation may be a set of titles: titles in braces, parted by commas.
It cites each of them as if it were written on its own, with the same file and the same
titles before it, so each gives its own result, and a missing one fails with a message that
names it.

```md
Python, Node.js and Ruby scripts all run. <!-- test: agent/launch_test.go > "TestRun" > {"python", "node", "ruby"} -->
```

A set can only be the last segment. An empty set is an error, and so is a title written
twice in one set.

## A page names a test file once with an alias

`<!-- tests: <name> = <file> -->` gives a test file a name on its page, and a citation on
that page may write the name where the file goes. An alias holds on the whole page,
wherever it is written, so it may sit at the top or beside the citations that use it. A
name is declared once on a page; declaring it again is an error there, and the first holds.

```md
<!-- tests: launch = agent/launch_test.go -->

Python scripts run. <!-- test: launch > "TestRun" > "python" -->
Node.js scripts run the same way. <!-- test: launch > "TestRun" > "node" -->
```

A name is letters, digits, `_` and `-`, starting with a letter or `_`, so it never looks
like a path, and a name the page does not declare is read as the path of a file. A name that
is also a file of the project, such as a `Makefile`, is an error at each citation that uses
it: rename the alias. The file is written unquoted, as in a citation, and an alias with no
closing `-->` is an error at its line, as any directive left open is. A comment that starts with `tests:` and has no `=` in it is an ordinary
comment.

Each row below is a page beside `test/launch.test.ts`, which declares the tests `python`,
`node` and `ruby` inside `describe('run')`, and a `Makefile`; the results are the page's,
in order.

<!-- cases: citations -->
| case | page | results |
|---|---|---|
| a set of titles | It runs scripts. <!-- test: test/launch.test.ts > "run" > {"python", "node", "ruby"} --> | pass; pass; pass |
| a set with a missing title | It runs scripts. <!-- test: test/launch.test.ts > "run" > {"python", "perl"} --> | pass; error: declares no test "run" > "perl" |
| an empty set | It runs scripts. <!-- test: test/launch.test.ts > "run" > {} --> | error: has an empty set of titles |
| a set that is not last | It runs scripts. <!-- test: test/launch.test.ts > {"run"} > "python" --> | error: has a set of titles that is not the last segment |
| a title twice in a set | It runs scripts. <!-- test: test/launch.test.ts > "run" > {"node", "node"} --> | error: names "node" twice in its set of titles |
| an alias | <!-- tests: launch = test/launch.test.ts -->¶¶It runs Python. <!-- test: launch > "run" > "python" --> | pass |
| an alias below its citation | It runs Python. <!-- test: launch > "run" > "python" -->¶¶<!-- tests: launch = test/launch.test.ts --> | pass |
| an alias and a set | <!-- tests: launch = test/launch.test.ts -->¶¶It runs scripts. <!-- test: launch > "run" > {"python", "node"} --> | pass; pass |
| an alias declared twice | <!-- tests: launch = test/launch.test.ts -->¶<!-- tests: launch = test/other.test.ts -->¶¶It runs Python. <!-- test: launch > "run" > "python" --> | error: tests at L2 declares launch again: it is declared at L1; pass |
| an alias that looks like a path | <!-- tests: test/launch = test/launch.test.ts --> | error: which is not a name of letters, digits, _ and - |
| an alias with its file quoted | <!-- tests: launch = "test/launch.test.ts" --> | error: write the file unquoted |
| an alias left unclosed | It runs.¶¶<!-- tests: launch = test/launch.test.ts | error: is not closed |
| a name the page does not declare | It runs Python. <!-- test: launch > "run" > "python" --> | error: cannot cite "run" > "python": launch |
| an alias that is also a file | <!-- tests: Makefile = test/launch.test.ts -->¶¶It runs Python. <!-- test: Makefile > "run" > "python" --> | error: is both an alias on its page and a file of the project |
| a `tests:` comment with no `=` | It runs Python. <!-- tests: none yet --> | none |
