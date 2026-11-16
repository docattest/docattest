//! Development tool: the plan of each Markdown file named on the command line, one JSON line
//! each.

use docattest::debug::ExampleOptions;
use docattest::debug::plan_page;
use serde_json::json;

fn main() {
    let languages: Vec<String> =
        ["ts", "typescript", "tsx", "mts", "cts", "js", "javascript", "jsx", "mjs", "cjs"]
            .iter()
            .map(|s| s.to_string())
            .collect();
    let ignore = vec!["twoslash".to_string()];
    let options = ExampleOptions { languages: &languages, ignore_tags: &ignore, info_words: &[] };
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        let plan = match plan_page("docs/p.md", &text, &options) {
            Ok(plan) => json!({
                "frozen": plan.frozen,
                "attestations": plan.attestations.iter().map(|a| a.to_json()).collect::<Vec<_>>(),
            }),
            Err(e) => json!({ "error": format!("docattest internal error: {e}") }),
        };
        println!("{}", json!({ "path": path, "page": plan }));
    }
}
