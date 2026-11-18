//! Constants read from source: those a library's module does not export, and those of a
//! binary crate, which nothing imports. Each test writes a workspace of a library and a
//! service's binary, which does not compile, to the temporary directory.

mod project;

use project::Written;
use serde_json::{Value, json};

const WORKSPACE: &str = "[workspace]\nmembers = [\"shop\", \"shopd\"]\nresolver = \"2\"\n";

const SHOP_MANIFEST: &str = "[package]\nname = \"shop\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

const SHOP: &str = "\
use std::time::Duration;

pub const MAX_DISCOUNT: u32 = 50;
const BASE_PORT: u16 = 8000;
const PORT: u16 = BASE_PORT + 80;
const TIMEOUT: Duration = Duration::from_secs(30);
static REGION: &str = \"nl\";
const DAYS: [(&str, u32); 2] = [(\"NL\", 1), (\"DE\", 2)];
const SHARE: f64 = 1.0 / 4.0;

pub struct Level(pub u8);
const LEVEL: Level = Level(2);
const NEXT_LEVEL: u8 = LEVEL.0 + 1;

fn round(hundredths: i64) -> i64 {
    (hundredths + 50) / 100
}

pub fn days(region: &str) -> u32 {
    DAYS.iter().find(|(code, _)| *code == region).map_or(0, |(_, days)| *days)
}
";

const SHOPD_MANIFEST: &str = "[package]\nname = \"shopd\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

/// A binary whose code does not compile: its constants are read all the same.
const SHOPD: &str = "\
const LISTEN: &str = \"0.0.0.0:8080\";
const WORKERS: usize = 4;

fn workers() -> usize {
    WORKERS
}

fn main() {
    let broken: u32 = \"not a number\";
}
";

fn open(test: &str) -> Written {
    let files = [
        ("Cargo.toml", WORKSPACE),
        ("shop/Cargo.toml", SHOP_MANIFEST),
        ("shop/src/lib.rs", SHOP),
        ("shopd/Cargo.toml", SHOPD_MANIFEST),
        ("shopd/src/main.rs", SHOPD),
    ];
    let surfaces = json!([
        { "name": "default", "entry": "shop" },
        { "name": "shopd", "entry": "./shopd" },
        { "name": "main", "entry": "./shopd/src/main.rs" },
    ]);
    Written::open(&format!("constants-{test}"), &files, surfaces)
}

fn fact(id: &str, expression: &str, surface: &str) -> Value {
    json!({ "id": id, "page": "docs/page.md", "line": 1, "claim": "", "kind": "fact", "expression": expression, "surface": surface, "ordered": false })
}

#[test]
fn a_fact_reads_constants_from_source() {
    let workspace = open("read");
    let facts = json!([
        fact("exported", "MAX_DISCOUNT", "default"),
        fact("sum", "PORT", "default"),
        fact("duration", "TIMEOUT", "default"),
        fact("seconds", "TIMEOUT.as_secs()", "default"),
        fact("static", "REGION.to_uppercase()", "default"),
        fact("pairs", "DAYS", "default"),
        fact("float", "SHARE", "default"),
        fact("both", "PORT as u32 + MAX_DISCOUNT", "default"),
        fact("binary", "LISTEN", "shopd"),
        fact("module", "WORKERS * 2", "main"),
    ]);
    let got = workspace.call("evaluate", json!({ "facts": facts }));
    let texts = [
        ("exported", "50"),
        ("sum", "8080"),
        ("duration", "30s"),
        ("seconds", "30"),
        ("static", "NL"),
        ("float", "0.25"),
        ("both", "8130"),
        ("binary", "0.0.0.0:8080"),
        ("module", "8"),
    ];
    for (id, text) in texts {
        assert_eq!(got[id]["value"]["text"], json!(text), "{id}: {}", got[id]);
    }
    assert_eq!(got["pairs"]["value"]["items"], json!(["NL", "DE"]));
    let described = workspace.call("describeSurfaces", json!({}));
    assert_eq!(
        described["shopd"]["resolution"],
        json!({ "import": "shopd", "entry": "shopd/src/main.rs", "package": { "name": "shopd", "display": "shopd" } })
    );
    assert_eq!(workspace.call("exports", json!({ "surface": "shopd" })), json!([]));
}

#[test]
fn only_constants_are_read_from_source() {
    let workspace = open("refused");
    let facts = json!([
        fact("function", "round(50)", "default"),
        fact("typed", "LEVEL.0", "default"),
        fact("reads", "NEXT_LEVEL", "default"),
        fact("binary", "workers()", "shopd"),
        fact("nothing", "NOWHERE", "default"),
    ]);
    let got = workspace.call("evaluate", json!({ "facts": facts }));
    let messages = [
        (
            "function",
            "fact `round(50)`: round is a function that surface \"default\" does not export; of its unexported names, a fact reads only constants",
        ),
        (
            "typed",
            "fact `LEVEL.0`: LEVEL is a constant that surface \"default\" does not export, and it cannot be read from source: its type `Level` is not made of primitive types and Duration",
        ),
        (
            "reads",
            "NEXT_LEVEL is a constant that surface \"default\" does not export, and it cannot be read from source: it reads LEVEL, and its type `Level`",
        ),
        (
            "binary",
            "workers is a function that surface \"shopd\" does not export; of its unexported names, a fact reads only constants",
        ),
        (
            "nothing",
            "NOWHERE is not exported by surface \"default\" (cannot find value `NOWHERE` in this scope)",
        ),
    ];
    for (id, message) in messages {
        let said = got[id]["message"].as_str().unwrap_or_default();
        assert!(got[id]["ok"] == false && said.contains(message), "{id}: {}", got[id]);
    }
}

#[test]
fn a_fact_attests_the_constants_it_reads() {
    let workspace = open("symbols");
    let attestations = json!([
        fact("lib", "PORT as u32 + MAX_DISCOUNT + round(1) as u32", "default"),
        fact("binary", "LISTEN", "shopd"),
    ]);
    let found = workspace.call("symbols", json!({ "attestations": attestations }));
    assert_eq!(project::surface_names(&found["lib"]), ["default:PORT", "default:MAX_DISCOUNT"]);
    assert_eq!(project::surface_names(&found["binary"]), ["shopd:LISTEN"]);
    let affected = |file: &str, now: &str, was: &str| {
        project::affected(&workspace.session, &workspace.root, file, now, was).join(" ")
    };
    assert_eq!(
        affected("shop/src/lib.rs", "BASE_PORT: u16 = 8000", "BASE_PORT: u16 = 9000"),
        "default:BASE_PORT default:PORT"
    );
    assert_eq!(affected("shop/src/lib.rs", "(\"DE\", 2)", "(\"DE\", 3)"), "default:days default:DAYS");
    assert_eq!(affected("shopd/src/main.rs", "0.0.0.0:8080", "0.0.0.0:80"), "shopd:LISTEN main:LISTEN");
}
