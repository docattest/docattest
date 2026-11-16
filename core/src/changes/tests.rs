//! Tests of the comparison, over projects planned in memory. No adapter runs here: the block
//! languages are the TypeScript adapter's (as `adapter::fake` answers), and doc-comment examples
//! are built the way it reports them (at the comment's first line, defending its description).

use crate::report::Detail;
use serde_json::json;

use super::compare::changes_between;
use super::project::{Planned, plan_memory};
use super::report_changes;
use crate::adapter::fake::{Typescript, typescript};
use crate::api::attestation::{Attestation, Body, Test};
use crate::files::memory::MemoryFiles;
use crate::report::{Coverage, Report};

const SRC: &str = "export const LIMIT = 3;\nexport const KEYS = [\"A\", \"B\"];\n";
const FACT: &str = "A call is tried at most `3` times. <!-- fact: LIMIT -->";
const CLAIM: &str = "A successful call returns its value unchanged.";
const EXAMPLE: [&str; 4] =
    ["```ts check", "import { LIMIT } from 'docattest-fixture';", "assert.equal(LIMIT, 3);", "```"];

type Files = Vec<(String, String)>;

fn config(edit: impl Fn(&mut serde_json::Value)) -> String {
    let mut config = json!({
        "pages": ["docs/**/*.md"],
        "frozen": ["docs/old/**"],
        "sources": ["src/**/*.ts"],
        "surfaces": { "default": "./src/index.ts" },
        "adapter": "typescript",
    });
    edit(&mut config);
    config.to_string()
}

fn page(lines: &[&str]) -> String {
    format!("# Retry\n\n{}\n", lines.join("\n"))
}

fn lines<'a>(parts: &[&[&'a str]]) -> Vec<&'a str> {
    parts.iter().flat_map(|p| p.iter().copied()).collect()
}

fn base_page() -> String {
    page(&lines(&[&[FACT, "", CLAIM, ""], &EXAMPLE]))
}

fn plan(files: &[(String, String)]) -> Planned {
    plan_memory(MemoryFiles::new(files.to_vec()), &typescript()).unwrap()
}

/// The changes from one planned project to another, the stand-in language holding a session
/// over the head's files, whose one surface is `src/index.ts`.
fn between(base: &Planned, head: &Planned, head_files: &[(String, String)]) -> Vec<super::Change> {
    let ts = Typescript::open(head_files, &["src/index.ts"]);
    changes_between(Some(&base.project(&ts)), &head.project(&ts)).unwrap()
}

/// The base's files and the head's: the base with `edits` applied (None deletes a file).
fn sides(edits: &[(&str, Option<String>)], base: &[(&str, String)]) -> (Files, Files) {
    let mut base_files: Vec<(String, String)> =
        vec![("docattest.config.json".into(), config(|_| {})), ("src/index.ts".into(), SRC.into())];
    let put = |files: &mut Vec<(String, String)>, path: &str, text: Option<&String>| {
        files.retain(|(p, _)| p != path);
        if let Some(text) = text {
            files.push((path.to_string(), text.clone()));
        }
    };
    for (path, text) in base {
        put(&mut base_files, path, Some(text));
    }
    let mut head_files = base_files.clone();
    for (path, text) in edits {
        put(&mut head_files, path, text.as_ref());
    }
    (base_files, head_files)
}

/// Changes from `base` to the base with `edits` applied, as `type page:line`.
fn kinds_from(edits: &[(&str, Option<String>)], base: &[(&str, String)]) -> Vec<String> {
    let (base_files, head_files) = sides(edits, base);
    let (base, head) = (plan(&base_files), plan(&head_files));
    between(&base, &head, &head_files)
        .iter()
        .map(|c| format!("{} {}:{}", c.kind.as_str(), c.page, c.line))
        .collect()
}

fn kinds(edits: &[(&str, Option<String>)]) -> Vec<String> {
    kinds_from(edits, &[("docs/retry.md", base_page())])
}

fn retry(text: String) -> Vec<(&'static str, Option<String>)> {
    vec![("docs/retry.md", Some(text))]
}

const NONE: [&str; 0] = [];

#[test]
fn fails_a_claim_whose_fact_directive_was_deleted_at_the_claim_line_now() {
    let head =
        page(&lines(&[&["Intro.", "", "A call is tried at most `3` times.", "", CLAIM, ""], &EXAMPLE]));
    let (base_files, head_files) = sides(&retry(head), &[("docs/retry.md", base_page())]);
    let (base, now) = (plan(&base_files), plan(&head_files));
    let changes = between(&base, &now, &head_files);
    assert_eq!(changes.len(), 1);
    assert_eq!(
        (changes[0].kind.as_str(), changes[0].line, changes[0].severity.as_str()),
        ("orphaned-claim", 5, "fail")
    );
    assert_eq!(
        changes[0].message,
        "\"A call is tried at most `3` times.\" is still on the page, but the fact that proved it (docs/retry.md:3 at the base) is gone"
    );
}

#[test]
fn fails_a_claim_whose_example_was_deleted_and_one_whose_block_lost_its_tag() {
    assert_eq!(kinds(&retry(page(&[FACT, "", CLAIM]))), ["orphaned-claim docs/retry.md:5"]);
    let untagged = page(&lines(&[&[FACT, "", CLAIM, "", "```ts"], &EXAMPLE[1..]]));
    assert_eq!(kinds(&retry(untagged)), ["orphaned-claim docs/retry.md:5"]);
}

#[test]
fn is_silent_when_the_claim_goes_with_its_proof_or_the_proof_is_replaced() {
    assert_eq!(kinds(&retry(page(&lines(&[&[CLAIM, ""], &EXAMPLE])))), NONE);
    assert_eq!(kinds(&retry(page(&[FACT]))), NONE);
    let other = ["```ts check", "assert.ok(true);", "```"];
    assert_eq!(kinds(&retry(page(&lines(&[&[FACT, "", CLAIM, ""], &other])))), NONE);
    let proven = page(&lines(&[&["The limit is `3` calls.", ""], &EXAMPLE]));
    let as_fact = page(&["The limit is `3` calls. <!-- fact: LIMIT -->"]);
    assert_eq!(kinds_from(&[("docs/l.md", Some(as_fact))], &[("docs/l.md", proven)]), NONE);
}

#[test]
fn catches_a_light_rewording_but_stays_silent_on_a_substantial_one() {
    let light = page(&lines(&[&["A call is attempted at most `3` times.", "", CLAIM, ""], &EXAMPLE]));
    assert_eq!(kinds(&retry(light)), ["orphaned-claim docs/retry.md:3"]);
    let heavy = page(&lines(&[&["Calls are retried up to `3` times in total.", "", CLAIM, ""], &EXAMPLE]));
    assert_eq!(kinds(&retry(heavy)), NONE);
}

#[test]
fn counts_an_unproven_copy_that_was_already_there() {
    let base = page(&lines(&[&[FACT, "", "A call is tried at most `3` times.", "", CLAIM, ""], &EXAMPLE]));
    let head = page(&lines(&[&["A call is tried at most `3` times.", "", CLAIM, ""], &EXAMPLE]));
    assert_eq!(kinds_from(&retry(head), &[("docs/retry.md", base)]), NONE);
}

#[test]
fn covers_list_facts_table_cells_cases_tables_and_doc_comments() {
    let list = page(&["The keys are: <!-- fact: KEYS -->", "- `A`", "- `B`"]);
    let unbound = page(&["The keys are:", "- `A`", "- `B`"]);
    assert_eq!(
        kinds_from(&[("docs/k.md", Some(unbound))], &[("docs/k.md", list)]),
        ["orphaned-claim docs/k.md:3"]
    );
    let row = |cell: &str| page(&["| name | value |", "|---|---|", &format!("| limit | {cell} |")]);
    assert_eq!(
        kinds_from(&[("docs/t.md", Some(row("`3`")))], &[("docs/t.md", row("`3` <!-- fact: LIMIT -->"))]),
        ["orphaned-claim docs/t.md:5"]
    );
    let table = ["| case | in |", "|---|---|", "| one | `1` |"];
    let cases = page(&lines(&[&["Inputs are echoed back unchanged. <!-- cases: echo -->", ""], &table]));
    let plain = page(&lines(&[&["Inputs are echoed back unchanged.", ""], &table]));
    assert_eq!(
        kinds_from(&[("docs/c.md", Some(plain))], &[("docs/c.md", cases)]),
        ["orphaned-claim docs/c.md:3"]
    );
    let doc = |example: &str| {
        format!("/**\n * Doubles a number.\n{example} */\nexport const double = (n: number) => n * 2;\n")
    };
    let proven = doc(" * @example\n * // check\n * assert.equal(double(2), 4);\n");
    assert_eq!(
        kinds_from(&[("src/double.ts", Some(doc("")))], &[("src/double.ts", proven.clone())]),
        ["orphaned-claim src/double.ts:1"]
    );
    assert_eq!(kinds_from(&[("src/double.ts", None)], &[("src/double.ts", proven)]), NONE);
}

#[test]
fn matches_by_what_an_attestation_is_not_where_moves_renames_additions() {
    let moved = page(&lines(&[&["Prose added.", "", "More prose.", "", CLAIM, ""], &EXAMPLE, &["", FACT]]));
    assert_eq!(kinds(&retry(moved)), NONE);
    assert_eq!(kinds(&[("docs/retry.md", None), ("docs/retrying.md", Some(base_page()))]), NONE);
    assert_eq!(kinds(&[("docs/retry.md", None)]), NONE);
    let renamed_losing = page(&lines(&[&["A call is tried at most `3` times.", "", CLAIM, ""], &EXAMPLE]));
    assert_eq!(
        kinds(&[("docs/retry.md", None), ("docs/retrying.md", Some(renamed_losing))]),
        ["orphaned-claim docs/retrying.md:3"]
    );
}

#[test]
fn treats_a_vanished_attestation_of_another_kind_the_same_way() {
    let text = page(&lines(&[&["Retries stop after the third failure.", "", CLAIM, ""], &EXAMPLE]));
    let (files, _) = sides(&[], &[("docs/retry.md", text)]);
    let cited = Attestation {
        id: "x".into(),
        page: "docs/retry.md".into(),
        line: 3,
        claim: String::new(),
        error: None,
        body: Body::Test(Test { file: "t.ts".into(), title: String::new(), describes: Vec::new() }),
    };
    let mut base = plan(&files);
    base.attestations.push(cited.clone());
    let mut head = plan(&files);
    let changes = |base: &Planned, head: &Planned| -> Vec<String> {
        between(base, head, &files).iter().map(|c| format!("{}:{}", c.kind.as_str(), c.line)).collect()
    };
    assert_eq!(changes(&base, &head), ["orphaned-claim:3"]);
    head.attestations.push(cited);
    assert_eq!(changes(&base, &head), NONE);
}

#[test]
fn warns_when_an_attested_page_leaves_the_living_set_by_any_route() {
    let frozen = config(|c| c["frozen"] = json!(["docs/retry.md"]));
    assert_eq!(kinds(&[("docattest.config.json", Some(frozen))]), ["frozen-page docs/retry.md:1"]);
    let frontmatter = format!("---\ndocattest: false\n---\n{}", base_page());
    assert_eq!(kinds(&retry(frontmatter)), ["frozen-page docs/retry.md:1"]);
    let narrower = config(|c| c["pages"] = json!(["docs/other/**"]));
    assert_eq!(kinds(&[("docattest.config.json", Some(narrower))]), ["frozen-page docs/retry.md:1"]);
    assert_eq!(
        kinds(&[("docs/retry.md", None), ("docs/old/retry.md", Some(base_page()))]),
        ["frozen-page docs/old/retry.md:1"]
    );
}

#[test]
fn warns_when_a_check_block_becomes_compile_or_loses_every_assertion() {
    let compile = page(&lines(&[&[FACT, "", CLAIM, "", "```ts compile"], &EXAMPLE[1..]]));
    assert_eq!(kinds(&retry(compile)), ["downgrade docs/retry.md:7"]);
    let weak = page(&lines(&[&[FACT, "", CLAIM, ""], &EXAMPLE[..2], &["console.log(LIMIT);", "```"]]));
    assert_eq!(kinds(&retry(weak)), ["downgrade docs/retry.md:7"]);
    let refactored =
        page(&lines(&[&[FACT, "", CLAIM, ""], &EXAMPLE[..2], &["assert.ok(LIMIT === 3);", "```"]]));
    assert_eq!(kinds(&retry(refactored)), NONE);
}

#[test]
fn warns_when_a_fact_expression_changes_and_its_literal_does_not() {
    let weaker = FACT.replace("LIMIT", "Math.min(LIMIT, 3)");
    assert_eq!(
        kinds(&retry(page(&lines(&[&[&weaker, "", CLAIM, ""], &EXAMPLE])))),
        ["weakened docs/retry.md:3"]
    );
    let honest = page(&lines(&[&[&FACT.replace('3', "5"), "", CLAIM, ""], &EXAMPLE]));
    assert_eq!(
        kinds(&[("docs/retry.md", Some(honest)), ("src/index.ts", Some(SRC.replacen('3', "5", 1)))]),
        NONE
    );
    let renamed_fact = FACT.replace("LIMIT", "MAX_TRIES");
    let renamed_example: Vec<String> = EXAMPLE.iter().map(|l| l.replace("LIMIT", "MAX_TRIES")).collect();
    let renamed_example: Vec<&str> = renamed_example.iter().map(String::as_str).collect();
    let renamed = page(&lines(&[&[&renamed_fact, "", CLAIM, ""], &renamed_example]));
    assert_eq!(
        kinds(&[
            ("docs/retry.md", Some(renamed)),
            ("src/index.ts", Some(SRC.replacen("LIMIT", "MAX_TRIES", 1)))
        ]),
        NONE
    );
}

#[test]
fn has_nothing_to_say_without_a_base_config_or_without_a_change() {
    let head = plan(&[("docattest.config.json".into(), config(|_| {}))]);
    assert!(changes_between(None, &head.project(&typescript())).unwrap().is_empty());
    assert_eq!(kinds(&[]), NONE);
}

fn report() -> Report {
    Report {
        results: Vec::new(),
        coverage: Coverage {
            living: 1,
            covered: 1,
            uncovered: Vec::new(),
            frozen: Vec::new(),
            attestations: 1,
            weak: 0,
            cited: 0,
        },
        surfaces: Vec::new(),
        snapshots: Vec::new(),
        exit_code: 0,
        changes: None,
    }
}

// Tests of the report's changes section.
#[test]
fn with_changes_fails_the_report_on_a_fail_change_only() {
    let changes_to = |head: Vec<&str>| {
        let (base_files, head_files) = sides(&retry(page(&head)), &[("docs/retry.md", page(&[FACT]))]);
        let (base, now) = (plan(&base_files), plan(&head_files));
        between(&base, &now, &head_files)
    };
    let mut kept = report();
    report_changes(&mut kept, changes_to(vec![FACT]));
    assert_eq!((kept.changes.as_deref(), kept.exit_code), (Some(&[][..]), 0));
    assert!(kept.format(Detail::Full).ends_with("\nchanges since the base: none\nok"));
    assert_eq!(kept.to_json()["changes"], json!([]));

    let mut lost = report();
    report_changes(&mut lost, changes_to(vec!["A call is tried at most `3` times."]));
    assert_eq!(lost.exit_code, 1);
    let text = lost.format(Detail::Full);
    assert!(text.contains(
        "\nchanges since the base: 1 fail, 0 warnings\n  ✗ docs/retry.md:3 orphaned-claim: \"A call is tried"
    ));
    assert!(text.ends_with("FAILED"));
    let printed = lost.to_json();
    assert_eq!(printed["changes"][0]["type"], "orphaned-claim");
    assert_eq!(printed.as_object().unwrap().keys().next_back().map(String::as_str), Some("changes"));
    assert_eq!(printed["exitCode"], 1);

    let mut warned = report();
    let compile = lines(&[&[FACT, "", CLAIM, "", "```ts compile"], &EXAMPLE[1..]]);
    let (base_files, head_files) = sides(&retry(page(&compile)), &[("docs/retry.md", base_page())]);
    let (base, now) = (plan(&base_files), plan(&head_files));
    report_changes(&mut warned, between(&base, &now, &head_files));
    assert_eq!(warned.exit_code, 0);
    assert!(
        warned.format(Detail::Full).ends_with(
            "\n  ⚠ docs/retry.md:7 downgrade: the example became `compile`: it no longer runs\nok"
        )
    );

    // Without a base the report has no changes at all.
    assert!(!report().format(Detail::Full).contains("changes"));
    assert!(report().to_json().get("changes").is_none());
}

#[test]
fn what_is_wrong_with_a_base_is_known_once_it_is_found_before_its_files_are_read() {
    let broken = crate::git::tests::repository(&[("docattest.config.json", "{ nope"), ("docs/a.md", "A")]);
    let said = |reference: &str| super::read_base(&broken.0, reference).found().unwrap_err().to_string();
    assert!(
        said("HEAD").starts_with(
            "docattest: the base \"HEAD\" cannot be read: docattest.config.json is not valid JSON: "
        ),
        "{}",
        said("HEAD")
    );
    assert_eq!(
        said("nope"),
        format!("docattest: unknown git ref \"nope\": no such commit in {}", broken.0.display())
    );
    let invalid = crate::git::tests::repository(&[("docattest.config.json", "{}")]);
    let error = super::read_base(&invalid.0, "main").found().unwrap_err().to_string();
    assert!(
        error.starts_with("docattest: the base \"main\" cannot be read: invalid docattest.config.json: "),
        "{error}"
    );
    // A base with a valid config, and one without any, are found.
    let valid = crate::git::tests::repository(&[("docattest.config.json", &config(|_| {}))]);
    assert_eq!(super::read_base(&valid.0, "HEAD").found(), Ok(()));
    let bare = crate::git::tests::repository(&[("README.md", "before docattest")]);
    assert_eq!(super::read_base(&bare.0, "HEAD").found(), Ok(()));
}
