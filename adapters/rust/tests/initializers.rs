//! What a value read attests: a constant a proof reads holds the values of the items its
//! initializer uses, so a fact over it, directly or through a helper, and a cases runner
//! that reads it attest those items too. Each test writes a launcher's crate, whose
//! constants are built from other constants, to the temporary directory.

mod project;

use project::Written;
use serde_json::{Value, json};

const MANIFEST: &str = "\
[package]
name = \"launch\"
version = \"0.1.0\"
edition = \"2021\"

[lints.rust]
unexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(docattest)'] }
";

/// Constants built from others, exported or not, at more than one depth, and a function
/// whose body is the only reader of `RETRIES`.
const LAUNCH: &str = "\
pub const COMMAND_ENV: &[&str] = &[COMMAND_PATH, SHELL];
pub const COMMAND_PATH: &str = \"/bin/run\";
const SHELL: &str = DEFAULT_SHELL;
const DEFAULT_SHELL: &str = \"/bin/sh\";

pub const RETRIES: u32 = 3;

pub fn attempts() -> u32 {
    RETRIES + 1
}

pub const LIMITS: [u32; 2] = [MIN, MAX];
pub const MIN: u32 = 1;
pub const MAX: u32 = 9;

pub const STEPS: &[u32] = &[STEP];
pub const STEP: u32 = 2;

pub const ALLOWED: &[&str] = &[ALLOWED_FIRST];
pub const ALLOWED_FIRST: &str = \"fs\";

const FORBIDDEN: &[&str] = &[FORBIDDEN_CAP];
const FORBIDDEN_CAP: &str = \"net\";

#[cfg(docattest)]
pub fn allowed() -> usize {
    ALLOWED.len()
}

#[cfg(docattest)]
pub fn forbidden() -> Vec<&'static str> {
    FORBIDDEN.to_vec()
}
";

/// A setup file whose fact function and whose runner each read one exported constant.
const SETUP: &str = "\
use launch::LIMITS;
use std::collections::HashMap;

type Row = HashMap<String, String>;

pub const CASES: &[(&str, fn(&Row) -> Result<(), String>)] = &[(\"step\", step)];

fn step(row: &Row) -> Result<(), String> {
    let step: u32 = row[\"step\"].parse().map_err(|_| \"not a number\".to_string())?;
    if launch::STEPS.contains(&step) { Ok(()) } else { Err(format!(\"no step {step}\")) }
}

pub fn limits() -> String {
    format!(\"{}..{}\", LIMITS[0], LIMITS[1])
}
";

const CONFIG: &str = r#"{ "pages": ["docs/*.md"], "sources": ["src/**/*.rs"], "surfaces": { "default": "launch" }, "setup": "docs/docattest_setup.rs", "adapter": "rust" }"#;

const PAGE: &str = "\
The command sees `2` variables. <!-- fact: COMMAND_ENV.len() -->

It tries `4` times. <!-- fact: attempts() -->

Its limits are `1..9`. <!-- fact: limits() -->

`1` capability is allowed. <!-- fact: allowed() -->

It refuses: <!-- fact: forbidden() -->
- `net`

<!-- cases: step -->
| step |
|------|
| 2    |
";

fn open(test: &str) -> Written {
    let files = [
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", LAUNCH),
        ("docs/docattest_setup.rs", SETUP),
        ("docattest.config.json", CONFIG),
        ("docs/launch.md", PAGE),
    ];
    let root = project::write(&format!("initializers-{test}"), &files);
    let surfaces = json!([{ "name": "default", "entry": "launch" }]);
    let session = project::open_session(&root, surfaces, json!({ "setup": "docs/docattest_setup.rs" }));
    Written { root, session }
}

fn fact(id: &str, expression: &str) -> Value {
    json!({ "id": id, "page": "docs/launch.md", "line": 1, "claim": "", "kind": "fact", "expression": expression, "surface": "default", "ordered": false })
}

#[test]
fn a_value_read_attests_what_its_initializer_uses() {
    let launch = open("symbols");
    let attestations = json!([
        fact("direct", "COMMAND_ENV.len()"),
        fact("function", "attempts()"),
        fact("setup", "limits()"),
        fact("helper", "allowed()"),
        fact("private", "forbidden()"),
        { "id": "runner", "page": "docs/launch.md", "line": 1, "claim": "", "kind": "cases", "runner": "step" },
    ]);
    let found = launch.call("symbols", json!({ "attestations": attestations }));
    let named = |id: &str| project::surface_names(&found[id]).join(" ");
    let want = [
        ("direct", "default:COMMAND_ENV default:COMMAND_PATH default:SHELL default:DEFAULT_SHELL"),
        ("function", "default:attempts"),
        ("setup", "default:LIMITS default:MIN default:MAX"),
        ("helper", "default:allowed default:ALLOWED default:ALLOWED_FIRST"),
        ("private", "default:forbidden default:FORBIDDEN default:FORBIDDEN_CAP"),
        ("runner", "default:STEPS default:STEP"),
    ];
    for (id, names) in want {
        assert_eq!(named(id), names, "{id}");
    }
    let affected = |now: &str, was: &str| {
        project::affected(&launch.session, &launch.root, "src/lib.rs", now, was).join(" ")
    };
    assert_eq!(
        affected("\"/bin/sh\"", "\"/bin/bash\""),
        "default:COMMAND_ENV default:SHELL default:DEFAULT_SHELL"
    );
}

#[test]
fn where_and_uncovered_follow_initializers() {
    let launch = open("index");
    let run = |args: &[&str]| project::docattest(&launch.root, args, &[]).0;
    assert_eq!(run(&["uncovered"]), "RETRIES\n");
    for (name, line) in [
        ("COMMAND_PATH", "docs/launch.md:1 fact  The command sees `2` variables."),
        ("DEFAULT_SHELL", "docs/launch.md:1 fact  The command sees `2` variables."),
        ("MAX", "docs/launch.md:5 fact  Its limits are `1..9`."),
        ("ALLOWED_FIRST", "docs/launch.md:7 fact  `1` capability is allowed."),
        ("FORBIDDEN_CAP", "docs/launch.md:9 fact  It refuses:"),
        ("STEP", "docs/launch.md:12 cases"),
    ] {
        let found = run(&["where", name]);
        assert!(found.starts_with(line), "where {name}: {found}");
    }
}
