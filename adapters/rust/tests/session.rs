//! A session, driven as the core drives it, on the sample project in `tests/fixture`:
//! real code, compiled and run with this machine's cargo.

mod project;

use docattest_rust::protocol::Session;
use project::call;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixture");

/// Opens a session on the fixture, told of these examples.
fn open(examples: &[Value]) -> Arc<dyn Session> {
    let surfaces = json!([
        { "name": "default", "entry": "shop" },
        { "name": "shipping", "entry": "./src/shipping.rs" },
        { "name": "gone", "entry": "shop::no_such_module" },
    ]);
    let told = json!({
        "setup": "docs/docattest_setup.rs",
        "workers": 4,
        "snapshots": { "dir": "docs/.snapshots", "data": { "regions": ["NL", "DE"] } },
        "manifests": { "Cargo.toml": { "package": { "name": "shop", "rust-version": "1.80" } } },
        "texts": { "Dockerfile": "FROM rust:1.80\nUSER 10001\n" },
        "matches": { "Dockerfile": { "^USER (\\d+)$": "10001" } },
        "manifestFields": { "ci.yml": { "deploy.variables.VERSION": "0.2.2", "deploy.replicas": 3 } },
        "examples": examples,
    });
    project::open_session(Path::new(FIXTURE), surfaces, told)
}

fn block(id: &str, code: &str) -> Value {
    json!({ "id": id, "page": "docs/page.md", "line": 10, "claim": "", "kind": "example", "lang": "rust", "mode": "check", "network": false, "code": code, "weak": false })
}

fn fact(id: &str, expression: &str, surface: &str) -> Value {
    json!({ "id": id, "page": "docs/page.md", "line": 1, "claim": "", "kind": "fact", "expression": expression, "surface": surface, "ordered": false })
}

#[test]
fn surfaces_are_modules_and_export_their_public_names() {
    let session = open(&[]);
    assert_eq!(
        call(&session, "exports", json!({ "surface": "default" })),
        json!([
            "DEFAULT_CURRENCY",
            "MAX_DISCOUNT",
            "TAX_RATES",
            "ERROR_CODES",
            "Money",
            "apply_discount",
            "total"
        ])
    );
    assert_eq!(call(&session, "exports", json!({ "surface": "shipping" })), json!(["CARRIERS", "quote"]));
    assert_eq!(call(&session, "exports", json!({ "surface": "gone" })), Value::Null);
    let described = call(&session, "describeSurfaces", json!({}));
    assert_eq!(
        described["shipping"]["resolution"],
        json!({ "import": "shop::shipping", "entry": "src/shipping.rs", "package": { "name": "shop", "display": "shop" } })
    );
    assert!(
        described["gone"]["warnings"][0]
            .as_str()
            .unwrap()
            .contains("\"shop::no_such_module\" did not resolve")
    );
}

#[test]
fn blocks_compile_and_run_each_as_a_program_of_its_own() {
    let mut examples = vec![
        block(
            "passes",
            "use shop::{apply_discount, Money};\n\nassert_eq!(apply_discount(Money::eur(1000), 25).unwrap().to_string(), \"7.50 EUR\");",
        ),
        block("differs", "assert_eq!(shop::MAX_DISCOUNT, 60);"),
        block("panics", "let empty: Vec<u8> = Vec::new();\nlet first = empty[0];"),
        block("broken", "use shop::Money;\n\nlet price = shop::aply_discount(Money::eur(1), 1);"),
        block("weak", "println!(\"{}\", shop::Money::eur(5));"),
        block("slow", "loop {}"),
        block("exits", "std::process::exit(3);"),
        block("whole", "fn main() {\n    assert!(shop::MAX_DISCOUNT > 1);\n}"),
    ];
    let mut panics = block("panics as said", "shop::shipping::quote(\"XX\").unwrap();");
    panics["tags"] = json!(["should_panic"]);
    let mut network = block("network", "assert!(true);");
    network["network"] = json!(true);
    examples.extend([panics, network]);
    let session = open(&examples);
    let diagnostics = call(&session, "compile", json!({ "examples": examples }));
    assert_eq!(
        diagnostics,
        json!([{ "id": "broken", "message": "cannot find function `aply_discount` in crate `shop`", "line": 3 }])
    );
    let results = call(&session, "run", json!({ "examples": examples, "cases": [] }));
    let want = [
        ("pass", "all assertions held"),
        ("fail", "panicked: assertion `left == right` failed: expected 60, got 50 (block line 1)"),
        ("fail", "(block line 2)"),
        ("fail", "does not compile: cannot find function `aply_discount` in crate `shop` (block line 3)"),
        ("pass", "ran to its end, but asserts nothing (weak)"),
        ("fail", "timed out after 2000 ms"),
        ("fail", "exited with status 3"),
        ("pass", "all assertions held"),
        ("pass", "panicked, as it says it does"),
        ("skipped", "network example skipped"),
    ];
    for (k, (status, message)) in want.iter().enumerate() {
        let got = &results[k];
        assert!(
            got["status"] == *status && got["message"].as_str().unwrap().contains(message),
            "{}: {got}",
            examples[k]["id"]
        );
    }
    assert_eq!(
        (&results[1]["expected"], &results[1]["actual"], &results[0]["line"]),
        (&json!("60"), &json!("50"), &json!(10))
    );
}

/// A block whose program takes 2500 ms to reach its `main`: a constructor that the system
/// runs as it loads the program sleeps, as a loaded machine's start-up of a freshly linked
/// program takes its time.
const SLOW_START: &str = r#"
#[used]
#[cfg_attr(target_os = "macos", unsafe(link_section = "__DATA,__mod_init_func"))]
#[cfg_attr(target_os = "linux", unsafe(link_section = ".init_array"))]
static SLOW_START: extern "C" fn() = {
    extern "C" fn slow_start() {
        std::thread::sleep(std::time::Duration::from_millis(2500));
    }
    slow_start
};

fn main() {
    assert!(true);
}
"#;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn a_blocks_time_counts_from_when_its_code_starts() {
    let examples = [
        block("slow start", SLOW_START),
        block("sleeps", "std::thread::sleep(std::time::Duration::from_millis(2500));\nassert!(true);"),
    ];
    let session = open(&examples);
    let results = call(&session, "run", json!({ "examples": examples, "cases": [] }));
    let got: Vec<(&str, &str)> = results
        .as_array()
        .unwrap()
        .iter()
        .map(|result| (result["status"].as_str().unwrap(), result["message"].as_str().unwrap()))
        .collect();
    assert_eq!(got, [("pass", "all assertions held"), ("fail", "timed out after 2000 ms")]);
}

#[test]
fn facts_show_values_as_rust_prints_them() {
    let session = open(&[]);
    let facts = json!([
        fact("scalar", "MAX_DISCOUNT", "default"),
        fact("text", "DEFAULT_CURRENCY", "default"),
        fact("keys", "TAX_RATES", "default"),
        fact("call", "rates(&[\"NL\"])", "default"),
        fact("standard", "DEFAULT_CURRENCY.to_lowercase()", "default"),
        fact("snapshot", "snapshot(\"regions\")", "default"),
        fact("manifest", "manifest(\"Cargo.toml\")[\"package\"][\"rust-version\"]", "default"),
        fact(
            "file",
            "file(\"Dockerfile\").lines().find_map(|l| l.strip_prefix(\"USER \")).unwrap()",
            "default"
        ),
        fact("plain", "1 + 1", "default"),
        fact("elsewhere", "CARRIERS", "shipping"),
        fact("matched", "matched(\"Dockerfile\", r\"^USER (\\d+)$\")", "default"),
        fact("field", "manifestField(\"ci.yml\", \"deploy.variables.VERSION\")", "default"),
        fact("number", "manifestField(\"ci.yml\", \"deploy.replicas\")", "default"),
    ]);
    let got = call(&session, "evaluate", json!({ "facts": facts }));
    let texts = [
        ("scalar", "50"),
        ("text", "EUR"),
        ("call", "[\"21%\"]"),
        ("standard", "eur"),
        ("plain", "2"),
        ("snapshot", "[\"NL\",\"DE\"]"),
        ("manifest", "1.80"),
        ("file", "10001"),
        ("matched", "10001"),
        ("field", "0.2.2"),
        ("number", "3"),
    ];
    for (id, text) in texts {
        assert_eq!(
            (&got[id]["ok"], &got[id]["value"]["text"]),
            (&json!(true), &json!(text)),
            "{id}: {}",
            got[id]
        );
    }
    assert_eq!(got["keys"]["value"]["items"], json!(["NL", "DE", "FR"]));
    assert_eq!(got["elsewhere"]["value"]["items"], json!(["post", "courier", "drone"]));
    assert_eq!((&got["text"]["value"]["kind"], got["scalar"]["value"].get("kind")), (&json!("text"), None));
    assert_eq!((&got["field"]["value"]["kind"], got["number"]["value"].get("kind")), (&json!("text"), None));
}

#[test]
fn a_fact_without_a_value_says_why_and_costs_the_others_nothing() {
    let session = open(&[]);
    let facts = json!([
        fact("missing", "snapshot(\"none\")", "default"),
        fact("unread", "manifest(\"Cargo.lock\")", "default"),
        fact("unread_file", "file(\"Dockerfile.dev\")", "default"),
        fact("unread_match", "matched(\"Dockerfile\", \"USER\")", "default"),
        fact("unread_field", "manifestField(\"ci.yml\", \"deploy.image\")", "default"),
        fact("private", "round(50)", "default"),
        fact("syntax", "MAX_DISCOUNT +", "default"),
        fact("panics", "TAX_RATES[7]", "default"),
        fact("typed", "MAX_DISCOUNT + \"x\"", "default"),
        fact("fine", "MAX_DISCOUNT", "default"),
        fact("undeclared", "nowhere(1)", "default"),
        fact("unknown", "X", "nowhere"),
        fact("unresolved", "X", "gone"),
    ]);
    let got = call(&session, "evaluate", json!({ "facts": facts }));
    let messages = [
        ("missing", "fact `snapshot(\"none\")` panicked: snapshot \"none\" not found in docs/.snapshots"),
        (
            "unread",
            "panicked: manifest \"Cargo.lock\" was not read: name it in the fact itself, as manifest(\"Cargo.lock\")",
        ),
        (
            "unread_file",
            "panicked: file \"Dockerfile.dev\" was not read: name it in the fact itself, as file(\"Dockerfile.dev\")",
        ),
        (
            "unread_match",
            "panicked: matched(\"Dockerfile\", \"USER\") was not read: write both arguments out in the fact itself, as string literals",
        ),
        ("unread_field", "panicked: manifestField(\"ci.yml\", \"deploy.image\") was not read"),
        (
            "private",
            "fact `round(50)`: round is a function that surface \"default\" does not export; of its unexported names, a fact reads only constants",
        ),
        (
            "undeclared",
            "fact `nowhere(1)`: nowhere is not exported by surface \"default\" (cannot find function `nowhere` in this scope)",
        ),
        ("syntax", "cannot parse fact expression `MAX_DISCOUNT +`"),
        ("typed", "fact `MAX_DISCOUNT + \"x\"`: "),
        ("unknown", "unknown surface \"nowhere\" (configured: default, shipping, gone)"),
        ("unresolved", "surface \"gone\" (shop::no_such_module) not found"),
    ];
    for (id, message) in messages {
        assert!(
            got[id]["ok"] == false && got[id]["message"].as_str().unwrap().contains(message),
            "{id}: {}",
            got[id]
        );
    }
    assert_eq!(got["panics"]["ok"], false, "{}", got["panics"]);
    assert_eq!(got["fine"]["value"]["text"], "50");
}

#[test]
fn a_cases_table_gives_a_result_for_each_row() {
    let session = open(&[]);
    let table = |runner: &str, header: &[&str], rows: Value| {
        let rows: Vec<Value> = rows
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(k, values)| json!({ "line": 22 + k, "name": values["case"], "values": values }))
            .collect();
        json!({ "id": runner, "page": "docs/page.md", "line": 20, "claim": "", "kind": "cases", "runner": runner, "header": header, "rows": rows })
    };
    let discount = ["case", "price", "percent", "result"];
    let tables = json!([
        table(
            "discount",
            &discount,
            json!([
                { "case": "a quarter", "price": "1000", "percent": "25", "result": "750" },
                { "case": "wrong", "price": "1000", "percent": "25", "result": "800" },
                { "case": "too much", "price": "1000", "percent": "90", "result": "100" },
            ])
        ),
        table("nope", &["case"], json!([{ "case": "a" }])),
        table("discount", &["case", "price"], json!([{ "case": "a" }])),
        table("discount", &discount, json!([])),
    ]);
    let results = call(&session, "run", json!({ "examples": [], "cases": tables }));
    let got: Vec<String> = results
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            format!("{}|{}|{}", r["status"].as_str().unwrap(), r["line"], r["message"].as_str().unwrap())
        })
        .collect();
    let want = [
        "pass|22|docs/page.md:22 a quarter",
        "fail|23|docs/page.md:23 wrong: expected 800, got 750",
        "fail|24|docs/page.md:24 too much: The discount is outside the allowed range",
        "error|20|cases runner \"nope\" at L20 is not in CASES of docs/docattest_setup.rs (known: delivery, discount)",
        "error|20|cases \"discount\" at L20: the table is missing columns \"percent\", \"result\" declared by the runner",
    ];
    assert_eq!(got, want);
}
