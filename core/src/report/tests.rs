//! Tests of the text and JSON reports, over reports and coverage built by hand.

use super::*;

fn pages(n: usize) -> Vec<String> {
    (1..=n).map(|k| format!("docs/guide/page-{k:02}.md")).collect()
}

fn report(uncovered: Vec<String>, frozen: Vec<String>) -> Report {
    let living = uncovered.len() + 1;
    let prose = Vec::new();
    let coverage = Coverage {
        living,
        covered: 1,
        uncovered,
        prose,
        frozen,
        attestations: 1,
        weak: 0,
        cited: 0,
        cited_run: None,
    };
    let (surfaces, snapshots, warnings) = (Vec::new(), Vec::new(), Vec::new());
    Report { results: Vec::new(), coverage, surfaces, snapshots, warnings, exit_code: 0, changes: None }
}

#[test]
fn a_long_list_goes_on_over_lines_that_fit_and_a_short_one_stays_on_one() {
    assert_eq!(
        wrapped("  uncovered: ", &pages(2)),
        ["  uncovered: docs/guide/page-01.md, docs/guide/page-02.md"]
    );
    let lines = wrapped("  uncovered: ", &pages(9));
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|line| line.chars().count() <= TEXT_WIDTH), "{lines:#?}");
    assert!(lines[1].starts_with("             docs/guide/page-04.md,"), "{lines:#?}");
    assert!(lines[2].ends_with("docs/guide/page-09.md"), "{lines:#?}");
    assert_eq!(lines.join(" ").split_whitespace().skip(1).collect::<Vec<_>>().join(" "), pages(9).join(", "));
}

#[test]
fn the_coverage_line_counts_proofs_and_citations_apart_and_their_sum() {
    let line = |attestations, weak, cited| {
        let mut r = report(Vec::new(), Vec::new());
        r.coverage = Coverage { attestations, weak, cited, ..r.coverage };
        r.format(Detail::Full).lines().find(|l| l.starts_with("coverage:")).unwrap_or_default().to_string()
    };
    assert_eq!(
        line(41, 2, 3),
        "coverage: 1 living page, 1 covered (41 attestations: 38 proofs, 2 of them weak, and 3 citations), 0 uncovered, 0 frozen"
    );
    assert!(line(2, 0, 1).contains("(2 attestations: 1 proof, 0 of them weak, and 1 citation)"));
    assert!(line(1, 1, 0).contains("(1 attestation: 1 proof, 1 of them weak, and 0 citations)"));
    let json = report(Vec::new(), Vec::new()).to_json();
    assert_eq!((&json["coverage"]["attestations"], &json["coverage"]["proofs"]), (&json!(1), &json!(1)));
}

#[test]
fn citations_whose_test_ran_are_counted_among_the_citations_only_when_tests_ran() {
    let mut r = report(Vec::new(), Vec::new());
    r.coverage = Coverage { attestations: 3, cited: 2, ..r.coverage };
    assert!(!r.format(Detail::Full).contains("of them run"));
    assert_eq!(r.to_json()["coverage"].get("citedRun"), None);
    r.coverage.cited_run = Some(1);
    assert!(
        r.format(Detail::Full)
            .contains("(3 attestations: 1 proof, 0 of them weak, and 2 citations, 1 of them run)")
    );
    assert_eq!(r.to_json()["coverage"]["citedRun"], json!(1));
}

#[test]
fn coverage_counts_the_results_of_living_pages_only() {
    let counts = HashMap::from([("docs/a.md".to_string(), 3), ("snapshots/broken.json".to_string(), 1)]);
    let pages = PageSets { living: &["docs/a.md".into()], prose: &[], frozen: &[] };
    let coverage = coverage_for(pages, &counts, 0, 1);
    assert_eq!((coverage.attestations, coverage.proofs(), coverage.cited), (3, 2, 1));
}

#[test]
fn a_prose_page_without_results_is_counted_apart_from_the_uncovered() {
    let living = ["docs/a.md", "docs/guide.md", "docs/notes.md", "docs/plain.md"].map(String::from);
    let prose = ["docs/guide.md", "docs/notes.md"].map(String::from);
    let counts = HashMap::from([("docs/a.md".to_string(), 2), ("docs/notes.md".to_string(), 1)]);
    let coverage = coverage_for(PageSets { living: &living, prose: &prose, frozen: &[] }, &counts, 0, 0);
    assert_eq!((coverage.living, coverage.covered), (4, 2));
    assert_eq!(
        (coverage.uncovered.clone(), coverage.prose.clone()),
        (vec!["docs/plain.md".into()], vec!["docs/guide.md".into()])
    );
    let mut r = report(Vec::new(), Vec::new());
    r.coverage = coverage;
    assert!(r.format(Detail::Full).contains(", 1 uncovered, 1 prose, 0 frozen\n"));
    assert_eq!(r.to_json()["coverage"]["prose"], json!(["docs/guide.md"]));
    let none = report(Vec::new(), Vec::new());
    assert!(none.format(Detail::Full).contains(", 0 uncovered, 0 frozen\n"));
    assert!(none.to_json()["coverage"].get("prose").is_none());
}

#[test]
fn frozen_pages_are_counted_and_uncovered_ones_named_only_when_every_page_ran() {
    let both = report(pages(1), vec!["docs/old.md".into()]);
    let full = both.format(Detail::Full);
    assert!(full.contains("1 uncovered, 1 frozen"), "{full}");
    assert!(full.contains("\n  uncovered: docs/guide/page-01.md\n"), "{full}");
    assert!(!full.contains("docs/old.md"), "{full}");
    let named = both.format(Detail::Pages);
    assert!(!named.contains("page-01") && named.contains("1 uncovered, 1 frozen"), "{named}");
    assert_eq!(both.to_json()["coverage"]["frozen"], json!(["docs/old.md"]));
}

#[test]
fn the_configs_warnings_are_printed_and_kept_in_the_json_only_when_there_are_some() {
    let mut warned = report(Vec::new(), Vec::new());
    assert!(warned.to_json().get("warnings").is_none());
    warned.warnings.push("\"pages\" has \"guide/*.md\", which matches no file".into());
    assert!(
        warned
            .format(Detail::Full)
            .contains("\nwarning: \"pages\" has \"guide/*.md\", which matches no file\n")
    );
    assert_eq!(warned.to_json()["warnings"], json!(["\"pages\" has \"guide/*.md\", which matches no file"]));
    assert_eq!(warned.exit_code, 0);
}

#[test]
fn the_surface_line_counts_the_symbols_not_meant_to_be_documented_only_when_the_config_names_some() {
    let surface = |undocumented: Option<Vec<String>>| {
        let unattested = vec!["rate".to_string()];
        let warnings = Vec::new();
        let coverage =
            SurfaceCoverage { symbols: 9, attested: 7, unattested, undocumented, resolution: None, warnings };
        let mut r = report(Vec::new(), Vec::new());
        r.surfaces = vec![("default".into(), coverage)];
        let line = r
            .format(Detail::Full)
            .lines()
            .find(|l| l.starts_with("surface "))
            .unwrap_or_default()
            .to_string();
        (line, r.to_json()["surfaces"]["default"].get("undocumented").cloned())
    };
    assert_eq!(
        surface(Some(vec!["ProgramMain".into()])),
        (
            "surface default: 9 symbols, 7 attested, 1 not attested by any page, 1 not meant to be documented".into(),
            Some(json!(["ProgramMain"]))
        )
    );
    assert_eq!(
        surface(None),
        ("surface default: 9 symbols, 7 attested, 1 not attested by any page".into(), None)
    );
}
