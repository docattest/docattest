//! A workspace of two crates, as frameworks shape them: one declares a module of its own
//! name (a macro such as Anchor's `#[program]` writes `pub mod <crate> { … }`), the other
//! keeps a module private and re-exports from it. Each test writes it to the temporary
//! directory.

mod project;

use project::Written;
use serde_json::{Value, json};
use std::path::Path;

const WORKSPACE: &str = "[workspace]\nmembers = [\"vault\", \"jobs\"]\nresolver = \"2\"\n";

const VAULT_MANIFEST: &str =
    "[package]\nname = \"acme-vault\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\nmainnet = []\n";

const VAULT: &str = "\
pub struct Vault {
    pub balance: u64,
}

impl Vault {
    pub const SIZE: usize = 8;
}

#[cfg(feature = \"mainnet\")]
pub const NETWORK: &str = \"mainnet\";
#[cfg(not(feature = \"mainnet\"))]
pub const NETWORK: &str = \"devnet\";

pub mod acme_vault {
    pub const FEE: u64 = 5;
}
";

const JOBS_MANIFEST: &str = "[package]\nname = \"acme-jobs\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

const JOBS: &str = "mod types;\n\npub use types::RETRY;\n";

const JOB_TYPES: &str = "pub const RETRY: u32 = 3;\n";

const FILES: [(&str, &str); 6] = [
    ("Cargo.toml", WORKSPACE),
    ("vault/Cargo.toml", VAULT_MANIFEST),
    ("vault/src/lib.rs", VAULT),
    ("jobs/Cargo.toml", JOBS_MANIFEST),
    ("jobs/src/lib.rs", JOBS),
    ("jobs/src/types.rs", JOB_TYPES),
];

/// Opens a session on the workspace with its surfaces: `default` the jobs crate, `vault`
/// the vault crate (and `rooted`, the same written from the root), `types` the jobs
/// crate's private module, and `misnamed` the vault crate imported by a wrong path.
fn open(test: &str) -> Written {
    let surfaces = json!([
        { "name": "default", "entry": "acme_jobs" },
        { "name": "vault", "entry": "acme_vault" },
        { "name": "types", "entry": "acme_jobs::types" },
        { "name": "rooted", "entry": "::acme_vault" },
        { "name": "misnamed", "entry": { "import": "acme_vault::nowhere", "path": "vault/src/lib.rs" } },
    ]);
    Written::open(&format!("workspace-{test}"), &FILES, surfaces)
}

fn fact(id: &str, expression: &str, surface: &str) -> Value {
    json!({ "id": id, "page": "docs/page.md", "line": 1, "claim": "", "kind": "fact", "expression": expression, "surface": surface, "ordered": false })
}

#[test]
fn a_crate_that_declares_a_module_of_its_own_name_is_a_surface() {
    let workspace = open("own-name");
    let facts = json!([
        fact("export", "NETWORK", "vault"),
        fact("module", "acme_vault::FEE", "vault"),
        fact("crate", "::acme_vault::Vault::SIZE", "vault"),
    ]);
    let got = workspace.call("evaluate", json!({ "facts": facts }));
    for (id, text) in [("export", "devnet"), ("module", "5"), ("crate", "8")] {
        assert_eq!(got[id]["value"]["text"], json!(text), "{id}: {}", got[id]);
    }
    let offered = workspace.call("observables", json!({ "surface": "vault" }));
    assert_eq!(offered[0]["expression"], "NETWORK", "{offered}");
    let rooted = workspace.call("describeSurfaces", json!({}))["rooted"]["resolution"]["import"].clone();
    assert_eq!(rooted, "acme_vault");
}

#[test]
fn a_surface_that_does_not_load_says_where_the_program_that_imports_it_is() {
    let workspace = open("misnamed");
    let got = workspace.call("evaluate", json!({ "facts": [fact("lost", "NETWORK", "misnamed")] }));
    let message = got["lost"]["message"].as_str().unwrap();
    let at = "surface \"misnamed\" failed to load: target/docattest/crate/src/bin/p_";
    assert!(message.starts_with(at) && message.contains("unresolved import"), "{message}");
}

#[test]
fn a_private_module_is_no_surface_and_says_why() {
    let workspace = open("private");
    let why = "\"acme_jobs::types\" cannot be imported from outside its crate: module `types` is private";
    let warnings = &workspace.call("describeSurfaces", json!({}))["types"]["warnings"];
    assert!(warnings[0].as_str().unwrap().contains(why), "{warnings}");
    assert_eq!(workspace.call("exports", json!({ "surface": "types" })), Value::Null);
    let got = workspace.call("evaluate", json!({ "facts": [fact("hidden", "RETRY", "types")] }));
    assert!(got["hidden"]["message"].as_str().unwrap().contains(why), "{}", got["hidden"]);
    assert_eq!(workspace.call("exports", json!({ "surface": "default" })), json!(["RETRY"]));
}

#[test]
fn a_fact_names_the_surface_exports_its_crate_paths_reach() {
    let workspace = open("paths");
    let attested = json!([fact("path", "acme_vault::Vault::SIZE", "default")]);
    let found = workspace.call("symbols", json!({ "attestations": attested }));
    let named = json!([{ "name": "Vault", "surface": "vault" }, { "name": "Vault", "surface": "rooted" }]);
    assert_eq!(found["path"], named);
}

/// What `docattest check` says of a page of the workspace that reads the vault's network,
/// with the generated crate built with some features.
fn checked_with(root: &Path, features: &str) -> String {
    project::docattest(root, &["check"], &[("DOCATTEST_CARGO_FEATURES", features)]).0
}

#[test]
fn the_generated_crate_is_built_with_the_features_it_is_given() {
    let config = r#"{ "pages": ["docs/*.md"], "surfaces": { "default": "acme_vault" }, "adapter": "rust" }"#;
    let page = "The network is `mainnet`. <!-- fact: NETWORK -->\n";
    let files: Vec<(&str, &str)> =
        FILES.iter().copied().chain([("docattest.config.json", config), ("docs/network.md", page)]).collect();
    let root = project::write("workspace-features", &files);
    let (default, chosen) = (checked_with(&root, ""), checked_with(&root, "acme-vault/mainnet"));
    let _ = std::fs::remove_dir_all(&root);
    assert!(default.contains("expected mainnet, got devnet"), "{default}");
    assert!(chosen.contains("NETWORK is mainnet"), "{chosen}");
}
