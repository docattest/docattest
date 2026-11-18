//! The cell helper of the harness, `docattest_harness::cells::code_spans`, held to the core
//! that ships with it: run with the `docattest` command on projects written to the
//! temporary directory.

mod project;

use serde_json::Value;

/// Cells as a page writes them, each with at least one code span: a cell with none is
/// compared by its text, which `code_spans` does not give.
const CELLS_AS_WRITTEN: [&str; 44] = [
    "`FOO=1` `BAR=2`",
    "`fmt.Println(1)`",
    "``a`b``",
    "`` `x` ``",
    "```a``b```",
    "`  padded  `",
    "`  `x",
    "text `a` more `b` end",
    "`a\\|b`",
    "\\`x` `y`",
    "\\\\`x`",
    "`x\\`y`",
    "``a` `b`",
    "`a`` `b`",
    "\t`\u{a0}tab and no-break\u{a0}` ",
    "`\u{feff}byte order mark\u{85}`",
    "`a` <!-- `b` --> `c`",
    "<span title=\"`\">`x`</span>",
    "<https://example.com/`a`> `b`",
    "<a@b.c> `d`",
    "a < b `c` > d",
    "**`bold`** _`em`_",
    "[`a`](b) `c`",
    "[a](`b`) `c`",
    "[a](u \"`t`\") `c`",
    "[a](u '`t`') `c`",
    "[a](u (`t`)) `c`",
    "[a](<`b` c>) `d`",
    "[a](b(`c`)) `d`",
    "[a] (`b`)",
    "a](`b`)",
    "\\[a](`b`)",
    "[[a](b)](`c`) `z`",
    "![[a](b)](`c`) `z`",
    "[![a](b)](`c`) `z`",
    "[a [b] c](`d`) `z`",
    "[a][`b`]",
    "[a`](b)` `c`",
    "[a](b `c`) `d`",
    "<!-->`a`",
    "<?x `a` ?> `b`",
    "<!X `a`> `b`",
    "<![CDATA[`a`]]> `b`",
    "</a `b`> `c`",
];

const MANIFEST: &str =
    "[package]\nname = \"cells\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n";

/// The results `docattest check --json` gives for a page of a project with no code.
fn checked(test: &str, page: &str, setup: Option<&str>) -> Vec<Value> {
    let config = format!(
        r#"{{ "pages": ["cells.md"], "surfaces": {{ "default": "cells" }}, {}"adapter": "rust" }}"#,
        setup.map_or("", |_| r#""setup": "setup.rs", "#)
    );
    let mut files = vec![
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", ""),
        ("docattest.config.json", &config),
        ("cells.md", page),
    ];
    files.extend(setup.map(|text| ("setup.rs", text)));
    let root = project::write(&format!("cells-{test}"), &files);
    let (out, _) = project::docattest(&root, &["check", "--json"], &[]);
    let _ = std::fs::remove_dir_all(&root);
    let report: Value = serde_json::from_str(&out).unwrap_or_else(|_| panic!("not JSON: {out}"));
    report["results"].as_array().unwrap().clone()
}

/// Each cell is the one row of a table whose column fact is what `code_spans` gives for
/// the cell as a runner gets it, read in the program the fact is compiled in: a fact that
/// fails is a cell the two read differently.
#[test]
fn code_spans_are_what_a_column_fact_compares() {
    let page: String = CELLS_AS_WRITTEN
        .iter()
        .map(|cell| {
            // The literal is in a comment, which a `-->` in it would end.
            let literal = format!("{:?}", cell.replace("\\|", "|")).replace("--", "-\\u{2d}");
            format!("<!-- fact column=cell: docattest_harness::cells::code_spans({literal}) -->\n| cell |\n|---|\n| {cell} |\n\n")
        })
        .collect();
    let results = checked("agree", &page, None);
    assert_eq!(results.len(), CELLS_AS_WRITTEN.len(), "{results:?}");
    for (cell, result) in CELLS_AS_WRITTEN.iter().zip(&results) {
        assert_eq!(result["status"], "pass", "{cell:?}: {}", result["message"]);
    }
}

const RUNNER: &str = r#"
use std::collections::HashMap;

pub const CASES: &[(&str, fn(&HashMap<String, String>) -> Result<(), String>)] = &[("spans", spans)];

fn spans(row: &HashMap<String, String>) -> Result<(), String> {
    let got = docattest_harness::cells::code_spans(&row["cell"]).join(",");
    if got == row["spans"] { Ok(()) } else { Err(format!("got {got}")) }
}
"#;

#[test]
fn a_setup_file_reads_its_cells_with_code_spans() {
    let page = "<!-- cases: spans -->\n| cell | spans |\n|---|---|\n| `FOO=1` `BAR=2` | FOO=1,BAR=2 |\n| ``a`b`` | a`b |\n";
    let results = checked("runner", page, Some(RUNNER));
    let statuses: Vec<&str> = results.iter().map(|result| result["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["pass", "pass"], "{results:?}");
}
