use super::*;

fn found(lines: &[&str]) -> PageCandidates {
    candidates_of(&(lines.join("\n") + "\n")).unwrap()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

fn column(line: usize, end: usize, items: &[&str], name: &str) -> SetCandidate {
    SetCandidate { target: Target::Column, line, end, items: strings(items), column: Some(name.into()) }
}

#[test]
fn takes_lists_and_table_columns_with_fact_item_values() {
    let found = found(&[
        "Keys:",
        "",
        "- `A` the first",
        "- **B**: a bold lead term",
        "- plain text",
        "",
        "| Method | Path |",
        "|---|---|",
        "| GET | `/a` and `/b` |",
        "| POST | `/c` |",
    ]);
    assert_eq!(
        found.sets,
        vec![
            vec![SetCandidate {
                target: Target::List,
                line: 3,
                end: 5,
                items: strings(&["A", "B", "plain text"]),
                column: None
            }],
            vec![column(7, 10, &["GET", "POST"], "Method"), column(7, 10, &["/a", "/b", "/c"], "Path")],
        ]
    );
}

#[test]
fn leaves_out_lists_and_columns_with_fewer_than_two_distinct_items() {
    let found =
        found(&["- `A`", "- `A`", "", "| Name | Kind |", "|---|---|", "| `x` | same |", "| `y` | same |"]);
    assert_eq!(found.sets, vec![vec![column(4, 7, &["x", "y"], "Name")]]);
}

#[test]
fn skips_prose_description_columns_that_mention_inline_code() {
    let found = found(&[
        "| Option | Description |",
        "|---|---|",
        "| `network` | Target cluster (`devnet` or `mainnet`) for RPC calls. |",
        "| `timeout` | Maximum wait before failing the request. |",
    ]);
    assert_eq!(found.sets, vec![vec![column(1, 4, &["network", "timeout"], "Option")]]);
}

#[test]
fn skips_whatever_a_directive_binds() {
    let found = found(&[
        "Keys: <!-- fact: KEYS -->",
        "- `A`",
        "",
        "<!-- fact column=Path: P -->",
        "",
        "| Path |",
        "|---|",
        "| `/a` |",
        "",
        "<!-- fact: SHAPE -->",
        "```ts",
        "x",
        "```",
        "",
        "Port `8080`. <!-- fact: PORT -->",
        "",
        "| limit | value |",
        "|---|---|",
        "| tries | `3` <!-- fact: N --> |",
        "",
        "Also `9090`.",
    ]);
    assert_eq!(found.sets, Vec::<Vec<SetCandidate>>::new());
    assert_eq!(found.blocks, Vec::new());
}

#[test]
fn leaves_out_the_sentence_a_directive_on_the_line_above_attests() {
    let found = found(&[
        "<!-- fact: PORT -->",
        "Port `8080`. Also `9090`.",
        "",
        "<!-- fact: KEYS -->",
        "Keys:",
        "- `a`",
        "- `b`",
    ]);
    let spans: Vec<&[String]> = found.sentences.iter().map(|s| s.spans.as_slice()).collect();
    assert_eq!(spans, [["9090".to_string()]]);
    assert_eq!(found.sets, Vec::<Vec<SetCandidate>>::new());
}

#[test]
fn treats_a_malformed_directive_as_binding_too() {
    let found = found(&["<!-- fact KEYS -->", "- `A`", "- `B`"]);
    assert_eq!(found.sets, Vec::<Vec<SetCandidate>>::new());
}

#[test]
fn a_test_citation_binds_no_list_and_no_sentence_below_it() {
    let found = found(&[
        "<!-- test: a.test.ts > \"keys\" -->",
        "Reads `/etc/app.conf`.",
        "",
        "<!-- test: a.test.ts > \"keys\" -->",
        "- `A`",
        "- `B`",
    ]);
    let spans: Vec<&[String]> = found.sentences.iter().map(|s| s.spans.as_slice()).collect();
    assert_eq!(spans[0], ["/etc/app.conf".to_string()]);
    assert_eq!(found.sets.len(), 1);
}

#[test]
fn keeps_documentation_blocks_and_leaves_out_examples() {
    let found = found(&["```yaml", "a: 1", "```", "", "```ts check", "x", "```"]);
    assert_eq!(found.blocks, vec![BlockCandidate { line: 1, text: "a: 1".into() }]);
}

#[test]
fn looks_inside_nothing_fenced() {
    let found = found(&["```md", "- `eu`", "- `us`", "Port `8080`.", "```"]);
    assert_eq!(found.sets, Vec::<Vec<SetCandidate>>::new());
}

#[test]
fn example_tags_need_word_boundaries() {
    assert!(is_example("ts check"));
    assert!(is_example("ts title=x,compile"));
    assert!(is_example("check-me"));
    assert!(!is_example("ts checked"));
    assert!(!is_example("ts precompile"));
    assert!(!is_example("ts check_"));
    assert!(is_example("ts chécheck")); // `é` is not a word character: `\w` is ASCII
}
