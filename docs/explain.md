---
title: Explain: what a directive binds, and why
---

# Explain: what a directive binds, and why

Which literal a fact compares, which sentence a citation defends and which table a
`cases` directive runs are decided by where the directive sits. `docattest explain
<page>:<line>` says what that decision was, so nobody has to break a page on purpose to find
out. For each directive on the line it prints what the directive binds and the rule that
bound it, the target as the page has it, the sentence it defends, and what checking it
gives, as `check` would: for a fact, the value its expression gives and whether that
matches; for a citation, the file it names and whether that file declares the test; for a
table, its rows and a result for each. `docattest explain <page>` does the same for every
directive on the page. Here it is for a small project,
[docs/.fixtures/explain](.fixtures/explain), whose page says the orders ship to two regions
where the code lists three:

```console check cwd=docs/.fixtures/explain
$ docattest explain docs/limits.md:7
docs/limits.md:7 fact: REGIONS
  claim     Orders ship to `EU` and `UK`.
  placed    on the sentence below it, as if written where that sentence ends, on L8
  binds     the code spans of the sentence it ends, as a set
  target    EU, UK (2 items)
  surface   default
  value     [EU, UK, US]
  result    ✗ fail: missing: US (2 code spans on the page, 3 in REGIONS)
```

`placed` says where the directive takes effect: where it is written; alone on the line above
a sentence, as if written at that sentence's end; or nowhere, when a blank line parts it from
the prose below. A citation names its file, and the [alias](citation-shorthands.md#a-page-names-a-test-file-once-with-an-alias)
it was named by, when it was; an alias's own line lists the citations that use it:

```console check cwd=docs/.fixtures/explain
$ docattest explain docs/limits.md:3
docs/limits.md:3 tests: launch = test/launch.test.ts
  cited by  L10
$ docattest explain docs/limits.md:10
docs/limits.md:10 test: test/launch.test.ts > "run" > "python"
  claim     Python and Node.js scripts run.
  placed    where it is written
  file      test/launch.test.ts, by the alias launch
  result    ✓ pass: test/launch.test.ts:2 declares "run" > "python" (not run by docattest)

docs/limits.md:10 test: test/launch.test.ts > "run" > "node"
  claim     Python and Node.js scripts run.
  placed    where it is written
  file      test/launch.test.ts, by the alias launch
  result    ✓ pass: test/launch.test.ts:2 declares "run" > "node" (not run by docattest)
```

`explain` exits `0` whatever the results are: it explains, and `check` judges. A line that
holds no directive is an error that names the lines that do, so a line number that is one off
is quick to put right. A directive's line is the line its comment starts on.

```console check cwd=docs/.fixtures/explain
$ docattest explain docs/limits.md:9
docattest: docs/limits.md:9 holds no directive; directives are at L3, L5, L7, L10
[exit 1]
```

## `--json` and the API

`explain --json`, and `project.explain(page, line)` in the programmatic API, give the same
as an object: for each directive its `line` and `kind`; `placed`, with the line of the
sentence for a directive placed above one; a fact's `rule`; a citation's `alias`; the
`attestation` as the adapter is sent it, with its claim and its target, file or rows; and
the `results`, a fact's carrying the value its expression gives as `actual`. An alias is an
entry of the kind `alias`, with its `name`, its `file` and the lines it is `citedAt`.

```ts check
import { createProject } from 'docattest';

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, adapter: 'typescript' }),
    'src/index.ts': `export const RETRY = { max: 3 };`,
    'docs/a.md': '# Retries\n\n<!-- fact: RETRY.max -->\nA call is tried at most 4 times.\n',
  },
});

const { page, directives } = await project.explain('docs/a.md', 3);
assert.equal(page, 'docs/a.md');
const [fact] = directives;
assert.ok(fact?.kind === 'fact');
assert.deepEqual(fact.placed, { where: 'above', line: 4 });
assert.equal(fact.rule, 'literal');
assert.equal(fact.attestation.claim, 'A call is tried at most 4 times.');
assert.deepEqual(fact.attestation.kind === 'fact' && fact.attestation.target, { kind: 'scalar', value: '4' });
assert.deepEqual(fact.results.map((r) => [r.status, r.actual]), [['fail', '3']]);
await assert.rejects(project.explain('docs/a.md', 1), /docs\/a\.md:1 holds no directive; directives are at L3/);
```

## The rules a fact is bound by

A fact tries these in order, and `rule` names the one that bound it
([facts](facts.md) has the whole story). Each row is a page with one fact, the rule
`explain` names, and where the fact took effect.

<!-- cases: explainedRule -->
| case | page | rule | placed |
|---|---|---|---|
| `field=`: a value of the fenced block after it | <!-- fact field=free_from: N -->¶```json¶{ "free_from": 3 }¶``` | field | written |
| `column=`: a column of the table after it | <!-- fact column=key: KEYS -->¶\| key \|¶\|---\|¶\| `A` \|¶\| `B` \|¶\| `C` \| | column | written |
| the list or fenced block on the line after it | The keys: <!-- fact: KEYS -->¶- `A`¶- `B`¶- `C` | block-below | written |
| the code span before it on its line | At most `3` tries. <!-- fact: N --> | code-span | written |
| the number or version before it on its line | At most 3 tries. <!-- fact: N --> | literal | written |
| the code spans of the sentence it ends | Keys are `A`, `B` and `C`. <!-- fact: KEYS --> | sentence-spans | written |
| the code span of the sentence it ends, on an earlier line | At most `3`¶tries. <!-- fact: N --> | sentence-span | written |
| the list or fenced block after the paragraph it ends | The keys: <!-- fact: KEYS -->¶¶- `A`¶- `B`¶- `C` | block-after | written |
| alone on the line above a sentence | <!-- fact: N -->¶At most `3` tries. | code-span | above |
