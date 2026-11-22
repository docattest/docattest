//! Development tool: validate each config file named on the command line; prints one JSON
//! line each.
use docattest::config::{SurfaceEntry, parse_config};
use docattest::debug::Error;
use serde_json::json;

fn main() {
    for path in std::env::args().skip(1) {
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default()).into_owned();
        let out = match parse_config(&text) {
            Ok(c) => json!({
                "pages": c.pages, "frozen": c.frozen, "sources": c.sources,
                "surfaces": c.surfaces.iter().map(|(n, e)| match e {
                    SurfaceEntry::Specifier { path, .. } => json!([n, path]),
                    SurfaceEntry::Both { import, path, .. } => json!([n, { "import": import, "path": path }]),
                }).collect::<Vec<_>>(),
                "setup": c.setup, "dir": c.snapshots_dir, "maxAgeDays": c.max_age_days,
                "timeoutMs": c.timeout_ms, "ignoreTags": c.ignore_tags,
            }),
            Err(Error::User(m)) => json!({ "error": m }),
            Err(e) => json!({ "error": e.to_string() }),
        };
        println!("{out}");
    }
}
