//! The core compiles in its own copy of the protocol's files (`core/protocol/`), because a
//! crate published to crates.io carries only what is inside its own directory. The copy is
//! written by `node protocol/sync.ts core/protocol` and must be the source, byte for byte.

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn the_copy_is_the_source() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = core.join("../protocol");
    if !source.join("core.json").is_file() {
        // A packaged crate has no repository around it to compare with.
        return;
    }
    let copy = core.join("protocol");
    let (mine, theirs) = (files(&copy, &copy), files(&source, &source));
    let missing: Vec<&PathBuf> = theirs.iter().filter(|file| !mine.contains(file)).collect();
    assert!(
        missing.is_empty(),
        "missing from core/protocol: {missing:?}; run node protocol/sync.ts core/protocol"
    );
    for file in &mine {
        let (a, b) = (fs::read(copy.join(file)), fs::read(source.join(file)));
        assert!(
            matches!((&a, &b), (Ok(a), Ok(b)) if a == b),
            "core/protocol/{} differs from protocol/{0}: run node protocol/sync.ts core/protocol",
            file.display()
        );
    }
}

/// The files `protocol/sync.ts` copies under `dir`, relative to `root`: JSON, and the agent
/// texts beside the descriptors.
fn files(root: &Path, dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).expect("a protocol directory") {
        let path = entry.expect("an entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let at_top = dir == root;
        if path.is_dir() && at_top && (name == "schema" || name == "agents") {
            out.extend(files(root, &path));
        } else if name.ends_with(".json") || (!at_top && name.ends_with(".md") && name != "README.md") {
            out.push(path.strip_prefix(root).expect("under the root").to_path_buf());
        }
    }
    out.sort();
    out
}
