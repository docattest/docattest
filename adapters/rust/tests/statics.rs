//! What a session answers from syntax alone, on the sample project in `tests/fixture`.

mod project;

use docattest_rust::protocol::Session;
use project::{call, try_call};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixture");

fn open() -> Arc<dyn Session> {
    let surfaces = json!([
        { "name": "default", "entry": "shop" },
        { "name": "shipping", "entry": "shop::shipping" },
        { "name": "gone", "entry": "./src/nowhere.rs" },
    ]);
    project::open_session(Path::new(FIXTURE), surfaces, json!({ "setup": "docs/docattest_setup.rs" }))
}

/// The symbols an attestation names, as `surface:name` words.
fn named(session: &Arc<dyn Session>, attestation: Value) -> String {
    let mut attestation = attestation;
    attestation["id"] = json!("a");
    let found = call(session, "symbols", json!({ "attestations": [attestation] }));
    project::surface_names(&found["a"]).join(" ")
}

#[test]
fn symbols_are_what_an_attestation_names_of_a_surface() {
    let session = open();
    let cases = [
        (
            "a block names what it imports and what it reaches by path",
            json!({ "kind": "example", "page": "docs/p.md", "code": "use shop::{total, Money as Cash};\n\nassert!(total(&[], shop::DEFAULT_CURRENCY).is_ok());\nlet _ = shop::shipping::CARRIERS;\nlet _ = shop::private;" }),
            "default:total default:Money default:DEFAULT_CURRENCY shipping:CARRIERS",
        ),
        (
            "a glob import reaches exports by their bare names",
            json!({ "kind": "example", "page": "docs/p.md", "code": "use shop::shipping::*;\nassert!(quote(\"NL\").is_ok());" }),
            "shipping:quote",
        ),
        (
            "an example of a doc comment names what it documents and what it uses",
            json!({ "kind": "example", "page": "src/lib.rs", "owner": "total", "code": "let total = shop::total(&[shop::Money::eur(1000)], \"NL\");\n" }),
            "default:total default:Money",
        ),
        (
            "a fact names the exports its expression reads",
            json!({ "kind": "fact", "surface": "default", "expression": "TAX_RATES.len() as u32 + MAX_DISCOUNT + x.total" }),
            "default:TAX_RATES default:MAX_DISCOUNT default:total",
        ),
        (
            "a citation names what the cited test uses",
            json!({ "kind": "test", "file": "src/lib.rs", "title": "rejects_big_discounts", "describes": ["tests"] }),
            "default:apply_discount default:Money",
        ),
        (
            "a cases table names what its runner uses",
            json!({ "kind": "cases", "runner": "delivery" }),
            "shipping:quote",
        ),
        (
            "a surface that did not resolve has no symbols",
            json!({ "kind": "fact", "surface": "gone", "expression": "X" }),
            "",
        ),
    ];
    for (what, attestation, want) in cases {
        assert_eq!(named(&session, attestation), want, "{what}");
    }
}

#[test]
fn tests_are_listed_with_the_modules_around_them() {
    let session = open();
    let listing = call(&session, "tests", json!({ "file": "src/lib.rs" }));
    assert_eq!(
        listing["tests"],
        json!([
            { "title": "adds_tax", "describes": ["tests"], "line": 82 },
            { "title": "rejects_big_discounts", "describes": ["tests"], "line": 87 },
        ])
    );
    assert_eq!(
        listing["suites"],
        json!([{ "title": "tests", "describes": [], "line": 77, "kind": "a module" }])
    );
    let missing = call(&session, "tests", json!({ "file": "src/none.rs" }));
    assert_eq!(missing, json!({ "ok": false, "message": "src/none.rs does not exist" }));
}

#[test]
fn names_the_code_still_holds_are_told_from_those_it_does_not() {
    let session = open();
    let held = call(
        &session,
        "inCode",
        json!({ "names": ["GONE", "quote", "delivery", "round"], "files": ["docs/docattest_setup.rs"] }),
    );
    assert_eq!(held, json!(["quote", "delivery", "round"]));
}

#[test]
fn a_surface_offers_its_exports_that_are_values() {
    let session = open();
    let found = call(&session, "observables", json!({ "surface": "default" }));
    let read: Vec<(&str, &str)> = found
        .as_array()
        .unwrap()
        .iter()
        .map(|o| (o["expression"].as_str().unwrap(), o["value"]["text"].as_str().unwrap()))
        .collect();
    assert_eq!(read[..2], [("DEFAULT_CURRENCY", "EUR"), ("MAX_DISCOUNT", "50")]);
    // Then each setup helper that takes no arguments, called; `rates` takes some.
    assert_eq!(
        read.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        ["DEFAULT_CURRENCY", "MAX_DISCOUNT", "TAX_RATES", "ERROR_CODES", "error_messages()"]
    );
    assert_eq!(found[2]["value"]["items"], json!(["NL", "DE", "FR"]));
    assert_eq!(found[4]["value"]["items"].as_array().map(Vec::len), Some(3));
    let shipping = call(&session, "observables", json!({ "surface": "shipping" }));
    let called =
        shipping.as_array().unwrap().iter().filter(|o| o["expression"].as_str().unwrap().ends_with("()"));
    assert_eq!(called.count(), 0, "a setup helper is offered on the default surface only");
    let refused = try_call(&session, "observables", json!({ "surface": "gone" })).unwrap_err();
    assert!(refused.starts_with("docattest: surface \"gone\""), "{refused}");
}

#[test]
fn a_change_affects_the_exports_that_reach_it() {
    let session = open();
    let cases = [
        (
            "a helper reaches the exports that use it",
            "src/lib.rs",
            "(hundredths + 50) / 100",
            "hundredths / 100",
            "default:apply_discount default:total",
        ),
        (
            "a constant reaches what reads it",
            "src/lib.rs",
            "MAX_DISCOUNT: u32 = 50",
            "MAX_DISCOUNT: u32 = 40",
            "default:MAX_DISCOUNT default:apply_discount",
        ),
        (
            "what an impl block holds is part of its type",
            "src/lib.rs",
            "\"{}.{:02} {}\"",
            "\"{},{:02} {}\"",
            "default:Money default:apply_discount default:total",
        ),
        (
            "layout, comments and doc comments change nothing",
            "src/lib.rs",
            "/// Divides hundredths",
            "///   Divides    hundredths",
            "",
        ),
        (
            "a private constant of another module",
            "src/shipping.rs",
            "(\"DE\", 2)",
            "(\"DE\", 3)",
            "shipping:quote",
        ),
        (
            "an export that was not there at the base",
            "src/shipping.rs",
            "/// The number of days",
            "pub fn old() {}\n\n/// The number of days",
            "shipping:old",
        ),
    ];
    for (what, file, now, was, want) in cases {
        assert_eq!(project::affected(&session, Path::new(FIXTURE), file, now, was).join(" "), want, "{what}");
    }
    let new = call(
        &session,
        "affected",
        json!({ "base": { "src/shipping.rs": null }, "changed": ["src/shipping.rs"] }),
    );
    assert_eq!(new.as_array().unwrap().len(), 2, "a file the base did not have: {new}");
}
