//! What a page reaches through helpers: the functions of the setup file, which cases
//! runners and facts call, and the docs helpers of a surface's module, the items under
//! `#[cfg(docattest)]`. Each test writes a radio's crate, whose module `caps` keeps its
//! capabilities private, to the temporary directory.

mod project;

use project::Written;
use serde_json::{Value, json};

const MANIFEST: &str = "\
[package]
name = \"radio\"
version = \"0.1.0\"
edition = \"2021\"

[lints.rust]
unexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(docattest)'] }
";

const RADIO: &str = "\
pub mod caps;

pub const BANDS: [&str; 2] = [\"FM\", \"AM\"];

pub fn band_count() -> usize {
    BANDS.len()
}
";

/// A module whose capabilities are of a type it keeps private: no constant of it can be
/// read from source, and only docs helpers say what they are.
const CAPS: &str = "\
struct Cap {
    name: &'static str,
    watts: u32,
}

const CAPS: &[Cap] = &[Cap { name: \"scan\", watts: 2 }, Cap { name: \"record\", watts: 5 }];

pub const MAX_WATTS: u32 = 10;

/// Whether the radio has a capability.
///
/// ```
/// use radio::caps::*;
/// assert!(supports(cap_names()[0]));
/// ```
pub fn supports(name: &str) -> bool {
    CAPS.iter().any(|cap| cap.name == name)
}

fn watts() -> u32 {
    CAPS.iter().map(|cap| cap.watts).sum()
}

#[cfg(docattest)]
pub fn cap_names() -> Vec<&'static str> {
    CAPS.iter().map(|cap| cap.name).collect()
}

#[cfg(docattest)]
pub fn total_watts() -> u32 {
    watts().min(MAX_WATTS)
}

#[cfg(docattest)]
fn hidden() -> usize {
    CAPS.len()
}
";

/// A setup file whose runner and whose function a fact calls reach the crate only
/// through helpers of their own.
const SETUP: &str = "\
use radio::{band_count, BANDS};
use std::collections::HashMap;

type Row = HashMap<String, String>;

pub const CASES: &[(&str, fn(&Row) -> Result<(), String>)] = &[(\"band\", band)];

fn band(row: &Row) -> Result<(), String> {
    if known(&row[\"band\"]) { Ok(()) } else { Err(format!(\"no band {}\", row[\"band\"])) }
}

fn known(band: &str) -> bool {
    BANDS.contains(&band)
}

pub fn bands() -> String {
    joined()
}

fn joined() -> String {
    BANDS.join(\", \")
}

pub fn count() -> usize {
    band_count()
}
";

/// A binary, which nothing imports, so no fact can call its docs helper.
const RADIOD: &str = "\
const PORT: u16 = 7000;

#[cfg(docattest)]
pub fn port() -> u16 {
    PORT
}

fn main() {}
";

fn open(test: &str) -> Written {
    open_with(test, &[])
}

/// Opens the radio's crate with some more files.
fn open_with(test: &str, more: &[(&str, &str)]) -> Written {
    let files = [
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", RADIO),
        ("src/caps.rs", CAPS),
        ("src/bin/radiod.rs", RADIOD),
        ("docs/docattest_setup.rs", SETUP),
    ];
    let root = project::write(&format!("helpers-{test}"), &[&files[..], more].concat());
    let surfaces = json!([
        { "name": "default", "entry": "radio" },
        { "name": "caps", "entry": "radio::caps" },
        { "name": "radiod", "entry": "./src/bin/radiod.rs" },
    ]);
    let session = project::open_session(&root, surfaces, json!({ "setup": "docs/docattest_setup.rs" }));
    Written { root, session }
}

fn fact(id: &str, expression: &str, surface: &str) -> Value {
    json!({ "id": id, "page": "docs/page.md", "line": 1, "claim": "", "kind": "fact", "expression": expression, "surface": surface, "ordered": false })
}

/// The symbols each attestation names, as `surface:name` words, by id.
fn named(radio: &Written, attestations: Value) -> Value {
    let found = radio.call("symbols", json!({ "attestations": attestations }));
    let words = found
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, symbols)| (id.clone(), json!(project::surface_names(symbols))));
    Value::Object(words.collect())
}

#[test]
fn a_fact_names_what_the_setup_functions_it_calls_reach() {
    let radio = open("setup");
    let attestations = json!([
        fact("depth", "bands()", "default"),
        fact("export", "count() + 1", "default"),
        { "id": "runner", "page": "docs/page.md", "line": 1, "claim": "", "kind": "cases", "runner": "band" },
    ]);
    let want =
        json!({ "depth": ["default:BANDS"], "export": ["default:band_count"], "runner": ["default:BANDS"] });
    assert_eq!(named(&radio, attestations), want);
    let got = radio.call("evaluate", json!({ "facts": [fact("depth", "bands()", "default")] }));
    assert_eq!(got["depth"]["value"]["text"], "FM, AM", "{}", got["depth"]);
}

#[test]
fn a_fact_calls_a_docs_helper_by_its_name() {
    let radio = open("evaluate");
    let facts = json!([fact("names", "cap_names()", "caps"), fact("watts", "total_watts()", "caps")]);
    let got = radio.call("evaluate", json!({ "facts": facts }));
    assert_eq!(got["names"]["value"]["items"], json!(["scan", "record"]), "{}", got["names"]);
    assert_eq!(got["watts"]["value"]["text"], "7", "{}", got["watts"]);
    assert_eq!(radio.call("exports", json!({ "surface": "caps" })), json!(["MAX_WATTS", "supports"]));
}

#[test]
fn a_fact_calls_only_a_docs_helper_that_is_pub_in_a_library() {
    let radio = open("private");
    let facts = json!([fact("private", "hidden()", "caps"), fact("binary", "port()", "radiod")]);
    let got = radio.call("evaluate", json!({ "facts": facts }));
    let why = "a fact cannot call: a fact's program reaches a docs helper only when it is `pub`, in a module of a library";
    for (id, said) in [
        ("private", "hidden is a docs helper of surface \"caps\" that"),
        ("binary", "port is a docs helper of surface \"radiod\" that"),
    ] {
        let message = got[id]["message"].as_str().unwrap_or_default();
        assert!(message.contains(&format!("{said} {why}")), "{id}: {}", got[id]);
    }
}

#[test]
fn a_cargo_config_that_takes_the_place_of_build_rustflags_leaves_docs_helpers_out() {
    let config = "[target.'cfg(all())']\nrustflags = [\"-Cdebuginfo=0\"]\n";
    let radio = open_with("config", &[(".cargo/config.toml", config)]);
    let got = radio.call("evaluate", json!({ "facts": [fact("names", "cap_names()", "caps")] }));
    let why = "cap_names is a docs helper of surface \"caps\", and the build did not set `--cfg docattest`";
    assert!(got["names"]["message"].as_str().unwrap_or_default().contains(why), "{}", got["names"]);
}

#[test]
fn a_fact_attests_a_docs_helper_and_what_it_reaches() {
    let radio = open("symbols");
    let attestations = json!([fact("names", "cap_names()", "caps"), fact("watts", "total_watts()", "caps")]);
    let want = json!({
        "names": ["caps:cap_names", "caps:CAPS"],
        // `watts` is an ordinary function of the module: credited, and its body not followed.
        "watts": ["caps:total_watts", "caps:watts", "caps:MAX_WATTS"],
    });
    assert_eq!(named(&radio, attestations), want);
    let affected = |now: &str, was: &str| {
        project::affected(&radio.session, &radio.root, "src/caps.rs", now, was).join(" ")
    };
    assert_eq!(
        affected("watts: 5", "watts: 6"),
        "caps:supports caps:CAPS caps:cap_names caps:total_watts caps:hidden"
    );
    assert_eq!(affected("map(|cap| cap.name)", "map(|cap| cap.name.trim())"), "caps:cap_names");
}

const CONFIG: &str = r#"{ "pages": ["docs/*.md"], "sources": ["src/**/*.rs"], "surfaces": { "default": "radio", "caps": "radio::caps" }, "setup": "docs/docattest_setup.rs", "adapter": "rust" }"#;

const PAGE: &str = "\
The radio can: <!-- fact @caps: cap_names() -->
- `scan`
- `record`

It draws `7` watts at most. <!-- fact @caps: total_watts() -->

It receives `FM, AM`. <!-- fact: bands() -->

```rust check
assert_eq!(radio::caps::cap_names(), [\"scan\", \"record\"]);
```

```rust compile
use radio::caps::cap_names as names;
let all = names();
```
";

#[test]
fn a_page_states_what_only_helpers_say() {
    let radio = open_with("page", &[("docattest.config.json", CONFIG), ("docs/radio.md", PAGE)]);
    // RUSTFLAGS takes the place of cargo's config, so the cfg is added to it.
    let run = |args: &[&str]| project::docattest(&radio.root, args, &[("RUSTFLAGS", "-Cdebuginfo=0")]).0;
    let checked = run(&["check"]);
    assert!(checked.contains("results: 3 pass, 3 fail, 0 error"), "{checked}");
    // An example runs as the crate's users build it, without the docs helpers.
    let why = "a docs helper of surface \"caps\": a page's example runs as users build the code, without #[cfg(docattest)]";
    for refused in [
        format!("✗ L9 example: uses cap_names, {why} (block line 1)"),
        format!("✗ L13 example: uses cap_names, {why} (block line 1)"),
        format!("example: uses cap_names, {why} (block line 2)"),
    ] {
        assert!(checked.contains(&refused), "no {refused:?} in {checked}");
    }
    assert_eq!(run(&["uncovered"]), "band_count\n");
    assert_eq!(run(&["uncovered", "--surface", "caps"]), "every public symbol is attested\n");
    let found = run(&["where", "CAPS"]);
    assert_eq!(found, "docs/radio.md:1 fact  The radio can:\n");
}
