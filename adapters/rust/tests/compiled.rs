//! What a session asks the compiler, because syntax cannot say it: the exports a macro
//! writes, and with them what a change to a macro call affects. Each test writes a small
//! project of its own to the temporary directory.

use docattest_rust::protocol::{Error, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// A project on disk, removed when the test is done with it.
struct Depot {
    root: PathBuf,
    session: Arc<dyn Session>,
}

impl Depot {
    /// Writes a project named for its test and opens a session on it, with its root as
    /// the surface `depot` and its `levels` module, when it has one, as `levels`.
    fn open(test: &str, files: &[(&str, &str)]) -> Depot {
        let root =
            std::env::temp_dir().join(format!("docattest-rust-compiled-{}-{test}", std::process::id()));
        for (rel, text) in files.iter().chain(&[("Cargo.toml", MANIFEST)]) {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let mut surfaces = vec![json!({ "name": "depot", "entry": "depot" })];
        if files.iter().any(|(rel, _)| *rel == "src/levels.rs") {
            surfaces.push(json!({ "name": "levels", "entry": "depot::levels" }));
        }
        let told = json!({
            "root": root, "surfaces": surfaces,
            "timeoutMs": 2000, "workers": 2, "network": false, "snapshots": { "data": {} }, "manifests": {}, "examples": [],
        });
        let session =
            docattest_rust::session::open(told).unwrap_or_else(|_| panic!("the session did not open"));
        Depot { root, session }
    }

    fn call(&self, method: &str, params: Value) -> Value {
        match self.session.call(method, params) {
            Some(Ok(value)) => value,
            Some(Err(Error::User(message) | Error::Internal(message))) => panic!("{method}: {message}"),
            None => panic!("no method {method}"),
        }
    }

    /// The exports a change affects, as sorted `surface:name` words, when a file held
    /// `held` at the base where it now holds `now_holds`.
    fn affected(&self, file: &str, now_holds: &str, held: &str) -> String {
        let now = std::fs::read_to_string(self.root.join(file)).unwrap();
        assert!(now.contains(now_holds), "{file} does not hold {now_holds:?}");
        let base = json!({ "base": { file: now.replacen(now_holds, held, 1) }, "changed": [file] });
        let found = self.call("affected", base);
        let mut words: Vec<String> = found
            .as_array()
            .unwrap()
            .iter()
            .map(|symbol| {
                format!("{}:{}", symbol["surface"].as_str().unwrap(), symbol["name"].as_str().unwrap())
            })
            .collect();
        words.sort_unstable();
        words.join(" ")
    }

    /// Where the compiler's listing of the project is built.
    fn listing(&self) -> PathBuf {
        self.root.join("target").join("docattest").join("doc")
    }
}

impl Drop for Depot {
    fn drop(&mut self) {
        self.session.close();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn stocked(test: &str) -> Depot {
    Depot::open(test, &[("src/lib.rs", LIB), ("src/levels.rs", LEVELS)])
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
    let plain = Depot::open("plain", &[("src/lib.rs", "pub fn count() -> u32 {\n    4\n}\n")]);
    assert_eq!(plain.call("exports", json!({ "surface": "depot" })), json!(["count"]));
    assert_eq!(plain.call("describeSurfaces", json!({}))["depot"]["warnings"], json!([]));
    assert!(!plain.listing().exists(), "the compiler was asked about a module its source declares");

    let stocked = stocked("asked");
    assert_eq!(
        stocked.call("exports", json!({ "surface": "depot" })),
        json!(["low", "space", "label", "fastens"])
    );
    assert!(!stocked.listing().exists(), "the compiler was asked before a module needed it");
    stocked.call("exports", json!({ "surface": "levels" }));
    assert!(Path::new(&stocked.listing()).is_dir());
}

#[test]
fn a_changed_macro_call_affects_what_it_writes_and_what_uses_that() {
    let depot = stocked("affected");
    assert_eq!(
        depot.affected("src/levels.rs", "REORDER_BOLTS = 40", "REORDER_BOLTS = 30"),
        "depot:fastens depot:low levels:Bolt levels:Kind levels:Nut levels:REORDER_BOLTS levels:REORDER_NUTS",
        "a macro may have written any name its module does not declare, so all of the module is taken to use it"
    );
    assert_eq!(
        depot.affected("src/levels.rs", "Nut,", "Nut,\n    Washer,"),
        "depot:fastens levels:Bolt levels:Kind levels:Nut"
    );
    assert_eq!(depot.affected("src/lib.rs", "to_uppercase", "to_lowercase"), "depot:label");
    assert_eq!(depot.affected("src/lib.rs", "saturating_sub", "wrapping_sub"), "depot:space");
}

#[test]
fn a_crate_the_compiler_refuses_keeps_what_its_source_declares() {
    let lib = "use nowhere::Missing;\nthread_local! { pub static SHELVES: u32 = 4; }\npub fn count() -> u32 { 4 }\n";
    let depot = Depot::open("refused", &[("src/lib.rs", lib)]);
    assert_eq!(depot.call("exports", json!({ "surface": "depot" })), json!(["count"]));
    let described = depot.call("describeSurfaces", json!({}));
    let warning = described["depot"]["warnings"][0].as_str().unwrap();
    assert!(warning.contains("the compiler could not list it"), "{warning}");
    assert!(warning.contains("nowhere"), "{warning}");
}
