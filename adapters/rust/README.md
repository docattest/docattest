# docattest for Rust

[docattest](../../README.md) for Rust projects: documentation that CI can fail. Your
Markdown pages carry examples that run, facts that are compared with values in your code,
tables whose rows are tests, and citations of tests you already have. The examples in your
doc comments are checked the same way.

Try it in your browser first: the [playground](https://docattest.github.io/docattest/playground.html)
lets you break a small project's code and watch docattest catch its docs.

This package is what a Rust user installs. It provides two commands:

- **`docattest`**, the command you run. The docattest core is written in Rust, so this
  command is built with the core inside it: it tells the core how to start the Rust adapter
  and what command runs docattest in your project, and runs it.
- **`docattest-adapter-rust`**, the adapter: the program that answers everything that
  needs Rust. You do not run it yourself. It speaks
  [the adapter protocol](../../protocol/PROTOCOL.md), version 1. <!-- fact @protocol: VERSION -->

Your project needs `cargo` on the `PATH`, as it does to be built: examples and facts are
compiled by it. Node, Python and Go are never needed.

## Install

```sh
cargo install docattest
```

That builds both commands, the core among them (the crate `docattest-core`), and puts them
side by side in cargo's bin directory. Nothing is downloaded when docattest runs. From a
checkout of the repository, `cargo install --path adapters/rust` does the same.

Building takes a minute or two (no prebuilt binary is published for `cargo binstall`). In
CI, cache a directory of its own to install into: cargo then finds that version there.

```yaml
- uses: actions/cache@v4
  with: { path: ~/.docattest, key: "docattest-0.1.4-${{ runner.os }}-${{ runner.arch }}" }
- run: cargo install docattest --version 0.1.4 --locked --root ~/.docattest
- run: echo "$HOME/.docattest/bin" >> "$GITHUB_PATH"
```

Another core runs in place of the one built in when `DOCATTEST_CORE` names it. <!-- fact @launcher: CORE_VARIABLE -->
That core is asked for its version (`--version --json`) and refused when it speaks another
protocol than this package. <!-- test: src/launcher.rs > "tests" > "a_core_someone_names_is_asked_what_it_speaks" -->
`DOCATTEST_TRACE=1` says which core runs. A bare core, run without this command, finds the
adapter as `docattest-adapter-rust` on the `PATH`, or through `DOCATTEST_ADAPTER_RUST`. <!-- fact @launcher: ADAPTER_VARIABLE -->
That variable holds a JSON array (`["/path/to/docattest-adapter-rust"]`) or a command line
split on spaces.

`DOCATTEST_CARGO` names the `cargo` the adapter runs, when it is not the one on the `PATH`:

```console check cwd=tests/fixture
$ DOCATTEST_CARGO=/no/such/cargo docattest check docs/pricing.md | head -2
docs/pricing.md
  ✗ L5 example: cargo could not read the project: /no/such/cargo could not be run: No such file or directory (os error 2) (block line 1)
```

`DOCATTEST_CARGO_FEATURES` names the features your crates are built with for examples,
facts and tables, as cargo's `--features` takes them (`shop/eu`); else each has its default
features. <!-- test: tests/workspace.rs > "the_generated_crate_is_built_with_the_features_it_is_given" -->

`docattest hooks install`, `hooks install --git` and `agents install` write down a command
that runs docattest: `docattest` when that is what a shell finds on the `PATH`, else this
command by its full path. <!-- test: src/launcher.rs > "tests" > "the_hand_over_adds_only_what_is_not_set" -->
To have them write something else, set `DOCATTEST_COMMAND`. <!-- fact @launcher: COMMAND_VARIABLE -->

## Configure

`docattest.config.json` at the project root:

```json
{
  "pages": ["README.md", "docs/**/*.md"],
  "sources": ["src/**/*.rs"],
  "surfaces": { "default": "shop", "shipping": "shop::shipping" },
  "setup": "docs/docattest_setup.rs",
  "adapter": "rust"
}
```

- **`surfaces`** name what pages are allowed to observe. A surface is one module of a
  library of your cargo workspace that its users can import: a module declared without
  `pub`, or one below it, is refused, with a warning that names it. <!-- test: tests/workspace.rs > "a_private_module_is_no_surface_and_says_why" -->
  An entry is
  - a module path, `"shop"` or `"shop::shipping"`: the crate's root, or a module below it;
  - a path, `"."` (the package there, by its root module) or `"./src/shipping.rs"` (the
    module that file is);
  - both, `{ "import": "shop::shipping", "path": "src/shipping.rs" }`.
- **What a surface exports** is every `pub` item its module declares, except modules, and
  what it re-exports with `pub use` (a glob's by the exports of the module it names). <!-- test: src/analysis/modules.rs > "tests" > "exports_are_public_items_and_what_is_reexported" -->
  `uncovered` lists the exports no page attests. A crate whose root only declares modules
  exports nothing at its root: name the modules as surfaces.
- **`sources`** are the Rust files whose doc-comment examples are checked (see below).
- **`setup`** is one Rust file with the runners of cases tables and what facts may use by
  name.

## A page

````md check fixture=tests/fixture
# Pricing

A discount is a whole percentage taken off a price.

```rust check
use shop::{apply_discount, Money};

let price = apply_discount(Money::eur(1000), 25).unwrap();
assert_eq!(price.to_string(), "7.50 EUR");
```

Prices are in `EUR` unless told otherwise. <!-- fact: DEFAULT_CURRENCY -->

Tax is known for these regions: <!-- fact: TAX_RATES -->
- `NL`
- `DE`
- `FR`

<!-- cases: discount -->
| case           | price | percent | result |
|----------------|-------|---------|--------|
| a quarter off  | 1000  | 25      | 750    |
| rounds half up | 999   | 50      | 500    |

Discounts above the maximum are refused. <!-- test: src/lib.rs > "tests" > "rejects_big_discounts" -->
````

```sh
docattest check          # exit 1 when a page and the code disagree
docattest uncovered      # exports no page attests
docattest where total    # the pages, lines and sentences that attest a symbol
docattest audit          # lists, tables and values that look copied from the code, with no directive
docattest impact         # after an edit: the exports it affects and the prose that names them
```

When a page and the code disagree, `docattest check` lists each proof that does not hold,
by its line, with what went wrong, and exits with 1. Here it is on a page of this package's
test fixture that is wrong on purpose:

```console check cwd=tests/fixture
$ docattest check docs/broken.md
docs/broken.md
  ✗ L5 example: panicked: assertion `left == right` failed: expected Ok(Money { cents: 700, currency: "EUR" }), got Ok(Money { cents: 750, currency: "EUR" }) (block line 3)
  ✗ L11 example: unresolved import `shop::aply_discount` (block line 1)
  ✗ L15 example: panicked: called `Result::unwrap()` on an `Err` value: "no carrier serves \"XX\"" (block line 1)
  ✗ L19 example: timed out after 3000 ms
  ✗ L23 example: ran to its end, and says it panics
  ! L27 example: example at L27: unknown tag "chekc"; did you mean "check"?
  ✗ L31 fact: expected 60, got 50 from MAX_DISCOUNT
  ✗ L33 fact: missing: DE, FR; unexpected: BE (2 items on the page, 3 in TAX_RATES)
  ! L37 fact: fact `round(50)`: round is not exported by surface "default" (cannot find function `round` in this scope)
  ✗ L42 cases: docs/broken.md:42 wrong on purpose: expected 800, got 750
  ✗ L43 cases: docs/broken.md:43 overflows: panicked: attempt to multiply with overflow
  ✗ L44 cases: docs/broken.md:44 too much: The discount is outside the allowed range
  ! L46 cases: cases runner "nope" at L46 is not in CASES of docs/docattest_setup.rs (known: delivery, discount)
  ! L51 test: src/lib.rs declares no test "test_that_was_renamed"; tests there: "tests" > "adds_tax", "tests" > "rejects_big_discounts"
  ! L53 test: "tests" in src/lib.rs is a module, not a test; tests there: "tests" > "adds_tax", "tests" > "rejects_big_discounts"
results: 0 pass, 10 fail, 5 error, 0 skipped
...
FAILED
[exit 1]
```

## What each attestation means in Rust

### Examples: `rust check`, `rust compile`

Blocks tagged `rust` or `rs` are the adapter's. <!-- fact: hello().block_languages -->
A block is read as rustdoc reads a doc test, and compiled as a program of its own that
depends on your crates.
<!-- test: tests/session.rs > "blocks_compile_and_run_each_as_a_program_of_its_own" -->

- **What a block is.** A block that declares `fn main` is a whole program. Any other is
  the body of `main`: Rust lets a function hold `use` declarations and items, so a snippet
  needs nothing more. <!-- test: src/language/blocks.rs > "tests" > "a_snippet_is_the_body_of_main" -->
  A line that begins `# ` is part of the program and hidden by renderers (rustdoc, mdBook). <!-- test: src/language/blocks.rs > "tests" > "hidden_lines_are_part_of_the_program_and_keep_their_place" -->
- **`check`** builds the block and runs it. The proof is an assertion (the macros and
  methods that count are listed below). A failed `assert_eq!` reports both sides, and every
  panic the line of the block it came from. A block that holds none of these runs, but is
  counted as *weak*. <!-- test: src/language/text.rs > "tests" > "a_block_asserts_when_something_in_it_can_end_the_program" -->
- **`check should_panic`** passes when the block panics, and fails when it runs to its end.
- **`compile`** builds the block and never runs it. The diagnostics are the compiler's own
  errors, at the lines of the block. Warnings are not reported: a snippet is not held to a
  crate's lints, as a doc test is not. <!-- test: src/toolchain/cargo.rs > "tests" > "errors_are_read_from_the_compilers_messages" -->
- **Where it is built.** In a crate the adapter generates in your target directory
  (`target/docattest`), never inside your sources. That crate depends on the libraries of
  your workspace by path and on what they depend on, dev-dependencies included, so a block
  may use what a doc test may use; it carries your workspace's `[patch]` tables and starts
  from your `Cargo.lock`. What cargo built there is still there the next time: the first
  run of a project compiles its dependencies once more, for this crate, and later runs
  build only what changed. Deleting `target/docattest` costs nothing but that time.
  In the fixture:

  ```console check cwd=tests/fixture
  $ rm -rf target/docattest
  $ docattest check --quiet docs/pricing.md
  ...
  $ ls target/docattest/crate
  Cargo.lock
  Cargo.toml
  src
  ```
- **Isolation.** Every block is its own program and its own process, started in the
  project root. Blocks share nothing. This guide's project root is `adapters/rust`:

  ```rust check
  assert!(std::path::Path::new("docattest.config.json").is_file());
  assert!(std::path::Path::new("tests/fixture").is_dir());
  ```
- **Time.** A block that outlives `runtime.timeoutMs` is killed with everything it
  started. <!-- test: src/toolchain/mod.rs > "tests" > "a_program_that_overruns_is_ended_with_what_it_started" -->
  `runtime.workers` is the number of blocks that run at once. Building is not counted: the
  blocks of a run are built together, once, before any of them runs.
- **Output.** What a block prints is dropped; it never reaches the report. The block of
  the fixture's pricing page at L35 prints a price and asserts nothing:

  ```console check cwd=tests/fixture
  $ docattest check docs/pricing.md
  docs/pricing.md
    ✓ L5 example: all assertions held
    ✓ L14 example: all assertions held
    ✓ L25 example: compiles
    ✓ L35 example: ran to its end, but asserts nothing (weak)
    ✓ L43 example: all assertions held
  ...
  ok
  ```
- **Network.** A block tagged `check network` runs only with `DOCATTEST_NETWORK=1`, and is
  skipped otherwise. **The network is not taken away from other blocks**: Rust has no place
  to refuse it from inside a program, and this adapter does not sandbox what it runs.

  ```console check cwd=tests/fixture
  $ docattest check docs/shipping.md | grep network
    - L24 example: network example skipped: the network is off (set DOCATTEST_NETWORK=1 to run it)
  ```

A block asserts when it calls one of these macros: <!-- fact: ASSERTING_MACROS.map(|name| format!("{name}!")) -->
- `assert!`
- `assert_eq!`
- `assert_ne!`
- `assert_matches!`
- `debug_assert!`
- `debug_assert_eq!`
- `debug_assert_ne!`
- `panic!`
- `unreachable!`

or one of these methods, which end the program on a value they do not expect: <!-- fact: ASSERTING_METHODS -->
- `unwrap`
- `expect`
- `unwrap_err`
- `expect_err`

### Examples in doc comments

Every Rust block in a doc comment (`///`, `//!`) of a `sources` file is an example, as
`cargo test --doc` takes it: a block with no info string, or with only these words: <!-- fact: RUST_WORDS -->
- `rust`
- `no_run`
- `should_panic`
- `edition2015`
- `edition2018`
- `edition2021`
- `edition2024`

Any other word leaves the block alone, `ignore` and `compile_fail` among them. `no_run` is
compiled and not run, and `should_panic` must panic. The claim an example defends is the
prose of its doc comment, and it attests the item it documents (a method's attests its
type). <!-- test: src/language/doc.rs > "tests" > "examples_are_the_rust_blocks_of_doc_comments" -->

```rust
/// Takes a percentage off a price, rounding half up to a whole cent.
///
/// ```
/// use shop::{apply_discount, Money};
///
/// assert_eq!(apply_discount(Money::eur(999), 50), Ok(Money::eur(500)));
/// ```
pub fn apply_discount(price: Money, percent: u32) -> Result<Money, String> {
```

That is the fixture's `apply_discount`, and its `total` has a `no_run` example. Both are
checked, and each is listed under the item it documents, with its doc comment's sentence:

```console check cwd=tests/fixture
$ docattest check src/lib.rs
src/lib.rs
  ✓ L46 example: all assertions held
  ✓ L60 example: compiles
...
$ docattest where total | grep src/
src/lib.rs:60 example  Adds up a basket and the tax of a region.
```

### Facts

A fact expression is a Rust expression over the surface's exports, by their own names:

```markdown
A discount is at most `50` percent. <!-- fact: MAX_DISCOUNT -->
The carriers: <!-- fact @shipping: CARRIERS -->
```

It may use the surface's exports, the `pub` items of the setup file, `snapshot("name")`,
`manifest("path")`, and by its path anything of `std`, of your crates or of what they
depend on (`shop::shipping::CARRIERS`). A bare name the surface does not export is an error
(`round is not exported by surface "default"`), which is what keeps a page honest about
what a user can see.
<!-- test: tests/session.rs > "a_fact_without_a_value_says_why_and_costs_the_others_nothing" -->
A path through a surface's module names that surface's export, as it does in an example,
so `where`, `uncovered` and `impact` find the fact. <!-- test: tests/workspace.rs > "a_fact_names_the_surface_exports_its_crate_paths_reach" -->

The page's literal is compared with the value as Rust displays it: `Display` when the type
has it, else `Debug`. So `true`, `3`, `7.50 EUR` for a type that prints itself so, and
`Some(3)` or `["a", "b"]` for one that has only `Debug`. A list on the page is compared
with the items of anything a reference to which can be iterated (an array, a slice, a
`Vec`, a set), each shown the same way. A map's items are its keys (`HashMap`, sorted, and
`BTreeMap`), and so are those of a list of pairs, which is how a constant map is usually
written: `<!-- fact: TAX_RATES -->` above a list checks the keys of
`pub const TAX_RATES: [(&str, u32); 3]`.
<!-- test: src/harness/observe.rs > "tests" > "maps_and_lists_of_pairs_show_their_keys" -->

`snapshot("name")` is the snapshot's data as a small JSON value: index it with a key or a
position, iterate an array, and `to_string()` a member:
`snapshot("regions").iter().map(|r| r["code"].to_string()).collect::<Vec<_>>()`.
`manifest("Cargo.toml")` is the data of a project file, read and parsed by the core (JSON,
TOML, YAML and `go.mod`; see [manifests](../../docs/manifests.md)), as the same JSON value:
`manifest("Cargo.toml")["package"]["rust-version"]`, whose strings are text, as a `String`
is. <!-- test: tests/session.rs > "facts_show_values_as_rust_prints_them" --> The path is
relative to the project root and written out in the fact: a path built at run time is
never read.

### Cases tables and named facts: the setup file

```rust
// docs/docattest_setup.rs
use shop::{apply_discount, Money};
use std::collections::HashMap;

type Row = HashMap<String, String>;

/// The runners of cases tables, by name. A runner is called once per row with the row's
/// cells by column; it returns `Ok` when the row holds, else why it does not.
pub const CASES: &[(&str, fn(&Row) -> Result<(), String>)] = &[("discount", discount)];

/// (Optional.) The columns each runner's table must have.
pub const COLUMNS: &[(&str, &[&str])] = &[("discount", &["price", "percent", "result"])];

/// Something a fact may use: <!-- fact: error_messages() -->
pub fn error_messages() -> Vec<&'static str> { … }
```

The setup file is no part of your crates. docattest compiles it, as a module of a program
of its own that depends on them, so it imports your crate by its name, as a page does, and
`cargo build` never sees it.

A runner that panics fails its row, and the next row still runs:

```console check cwd=tests/fixture
$ docattest check docs/broken.md | grep ' cases: '
  ✗ L42 cases: docs/broken.md:42 wrong on purpose: expected 800, got 750
  ✗ L43 cases: docs/broken.md:43 overflows: panicked: attempt to multiply with overflow
  ✗ L44 cases: docs/broken.md:44 too much: The discount is outside the allowed range
  ! L46 cases: cases runner "nope" at L46 is not in CASES of docs/docattest_setup.rs (known: delivery, discount)
```

### Citations

`<!-- test: src/lib.rs > "tests" > "rejects_big_discounts" -->` ties a sentence to a test.
A test is a function with a `#[test]` attribute (or one that ends in `test`, such as
`#[tokio::test]`), titled by its name, with the modules written around it named first,
outermost first. Citing a module is an error that says so
(`"tests" in src/lib.rs is a module, not a test`). <!-- test: tests/statics.rs > "tests_are_listed_with_the_modules_around_them" -->
The file is read, never run.

## What is never your project's own

The adapter tells the core to leave every `target` directory alone. <!-- fact: hello().ignored_names.join(", ") --> Hidden directories are
left out by the core itself.

## What is read, and what is asked of the compiler

Exports, the index (`where`, `uncovered`) and `impact` are read from your source as it is
written, without compiling it. That is fast, and it is exact for a module whose items are
all written out.

A module's source does not say everything it exports when it calls a macro where its
items are (`thread_local!`, `include!`, a macro of your own), or re-exports whole
something its source does not lead to (`pub use Kind::*` of an enum, a module of another
crate). For a surface that is such a module, the adapter asks the compiler, through
rustdoc's JSON output, and lists what the compiler lists. This happens by itself, and
only for such a surface. The listing is built beside the examples, in
`target/docattest/doc`, and costs one `cargo check` of your dependencies the first time.

rustdoc's JSON is not stable yet, so the adapter sets `RUSTC_BOOTSTRAP` to the name of
your crate for that one command. When the compiler cannot answer (your crate does not
compile, or a later Rust writes a format this adapter does not read), `check` says so as
a warning and the surface keeps the exports its source declares.

## Limits

- `impact` has no type information, and errs towards naming too much. It cuts a crate
  into the names its modules declare, with what an `impl` block holds as part of its
  type; a path stands for any item of that name, a method is found through its type, and
  a trait is not followed to what implements it.
- What a macro writes has no declaration to compare, so a change to a macro call written
  where a module's items are affects everything that module declares. An export a macro
  wrote that a change removes is not named: the earlier revision is not compiled.
- What a glob re-export of another crate brings is not listed. A surface is a module of
  a crate of your workspace, not of a dependency.
- `audit` and `inventory` see a surface's exports that are values, and not what lies
  below them (the fields of a struct, the values of a map), and on the default surface
  what the setup module's `pub` functions that take no arguments return. In the fixture,
  `Money` is a struct and `rates` takes the regions:

  ```console check cwd=tests/fixture
  $ docattest inventory
  DEFAULT_CURRENCY  →  EUR
  MAX_DISCOUNT  →  50
  TAX_RATES  →  [NL, DE, FR]
  ERROR_CODES  →  [out_of_stock, bad_discount, no_carrier]
  error_messages()  →  [The item is not available, The discount is outside the allowed range, No carrier serves the region]
  @shipping CARRIERS  →  [post, courier, drone]
  inventory: 6 values
  ```
- The network is not refused (see above).
- Two docattest runs on one project at the same time share `target/docattest`: cargo makes
  one build wait for the other, but they may rewrite each other's generated files.
- This adapter has not been run on Windows.

## Working on this package

[CONTRIBUTING.md](CONTRIBUTING.md) has how the crate is laid out, and how to test, lint and
format it.

## License

[MIT](LICENSE)
