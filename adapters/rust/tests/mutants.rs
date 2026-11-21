//! `mutants`, asked as the core asks it: which declaration each symbol is, and the edits to
//! the literals of its value, placed in the project's files. Each test writes a small
//! library to the temporary directory, or keeps it in memory.

mod project;

use project::Written;
use serde_json::{Value, json};
use std::path::Path;

const MANIFEST: &str = "[package]\nname = \"shop\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

const LIB: &str = "\
mod config;

pub use config::PORT;

/// Where the café is.
pub const NAME: &str = \"café\"; pub const OPEN: (bool, u8) = (true, 7);
pub const FALLBACK: u16 = config::PORT;
pub const EMPTY: Money = Money;
const LEVELS: &[u8] = &[1, 2, 3];

pub struct Money;

pub fn total() -> u16 {
    PORT
}
";

const CONFIG: &str = "pub const PORT: u16 = 8080;\n";

const FILES: [(&str, &str); 3] = [("Cargo.toml", MANIFEST), ("src/lib.rs", LIB), ("src/config.rs", CONFIG)];

fn open(test: &str) -> Written {
    Written::open(&format!("mutants-{test}"), &FILES, json!([{ "name": "default", "entry": "shop" }]))
}

fn of(name: &str) -> Value {
    json!({ "name": name, "surface": "default" })
}

/// Each edit of an answer as its description and the file it edits once applied.
fn applied(root: &Path, answer: &Value) -> Vec<(String, String)> {
    let edits = answer["edits"].as_array().unwrap();
    let apply = |edit: &Value| {
        let mut text = std::fs::read_to_string(root.join(edit["path"].as_str().unwrap())).unwrap();
        let range = edit["start"].as_u64().unwrap() as usize..edit["end"].as_u64().unwrap() as usize;
        text.replace_range(range, edit["replacement"].as_str().unwrap());
        (edit["description"].as_str().unwrap().to_string(), text)
    };
    edits.iter().map(apply).collect()
}

#[test]
fn a_reexported_constant_is_changed_where_it_is_declared() {
    let shop = open("reexported");
    let answer = shop.call("mutants", json!({ "symbols": [of("PORT")], "limit": 8 }));
    assert_eq!(
        answer,
        json!([{ "edits": [{ "path": "src/config.rs", "start": 22, "end": 26, "replacement": "8081", "description": "8080 → 8081" }] }])
    );
}

#[test]
fn edits_are_placed_in_bytes_and_cut_to_the_limit() {
    let shop = open("limit");
    let answer = shop.call("mutants", json!({ "symbols": [of("NAME"), of("OPEN")], "limit": 1 }));
    let lib = |was: &str, now: &str| LIB.replacen(was, now, 1);
    assert_eq!(
        applied(&shop.root, &answer[0]),
        [("\"café\" → \"caféx\"".to_string(), lib("\"café\"", "\"caféx\""))]
    );
    assert_eq!(applied(&shop.root, &answer[1]), [("true → false".to_string(), lib("true", "false"))]);
}

#[test]
fn a_constant_read_from_source_is_found_without_a_surface() {
    let shop = open("unexported");
    let answer = shop.call("mutants", json!({ "symbols": [{ "name": "LEVELS" }], "limit": 2 }));
    let descriptions: Vec<String> = applied(&shop.root, &answer[0]).into_iter().map(|(d, _)| d).collect();
    assert_eq!(descriptions, ["3 removed", "1 → 2"]);
}

#[test]
fn a_non_literal_scalar_constant_mutates_as_an_expression() {
    let shop = open("fallback");
    let answer = shop.call("mutants", json!({ "symbols": [of("FALLBACK")], "limit": 8 }));
    let descriptions: Vec<String> = applied(&shop.root, &answer[0]).into_iter().map(|(d, _)| d).collect();
    assert_eq!(descriptions, ["config::PORT → config::PORT + 1"]);
}

#[test]
fn a_symbol_without_edits_says_why() {
    let shop = open("why");
    let symbols = json!([of("total"), of("Money"), of("EMPTY"), of("NOWHERE"), { "name": "NOWHERE" }]);
    let answer = shop.call("mutants", json!({ "symbols": symbols, "limit": 8 }));
    let whys: Vec<&str> =
        answer.as_array().unwrap().iter().map(|entry| entry["why"].as_str().unwrap()).collect();
    assert_eq!(whys, ["function", "function", "noLiteral", "notFound", "notFound"]);
    assert!(answer.as_array().unwrap().iter().all(|entry| entry["edits"] == json!([])));
}

#[test]
fn a_project_in_memory_is_answered_as_one_on_disk() {
    let files: serde_json::Map<String, Value> =
        FILES.iter().map(|(path, text)| (path.to_string(), json!(text))).collect();
    let session = project::open_session(
        Path::new("/nowhere/shop"),
        json!([{ "name": "default", "entry": "shop" }]),
        json!({ "files": files }),
    );
    let answer = project::call(&session, "mutants", json!({ "symbols": [of("PORT")], "limit": 8 }));
    session.close();
    assert_eq!(answer[0]["edits"][0]["path"], "src/config.rs");
    assert_eq!(answer[0]["edits"][0]["description"], "8080 → 8081");
}
