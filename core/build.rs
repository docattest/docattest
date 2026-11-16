//! Carries the shipped agent descriptors (`protocol/agents/*.json`) into the binary, so that
//! no agent is named in the core's sources and reading a descriptor opens no file.

use std::fmt::Write;
use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../protocol/agents");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("protocol/agents beside the core")
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.strip_suffix(".json").map(String::from))
        .filter(|name| name != "index")
        .collect();
    names.sort();
    let index = std::fs::read_to_string(dir.join("index.json")).expect("protocol/agents/index.json");
    let index: serde_json::Value = serde_json::from_str(&index).expect("index.json is JSON");
    let default = index["default"].as_str().expect("index.json names the default agent");
    assert!(names.iter().any(|name| name == default), "the default agent has no descriptor");
    let mut out = String::from("/// The shipped descriptors, by name, sorted.\n");
    out.push_str("pub const SHIPPED: &[(&str, &str)] = &[\n");
    for name in &names {
        let file = dir.join(format!("{name}.json"));
        writeln!(out, "    ({name:?}, include_str!({:?})),", file.display().to_string()).expect("a string");
    }
    out.push_str("];\n\n/// The agent meant when `--agent` is not given.\n");
    writeln!(out, "pub const DEFAULT: &str = {default:?};").expect("a string");
    let target = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("agents.rs");
    std::fs::write(target, out).expect("writing agents.rs");
}
