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
  with: { path: ~/.docattest, key: "docattest-0.2.2-${{ runner.os }}-${{ runner.arch }}" }
- run: cargo install docattest --version 0.2.2 --locked --root ~/.docattest
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

  A surface may be a binary crate, which nothing imports, by a path: its root file
  (`"./src/bin/shopd.rs"`, `"./src/main.rs"`), the file of one of its modules, or the
  directory of a package that has no library and one binary. Facts read only its
  constants (see [Facts](#facts)). <!-- test: tests/constants.rs > "a_fact_reads_constants_from_source" -->
- **What a surface exports** is every `pub` item its module declares, except modules and
  [docs helpers](#docs-helpers-cfgdocattest), and what it re-exports with `pub use` (a
  glob's by the exports of the module it names). <!-- test: src/analysis/modules.rs > "tests" > "exports_are_public_items_and_what_is_reexported" -->
  `uncovered` lists the exports no page attests. A crate whose root only declares modules
  exports nothing at its root: name the modules as surfaces. A binary exports nothing.
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
  ! L37 fact: fact `round(50)`: round is a function that surface "default" does not export; of its unexported names, a fact reads only constants
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
  build only what changed. Deleting `target/docattest` costs nothing but that time. In a
  copy of the fixture, so that its other examples keep theirs:

  ```console check cwd=tests/fixture
  $ d="$(mktemp -d)" && cp -R . "$d" && cd "$d" && rm -rf target/docattest && docattest check --quiet docs/pricing.md > /dev/null && ls target/docattest/crate && rm -rf "$d"
  Cargo.lock
  Cargo.toml
  harness
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
  The time counts from when the block's own code starts, which its program says as `main`
  begins. The system's start-up of a freshly built program comes before, and on a loaded
  machine it can take seconds: it is not counted. <!-- test: tests/session.rs > "a_blocks_time_counts_from_when_its_code_starts" -->
  A program that has not started after 10 s is killed, and its block fails with
  `did not start within 10000 ms`. <!-- test: src/toolchain/mod.rs > "tests" > "a_program_that_never_starts_is_ended" --> <!-- test: src/running/blocks.rs > "tests" > "a_block_whose_program_never_starts_says_how_long_it_waited" -->
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

It may use the surface's exports, its module's `pub`
[docs helpers](#docs-helpers-cfgdocattest), the `pub` items of the setup file,
`snapshot("name")`, `manifest("path")`, `file("path")`, and by its path anything of `std`,
of your crates or of what they depend on (`shop::shipping::CARRIERS`). A bare name the surface's module does
not declare is an error (`nowhere is not exported by surface "default"`), and so is a
function, a type or a variable it declares and does not export (`round is a function that
surface "default" does not export; of its unexported names, a fact reads only constants`),
which is what keeps a page honest about what a user can see. <!-- test: tests/session.rs > "a_fact_without_a_value_says_why_and_costs_the_others_nothing" -->
A path through a surface's module names that surface's export, as it does in an example,
so `where`, `uncovered` and `impact` find the fact. <!-- test: tests/workspace.rs > "a_fact_names_the_surface_exports_its_crate_paths_reach" -->
A call of a function of the setup file names what that function uses of the surfaces,
and what the setup file's own functions it calls use, at any depth, as a cases table's
runner does. So `where`, `uncovered` and `impact` find the fact under those exports. <!-- test: tests/helpers.rs > "a_fact_names_what_the_setup_functions_it_calls_reach" -->
In the fixture, `error_messages()` reads `ERROR_CODES`:

```console check cwd=tests/fixture
$ docattest where ERROR_CODES
docs/pricing.md:63 fact  The error messages, through a function of the setup module:
```

The page's literal is compared with the value as Rust displays it: `Display` when the type
has it, else `Debug`. So `true`, `3`, `7.50 EUR` for a type that prints itself so, and
`Some(3)` or `["a", "b"]` for one that has only `Debug`. A list on the page is compared
with the items of anything a reference to which can be iterated (an array, a slice, a
`Vec`, a set), each shown the same way. A map's items are its keys (`HashMap`, sorted, and
`BTreeMap`), and so are those of a list of pairs, which is how a constant map is usually
written: `<!-- fact: TAX_RATES -->` above a list checks the keys of
`pub const TAX_RATES: [(&str, u32); 3]`.
<!-- test: src/harness/observe.rs > "tests" > "maps_and_lists_of_pairs_show_their_keys" -->

Constants are read from source instead, since a service's most documented values, its
port or its default timeout, are often private or in its binary. A fact may name any
`const`, or `static` that is not `mut`, declared at the top level of its surface's
module, exported or not, and a surface may be a binary crate, of which only constants can
be read. Here `shopd` is the fixture's `./src/bin/shopd.rs`:

```rust
// src/bin/shopd.rs
use std::time::Duration;

const LISTEN_PORT: u16 = 8080;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const REGION: &str = "nl";
```

```md check fixture=tests/fixture
The server listens on port `8080`. <!-- fact @shopd: LISTEN_PORT -->
A request is cancelled after `30s`, <!-- fact @shopd: REQUEST_TIMEOUT -->
which is `30` seconds. <!-- fact @shopd: REQUEST_TIMEOUT.as_secs() -->
A basket that names no region is taxed as `NL`. <!-- fact @shopd: REGION.to_uppercase() -->
Germany is served in `2` days. <!-- fact @shipping: DAYS[1].1 -->
```

The constant's declaration is copied into the fact's program, with those of the other
constants of its module it reads, and the compiler gives it its value there: it is not
taken from a build of the crate, and nothing of the crate runs. A binary is never built,
so one whose code does not compile still has its constants read. <!-- test: tests/constants.rs > "a_fact_reads_constants_from_source" -->
A fact that names nothing of your crates (no export of its surface, no name of the setup
file, and no crate by its name) is built in a program that does not depend on them. So a
library that does not compile fails only the facts that need it, each with the compiler's
error: `surface "default" failed to load: shop/src/lib.rs:34: mismatched types`. <!-- test: tests/constants.rs > "a_library_that_does_not_compile_fails_only_the_facts_that_need_it" -->
When such a fact does not compile without them, as a call of a method of a trait its
surface exports does not, it is built with them after all. <!-- test: tests/constants.rs > "a_fact_reads_constants_from_source" -->
A constant is copied when what it is written with means the same in any program: its type
is made of primitive types (`u16`, `f64`, `&str`, `char`) and `std::time::Duration`, in
references, slices, arrays and tuples, and its value of literals, operators, casts,
arrays, tuples, references, fields and indexes, the module's other such constants by
their names, and the constants and functions of std (`u32::MAX`, `Duration::from_secs(30)`).
A constant keeps its type, so `REQUEST_TIMEOUT` is a `Duration` and shows as one. <!-- test: src/analysis/constants/literal.rs > "tests" > "literal_values_of_std_types_are_reproduced" -->
Any other constant, of a type the crate declares, that calls a function of the crate or
is declared under `#[cfg]`, is an error that says why (`LEVEL is a constant that surface
"default" does not export, and it cannot be read from source: its type `Level` is not made
of primitive types and Duration`). <!-- test: tests/constants.rs > "only_constants_are_read_from_source" -->
A fact attests the constants it reads, exported or not, as `where` shows; `uncovered` lists
only exports. So `impact` lists a change to such a constant as it lists a change to an
export, with the facts that read it: a private constant, or any constant of a binary. <!-- test: tests/endtoend.rs > "impact_of_a_constant_read_from_source" -->
A constant's value holds the values its initializer uses. So a fact that reads a constant,
directly, through a helper, or in a cases runner, also attests the exports and constants
of its module that the initializer names, at any depth through other constants: with
`pub const COMMAND_ENV: &[&str] = &[COMMAND_PATH]`, a fact over `COMMAND_ENV` attests
`COMMAND_PATH`, and `where COMMAND_PATH` lists it. <!-- test: tests/initializers.rs > "a_value_read_attests_what_its_initializer_uses" --> <!-- test: tests/initializers.rs > "where_and_uncovered_follow_initializers" -->
A function is not followed: a fact that calls `attempts()` attests `attempts`, and not
what its body reads. <!-- test: tests/initializers.rs > "a_value_read_attests_what_its_initializer_uses" -->

#### Docs helpers: `#[cfg(docattest)]`

A page may state a value the crate keeps private, of a type of its own, which no constant
read from source can show: the names in a private `const CAPS: &[Cap]`. A docs helper
shows it with no export. It is an item of the surface's module under `#[cfg(docattest)]`:

```rust
// src/caps.rs
struct Cap { name: &'static str, watts: u32 }

const CAPS: &[Cap] = &[Cap { name: "scan", watts: 2 }, Cap { name: "record", watts: 5 }];

#[cfg(docattest)]
pub fn cap_names() -> Vec<&'static str> {
    CAPS.iter().map(|cap| cap.name).collect()
}
```

```markdown
The radio can: <!-- fact @caps: cap_names() -->
- `scan`
- `record`
```

Every program docattest builds compiles your crates with `--cfg docattest`, and nothing
else does, so `cargo build`, `cargo test` and your users never compile a docs helper. As
code of its module, a docs helper sees all the module sees, private items included, and a
fact calls it by its name, as it calls an export. <!-- test: tests/helpers.rs > "a_fact_calls_a_docs_helper_by_its_name" -->
An example stands for code your users write, and they build without the cfg. So a block
of a page, or an example of a doc comment, that names a docs helper (by a path, a `use` or
a glob import) is refused before it is built, at the line that names it: `uses cap_names, a
docs helper of surface "caps": a page's example runs as users build the code, without
#[cfg(docattest)] (block line 1)`. <!-- test: tests/helpers.rs > "a_page_states_what_only_helpers_say" -->
It must be `pub`, since the fact's program is a crate of its own that imports the
surface's module. One that is not `pub`, or one of a binary, is an error that says so
(`hidden is a docs helper of surface "caps" that a fact cannot call: …`). <!-- test: tests/helpers.rs > "a_fact_calls_only_a_docs_helper_that_is_pub_in_a_library" -->
A docs helper is no export, so `uncovered` never lists it. A fact attests the helper, the
exports it uses, the constants it reads and the functions of its module it calls. It
follows the other docs helpers and the unexported constants the helper reads, at any depth.
So `where CAPS` lists the fact over `cap_names()`, and `impact` lists it when `CAPS` or the
helper changes. <!-- test: tests/helpers.rs > "a_fact_attests_a_docs_helper_and_what_it_reaches" --> <!-- test: tests/helpers.rs > "a_page_states_what_only_helpers_say" -->
A docs helper is glue, so its body is followed. The body of an ordinary function of the
module is not, as that of a function a fact calls is not. `total_watts()` calls the
private `watts()`, which reads `CAPS`, so a fact over `total_watts()` attests `watts` and
not `CAPS`. <!-- test: tests/helpers.rs > "a_fact_attests_a_docs_helper_and_what_it_reaches" --> <!-- test: tests/helpers.rs > "a_page_states_what_only_helpers_say" -->

The cfg is added to `RUSTFLAGS` when that is set, else to cargo's `build.rustflags`, which
cargo joins with those of your own config: the flags your crates build with are kept. <!-- test: src/toolchain/cargo.rs > "tests" > "the_docs_cfg_joins_the_flags_the_project_builds_with" -->
The generated crate has a target directory of its own, so the cfg never makes cargo build
your own target directory again, and every program there is built with the same flags, so
your dependencies are compiled once. Declare the cfg in your manifest, so that your own builds do not
warn of it:

```toml
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ["cfg(docattest)"] }
```

A fact with `field=` checks one value of a JSON, TOML or YAML block
([facts](../../docs/facts.md#a-fact-can-check-one-field-of-a-data-block)), which is
written as its format writes values. The adapter shows the block's value as Rust displays
it, so it compares with the fact's as the same value: a number written as an integer as
that integer, any other as an `f64` (`1.0` is `1`), and `null` as the `Option::None` it
stands for, `None`, as a typed `None` constant shows. <!-- test: src/language/display.rs > "tests" > "a_data_blocks_values_show_as_rust_shows_them" -->
A null in a manifest or a snapshot shows the same, so a fact over JSON data agrees with one
over a typed constant; a null inside an array or an object is written as JSON writes it. <!-- test: src/harness/json.rs > "tests" > "values_are_written_as_json_and_shown_as_a_page_states_them" -->

`snapshot("name")` is the snapshot's data as a small JSON value: index it with a key or a
position, iterate an array, and `to_string()` a member:
`snapshot("regions").iter().map(|r| r["code"].to_string()).collect::<Vec<_>>()`.
`manifest("Cargo.toml")` is the data of a project file, read and parsed by the core (JSON,
TOML, YAML and `go.mod`; see [manifests](../../docs/manifests.md)), as the same JSON value:
`manifest("Cargo.toml")["package"]["rust-version"]`, whose strings are text, as a `String`
is. <!-- test: tests/session.rs > "facts_show_values_as_rust_prints_them" --> The path is
relative to the project root and written out in the fact: a path built at run time is
never read.

`file("path")` is the text of any other project file, a `String`, read by the same rules,
for the methods of `str`: this crate is released under the `MIT` license. <!-- fact: file("LICENSE").split_whitespace().next().unwrap() -->

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

A runner gets each cell's text as the page writes it, code spans included: a cell
`` `FOO=1` `` reaches it with its backticks
([docs/cases.md](../../docs/cases.md#a-runner-receives-each-cell-as-written)).
`docattest_harness::cells::code_spans` takes the code out: it gives the content of every
code span in a cell, in order, as a fact over the cell's column compares it. <!-- test: tests/cells.rs > "code_spans_are_what_a_column_fact_compares" -->

```rust check
let code_spans = docattest_harness::cells::code_spans;
assert_eq!(code_spans("`FOO=1` `BAR=2`"), ["FOO=1", "BAR=2"]);
assert_eq!(code_spans("``a`b`` or `` `c` ``"), ["a`b", "`c`"]);
assert!(code_spans("no code").is_empty());
```

A span opens with a run of backticks and closes at the next run of as many, and the white
space around its content is dropped. As CommonMark has it, a backtick escaped with a
backslash opens no span, and neither does one inside an HTML tag or comment, an autolink,
or a link's destination or title. <!-- test: src/harness/cells/mod.rs > "tests" > "a_cells_code_spans_are_the_shared_tables" -->
`docattest_harness` is the part of every program docattest generates that is the same
each time: a crate written with only `std`, which your examples, facts and setup file are
compiled with. So a setup file calls it with nothing added to your build. <!-- test: tests/cells.rs > "a_setup_file_reads_its_cells_with_code_spans" -->

```rust
fn spans(row: &Row) -> Result<(), String> {
    let got = docattest_harness::cells::code_spans(&row["cell"]).join(",");
    if got == row["spans"] { Ok(()) } else { Err(format!("got {got}")) }
}
```

A table attests what its runner uses of the surfaces, and what the setup file's own
functions it calls use, at any depth. <!-- test: tests/helpers.rs > "a_fact_names_what_the_setup_functions_it_calls_reach" -->

A row is named by the page, its line and its first cell. A runner that panics fails its
row, and the next row still runs:

```console check cwd=tests/fixture
$ docattest check docs/broken.md | grep ' cases: '
  ✗ L42 cases: docs/broken.md:42 wrong on purpose: expected 800, got 750
  ✗ L43 cases: docs/broken.md:43 overflows: panicked: attempt to multiply with overflow
  ✗ L44 cases: docs/broken.md:44 too much: The discount is outside the allowed range
  ! L46 cases: cases runner "nope" at L46 is not in CASES of docs/docattest_setup.rs (known: delivery, discount)
```

A table's rows run one after another, in the page's order, in one process, each on a
thread of its own. Statics, and anything a row starts and does not stop (threads,
servers), are still there for the rows after it, so a runner stops what it starts before
it returns. After a row that panics, the next row runs in the same process. A row that runs out of
time or ends the process (`std::process::exit`) is the last of its process: the next row
starts a fresh one. <!-- test: tests/cases.rs > "a_tables_rows_share_its_process" -->
Tables never share a process, and run side by side, with each other and with blocks:
`runtime.workers` processes at once in all. <!-- test: tests/cases.rs > "tables_run_side_by_side" -->

### Citations

`<!-- test: src/lib.rs > "tests" > "rejects_big_discounts" -->` ties a sentence to a test.
A test is a function with an attribute whose name ends in `test` (`#[test]`,
`#[tokio::test]`, `#[rstest]`) or is `test_case`, titled by its name, with the modules
written around it named first, outermost first. Citing a module is an error that says so
(`"tests" in src/lib.rs is a module, not a test`). <!-- test: tests/statics.rs > "tests_are_listed_with_the_modules_around_them" -->
The file is read, never run.

The cases of a parametrized test can be cited too, by the names its source writes, with
the function named around them: rstest's `#[case::quarter(1000, 25, 750)]` is
`"tests" > "discount" > "quarter"`, and test-case's
`#[test_case(1000, 25 => 750 ; "a quarter off")]` is `"tests" > "rounds" > "a quarter off"`.
`cargo test` runs each case in a module named after the function, which is why the function
is a level of its own, and prints a name that begins with the case's position
(`tests::discount::case_1_quarter`, and `test_case_1_a_quarter_off` from test-case 3.4 on):
a citation does not use that name, since a case added above shifts it. A case its source
gives no name (`#[case(999, 50, 500)]`, `#[test_case(999, 50 => 500)]`) is named only by
its position or its values, and cannot be cited; neither can an argument of `#[values(…)]`. <!-- test: src/analysis/tests/mod.rs > "listing" > "the_named_cases_of_parametrized_tests_are_tests" -->
The function itself is cited for all its cases. A loop over a table inside a `#[test]` is
one test: Rust's test harness has no subtests, so its rows have no names to cite.

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
  what the setup module's `pub` functions that take no arguments return. They do not see
  the constants a fact reads from source, so nothing of the binary `shopd`. In the
  fixture, `Money` is a struct and `rates` takes the regions:

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
- A cargo config that sets `target.<triple>.rustflags` or `target.'cfg(…)'.rustflags`
  takes the place of `build.rustflags`, so docattest's build lacks `--cfg docattest`, and
  a fact that calls a docs helper says so. Set `RUSTFLAGS` to those flags for docattest's
  run: the cfg is then added to it. <!-- test: tests/helpers.rs > "a_cargo_config_that_takes_the_place_of_build_rustflags_leaves_docs_helpers_out" -->
- Two docattest runs on one project at the same time share `target/docattest`: cargo makes
  one build wait for the other, but they may rewrite each other's generated files.
- This adapter has not been run on Windows.

## Working on this package

[CONTRIBUTING.md](CONTRIBUTING.md) has how the crate is laid out, and how to test, lint and
format it.

## License

[MIT](LICENSE)
