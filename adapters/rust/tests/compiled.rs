//! What a session asks the compiler, because syntax cannot say it: the exports a macro
//! writes, and with them what a change to a macro call affects. Each test writes a small
//! project of its own to the temporary directory.

mod project;

use project::Written;
use serde_json::json;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "[package]\nname = \"depot\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

/// The root of a library whose `levels` module a macro writes most of.
const LIB: &str = "\
pub mod levels;

mod counting {
    pub fn room_left(held: u32, room: u32) -> u32 {
        room.saturating_sub(held)
    }
}

use counting::*;

pub fn low(held: u32) -> bool {
    held < levels::REORDER_BOLTS
}

pub fn space(held: u32) -> u32 {
    room_left(held, 100)
}

pub fn label(item: &str) -> String {
    item.to_uppercase()
}

pub fn fastens(kind: &levels::Kind) -> bool {
    use levels::*;
    matches!(kind, Bolt)
}
";

const LEVELS: &str = "\
macro_rules! reorder_levels {
    ($($name:ident = $level:expr),*) => {
        $(pub const $name: u32 = $level;)*
    };
}

reorder_levels!(REORDER_BOLTS = 40, REORDER_NUTS = 25);

pub enum Kind {
    Bolt,
    Nut,
}

pub use Kind::*;
";

/// Writes a project named for its test and opens a session on it, with its root as the
/// surface `depot` and its `levels` module, when it has one, as `levels`.
fn open_depot(test: &str, files: &[(&str, &str)]) -> Written {
    let mut surfaces = vec![json!({ "name": "depot", "entry": "depot" })];
    if files.iter().any(|(rel, _)| *rel == "src/levels.rs") {
        surfaces.push(json!({ "name": "levels", "entry": "depot::levels" }));
    }
    let files: Vec<(&str, &str)> = files.iter().copied().chain([("Cargo.toml", MANIFEST)]).collect();
    Written::open(&format!("compiled-{test}"), &files, json!(surfaces))
}

/// The exports a change affects, as sorted `surface:name` words, when a file held `held`
/// at the base where it now holds `now_holds`.
fn affected(depot: &Written, file: &str, now_holds: &str, held: &str) -> String {
    let mut words = project::affected(&depot.session, &depot.root, file, now_holds, held);
    words.sort_unstable();
    words.join(" ")
}

/// Where the compiler's listing of a project is built.
fn listing(depot: &Written) -> PathBuf {
    depot.root.join("target").join("docattest").join("doc")
}

fn stocked(test: &str) -> Written {
    open_depot(test, &[("src/lib.rs", LIB), ("src/levels.rs", LEVELS)])
}

#[test]
fn exports_a_macro_writes_are_listed() {
    let depot = stocked("listed");
    assert_eq!(
        depot.call("exports", json!({ "surface": "levels" })),
        json!(["REORDER_BOLTS", "REORDER_NUTS", "Kind", "Bolt", "Nut"])
    );
    assert_eq!(depot.call("describeSurfaces", json!({}))["levels"]["warnings"], json!([]));
    let fact = json!({ "id": "f", "page": "docs/p.md", "kind": "fact", "expression": "REORDER_BOLTS", "surface": "levels" });
    let found = depot.call("symbols", json!({ "attestations": [fact] }));
    assert_eq!(found["f"], json!([{ "name": "REORDER_BOLTS", "surface": "levels" }]));
}

#[test]
fn the_compiler_is_asked_only_about_a_module_syntax_cannot_list() {
    let plain = open_depot("plain", &[("src/lib.rs", "pub fn count() -> u32 {\n    4\n}\n")]);
    assert_eq!(plain.call("exports", json!({ "surface": "depot" })), json!(["count"]));
    assert_eq!(plain.call("describeSurfaces", json!({}))["depot"]["warnings"], json!([]));
    assert!(!listing(&plain).exists(), "the compiler was asked about a module its source declares");

    let stocked = stocked("asked");
    assert_eq!(
        stocked.call("exports", json!({ "surface": "depot" })),
        json!(["low", "space", "label", "fastens"])
    );
    assert!(!listing(&stocked).exists(), "the compiler was asked before a module needed it");
    stocked.call("exports", json!({ "surface": "levels" }));
    assert!(Path::new(&listing(&stocked)).is_dir());
}

#[test]
fn a_changed_macro_call_affects_what_it_writes_and_what_uses_that() {
    let depot = stocked("affected");
    assert_eq!(
        affected(&depot, "src/levels.rs", "REORDER_BOLTS = 40", "REORDER_BOLTS = 30"),
        "depot:fastens depot:low levels:Bolt levels:Kind levels:Nut levels:REORDER_BOLTS levels:REORDER_NUTS",
        "a macro may have written any name its module does not declare, so all of the module is taken to use it"
    );
    assert_eq!(
        affected(&depot, "src/levels.rs", "Nut,", "Nut,\n    Washer,"),
        "depot:fastens levels:Bolt levels:Kind levels:Nut"
    );
    assert_eq!(affected(&depot, "src/lib.rs", "to_uppercase", "to_lowercase"), "depot:label");
    assert_eq!(affected(&depot, "src/lib.rs", "saturating_sub", "wrapping_sub"), "depot:space");
}

#[test]
fn a_crate_the_compiler_refuses_keeps_what_its_source_declares() {
    let lib = "use nowhere::Missing;\nthread_local! { pub static SHELVES: u32 = 4; }\npub fn count() -> u32 { 4 }\n";
    let depot = open_depot("refused", &[("src/lib.rs", lib)]);
    assert_eq!(depot.call("exports", json!({ "surface": "depot" })), json!(["count"]));
    let described = depot.call("describeSurfaces", json!({}));
    let warning = described["depot"]["warnings"][0].as_str().unwrap();
    assert!(warning.contains("the compiler could not list it"), "{warning}");
    assert!(warning.contains("nowhere"), "{warning}");
}
