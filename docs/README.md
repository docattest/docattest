# docs

docattest's documentation. Every page here is an attested page: its examples run, its facts
are checked against the npm package, and its tables run row by row, on every build
(`npm run test:docs`, configured by [docattest.config.json](../docattest.config.json)). So the
pages are also docattest's acceptance tests. Start at [index.md](index.md), which maps the
rest; one page with no proofs, on the reasoning behind the design, is the only page the
report lists as uncovered.

The design documents in [`design/`](design) are frozen: the report lists them and never
checks them. They hold the specification, the authoring pattern, the acceptance criteria and
a corpus of real drift events; the npm package's tests hold one test to each criterion and
each event that can be automated. The setup module beside the pages holds the runners of
their cases tables.

`.fixtures/` holds small projects that pages run docattest in (a `console check` block's
`cwd=`, an `md check` block's `fixture=`); a dot-directory, so `docs/**/*.md` does not take
in their pages.

This README is frozen too. The examples in these pages are TypeScript, because they drive
docattest through the npm package; each other language's package has its own guide in its
README under [adapters/](../adapters/README.md).

## How to add a page

1. Write the page in this directory, with a proof beside each claim (the kinds of proof are
   in [index.md](index.md)).
2. Link it from [index.md](index.md), so a reader can find it.
3. Run `npm run test:docs`; this repository is also a recorded project, so record its
   parity outputs in the same change ([CONTRIBUTING.md](../CONTRIBUTING.md#recording-on-purpose)).
