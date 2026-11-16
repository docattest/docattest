# Patches to markdown-rs 1.0.0

This is markdown-rs 1.0.0 from crates.io (MIT, Titus Wormer), vendored by docattest with three
changes, each marked `docattest patch` in the source:

- `src/construct/flow.rs`, `start`: the fast paths for lines starting with `#`, `$`, `` ` ``,
  `~`, `*`, `_`, `e`, `i` and `{` fell back straight to content when their construct did not
  match, skipping GFM tables. A table row (or header) without a leading pipe that started with
  one of those characters therefore ended the table, where micromark (the reference parser)
  continues it: ``a | b\n--- | ---\n`x` | y`` is a two-row table. The fallback is now the GFM
  table, then content, which is what the full construct chain would try for those lines.
- `src/construct/code_indented.rs`, `at_break`: micromark checks a new container on a line
  after indented code (blank lines between included) as if it interrupted, because the code
  construct is still open there. So `    code\n\n6. Run` is a paragraph, not an ordered list
  starting at 6, and an empty item there is not a list item. Indented code now sets the
  tokenizer's `interrupt` while it is open; its `after` clears it as upstream does.
- `src/construct/document.rs`, `exit_containers`: closing containers after a line
  (`Phase::After`) reset the flow's `interrupt`, though that line's flow (a paragraph) stays
  open. So in `- a\n\nR\n3. x` the `3.` started a list inside what micromark keeps as the
  paragraph `R\n3. x` (an ordered list not starting at 1 cannot interrupt a paragraph). The
  reset now happens only when the flow is flushed (`Phase::Prefix`, `Phase::Eof`).

Workarounds kept in docattest's own code rather than here (`src/markdown/parse.rs`): line
numbers recomputed from offsets, list and footnote end lines, the start of a paragraph that
opens with an escape or a task check, a leading byte order mark, list items nested in a list
after an unclosed fence, and frontmatter (enabled only when its fence closes, which also
avoids a panic in `to_mdast` on `---\n\n> a`).
