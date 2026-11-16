---
title: Commands and sample pages
---

# Commands and sample pages

Two kinds of block are run by docattest itself, whatever the project's language: a
`console check` block, a transcript of commands and what they print, and an `md check`
block, a sample page that is checked inside a copy of a fixture project. They prove what a
command-line tool does and what a page written for it holds, where an `example` in the
project's language cannot and a citation would only name a test. Both are reported as
`example`; no adapter is asked about them. A program's code is not an export, so
[`impact`](impact.md) reaches these blocks through the config's `commands`, which says what
each program is built from.

All examples on this page build a small in-memory project around one documentation page
and look at the result for that page.

## A console block is commands and what they print

In a block tagged `console check`, a line that starts with `$ ` is a command, and the lines
after it, up to the next command, are what it prints: everything it writes to stdout, then
everything it writes to stderr. The block passes when every command prints what the block
shows and exits with 0. When one does not, the block fails at that command, with its line
in the block and the first line of output that differs, and the commands after it are not
run.

```ts check
import { createProject } from 'docattest';

const page = (...block: string[]) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/cli.md': ['# The tool', '', '```console check', ...block, '```', ''].join('\n'),
  },
});

const good = await createProject(page('$ echo one; echo two >&2', 'one', 'two', '$ printf "a\\nb\\n"', 'a', 'b')).check();
assert.equal(good.results[0].status, 'pass');
assert.equal(good.results[0].kind, 'example');
assert.equal(good.results[0].line, 3);

const bad = await createProject(page('$ echo one', 'one', '$ printf "a\\nc\\n"', 'a', 'b', '$ echo never')).check();
assert.equal(bad.results[0].status, 'fail');
assert.equal(bad.results[0].message, '`printf "a\\nc\\n"` (block line 3): output line 2 differs: expected "b", got "c"');
assert.equal(bad.exitCode, 1);

const short = await createProject(page('$ echo one', 'one', 'two')).check();
assert.equal(short.results[0].message, '`echo one` (block line 1): the output ended before line 2, "two"');
```

A block whose first line is not a command, or that has no command, is an `error`. An
untagged `console` block is documentation, as any untagged block is.

```ts check
import { createProject } from 'docattest';

const page = (info: string, ...block: string[]) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/cli.md': ['```' + info, ...block, '```', ''].join('\n'),
  },
});

const noCommand = await createProject(page('console check', 'one', '$ echo one', 'one')).check();
assert.equal(noCommand.results[0].status, 'error');
assert.equal(noCommand.results[0].message, 'console block at L1: block line 1 is not a command; a command starts with "$ "');

const untagged = await createProject(page('console', '$ false')).check();
assert.deepEqual(untagged.results, []);
```

## A command that fails says so on its last line

A command that exits with anything but 0 fails the block, unless the last line of its
output in the block is `[exit N]`; it must then exit with exactly `N`.

```ts check
import { createProject } from 'docattest';

const page = (...block: string[]) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/cli.md': ['```console check', ...block, '```', ''].join('\n'),
  },
});

const unsaid = await createProject(page('$ echo no >&2; exit 2', 'no')).check();
assert.equal(unsaid.results[0].status, 'fail');
assert.equal(unsaid.results[0].message, '`echo no >&2; exit 2` (block line 1): exited with 2; end its output in the block with [exit 2] if it should');

const said = await createProject(page('$ echo no >&2; exit 2', 'no', '[exit 2]')).check();
assert.equal(said.results[0].status, 'pass');

const other = await createProject(page('$ exit 3', '[exit 2]')).check();
assert.equal(other.results[0].message, '`exit 3` (block line 1): exited with 3, not 2');
```

## Trailing whitespace is ignored, and `...` stands for any lines

Trailing whitespace on a line is ignored, in the block and in the output, and so are blank
lines at the end of either, which leaves room for a blank line between two commands. A line
that is exactly `...` matches any number of lines of output, none included. Nothing else is
a pattern: every other line is compared as written.

```ts check
import { createProject } from 'docattest';

const page = (...block: string[]) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/cli.md': ['```console check', ...block, '```', ''].join('\n'),
  },
});

const spaced = await createProject(page('$ printf "a  \\n\\n\\n"', 'a', '', '$ echo b', 'b   ')).check();
assert.equal(spaced.results[0].status, 'pass');

const elided = await createProject(page('$ seq 1 9', '1', '...', '9', '$ seq 1 2', '1', '...', '2')).check();
assert.equal(elided.results[0].status, 'pass');

const literal = await createProject(page('$ echo "a*"', 'a.')).check();
assert.equal(literal.results[0].status, 'fail');
```

## Where commands run, and for how long

Each command is run by `sh -c` in the project root, or in the directory a `cwd=<dir>`
attribute names, relative to the root. Each command has a shell of its own, so a `cd` or a
variable does not carry over to the next one; files written do. stdin is empty, and the
environment is docattest's own. A command may run for `runtime.timeoutMs` (by default
`10000` ms <!-- fact: DEFAULT_TIMEOUT_MS -->); then it is stopped with everything it
started, and the block fails.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript', runtime: { timeoutMs: 300 } }),
    'src/index.ts': `export const x = 1;`,
    'tool/hello.txt': 'hello',
    'docs/cli.md': [
      '```console check cwd=tool',
      '$ cat hello.txt; echo',
      'hello',
      '$ cd ..; X=1',
      '$ ls; echo "[$X]"',
      'hello.txt',
      '[]',
      '```',
      '',
      '```console check',
      '$ sleep 5',
      '```',
    ].join('\n'),
  },
}).check();

assert.equal(report.results[0].status, 'pass');
assert.equal(report.results[1].status, 'fail');
assert.equal(report.results[1].message, '`sleep 5` (block line 1): timed out after 300 ms');
```

Commands run through `sh`, which docattest does not use on Windows: there a console block
is `skipped`, which never changes the exit code.

## `docattest` in a command is the docattest that checks the page

In a command, `docattest` runs the same docattest that is checking the page: the same
core, with the same adapters, whatever command started it and whatever is on the `PATH`.
It is a shell function of the command's shell, so it is what a command calls by that name,
and not what a program the command starts finds on the `PATH` (`npx docattest` is npm's to
find). A block that checks the very page it is on would run itself for ever, so the second time
it is an `error`.

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'shop/docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'shop/src/index.ts': `export const N = 3;`,
    'shop/docs/n.md': '# N\n\nN is `4`. <!-- fact: N -->\n',
    'docs/cli.md': [
      'A page that disagrees with the code fails the check.',
      '',
      '```console check cwd=shop',
      '$ docattest check --quiet',
      'docs/n.md',
      '  ✗ L3 fact: expected 4, got 3 from N',
      'FAILED',
      '[exit 1]',
      '```',
    ].join('\n'),
  },
}).check();

assert.equal(report.results[0].status, 'pass');

const itself = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/cli.md': [
      '```console check',
      '$ docattest check --quiet docs/cli.md',
      'docs/cli.md',
      '  ! L1 example: this block is already running: a command in it checks this page again',
      '...',
      '[exit 1]',
      '```',
    ].join('\n'),
  },
}).check();
assert.equal(itself.results[0].status, 'pass');
```

## Commands run as a test suite runs

A console block runs whatever it says, with the network and the files the machine has: it
is not isolated, and docattest cannot cut a command off from the network as it does an
example. Treat the pages of a project with console blocks as you treat its tests, and keep
what a command changes inside a directory of its own.

## A sample page is checked inside a copy of a fixture

A block tagged `md check fixture=<dir>` (or `markdown`) is a page as a user would write it,
shown to the reader. docattest copies the project at `<dir>` (relative to the root) to a
temporary directory, writes the block there as `docs/sample.md`, or at the path an
`as=<path>` attribute names, and checks that page with the fixture's own config and
adapter. That adapter starts as the checked project, or one of its `members`, started the
adapter of the fixture's language (a member's `adapterCommand` included), so a page in one
language can show and check a sample in another; a command's `docattest` finds the same
adapters. The block passes when every proof on the sample page passes; when one does not,
the failure gives the line of the block's page it is on. A sample with no proof on it
fails, as does one that the fixture's `pages` do not take in.

```ts check
import { createProject } from 'docattest';

const sample = (...lines: string[]) => ({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'fixture/docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md', 'README.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'fixture/src/index.ts': `export const N = 3;`,
    'docs/guide.md': ['# Guide', '', 'A page:', '', ...lines, ''].join('\n'),
  },
});

const holds = await createProject(sample('````md check fixture=fixture', 'N is `3`. <!-- fact: N -->', '````')).check();
assert.equal(holds.results[0].status, 'pass');
assert.equal(holds.results[0].kind, 'example');
assert.equal(holds.results[0].line, 5);

const drifted = await createProject(sample('````markdown check fixture=fixture as=README.md', '# N', '', 'N is `4`. <!-- fact: N -->', '````')).check();
assert.equal(drifted.results[0].status, 'fail');
assert.equal(drifted.results[0].message, 'the sample page fails in a copy of fixture: L8 fact: expected 4, got 3 from N');

const unproven = await createProject(sample('````md check fixture=fixture', 'N is 3.', '````')).check();
assert.equal(unproven.results[0].message, 'the sample page has no proof on it');

const missing = await createProject(sample('````md check', 'N is `3`. <!-- fact: N -->', '````')).check();
assert.equal(missing.results[0].status, 'error');
assert.equal(missing.results[0].message, 'md block at L5 needs fixture=<dir>: the project the sample page is checked in');
```
