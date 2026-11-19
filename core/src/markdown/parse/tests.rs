use super::*;

fn parse_page(text: &str) -> Page {
    super::parse_page(text).unwrap_or_default()
}
use crate::markdown::model::{Declared, Keyword, LeafKind};

#[test]
fn frontmatter_freezes_and_invalid_yaml_does_not() {
    assert!(parse_page("---\ndocattest: false\ntitle: x\n---\n# H").is_frozen());
    assert!(!parse_page("---\n: : :\n---\n# H").is_frozen());
    assert_eq!(parse_page("---\ndocattest: prose\n---\n# H").declared, Declared::Prose);
}

#[test]
fn top_level_blocks_with_lines_from_positions() {
    let page = parse_page("# Title\n\nSome `code` here.\n\n- `a`\n- b\n\n```ts check\nx\n```\n");
    let blocks: Vec<_> = page
        .blocks
        .iter()
        .map(|&k| (page.flow[k].type_name(), page.flow[k].start, page.flow[k].end))
        .collect();
    assert_eq!(blocks, vec![("heading", 1, 1), ("paragraph", 3, 3), ("list", 5, 6), ("code", 8, 10)]);
    assert_eq!(page.flow[page.blocks[1]].kind, BlockKind::Paragraph { text: "Some `code` here.".into() });
    assert_eq!(
        page.flow[page.blocks[2]].kind,
        BlockKind::List {
            items: vec![ListItem { line: 5, value: "a".into() }, ListItem { line: 6, value: "b".into() }]
        }
    );
    assert_eq!(
        page.flow[page.code[0].block].kind,
        BlockKind::Code { info: "ts check".into(), value: "x".into() }
    );
}

#[test]
fn table_cells_as_written_with_the_pipe_escape_undone() {
    let page = parse_page(
        "| case | page |\n|---|---|\n| a | At `3` <!-- c --> x \\| y |\n| b |  |\n| c | `A`, **`B`** |\n",
    );
    let table = &page.tables[0];
    assert_eq!(table.header, vec!["case", "page"]);
    let raw: Vec<_> = table.rows[0].cells.iter().map(|c| c.raw.as_str()).collect();
    assert_eq!(raw, vec!["a", "At `3` <!-- c --> x | y"]);
    assert_eq!(table.rows[0].cells[1].values, ["3"]);
    let raw: Vec<_> = table.rows[1].cells.iter().map(|c| c.raw.as_str()).collect();
    assert_eq!(raw, vec!["b", ""]);
    assert_eq!(table.column_values(1), ["3", "", "A", "B"]);
}

#[test]
fn inline_comments_with_the_leaves_before_them() {
    let page = parse_page("At most `3` tries. <!-- fact: N -->");
    let c = &page.comments[0];
    assert_eq!((c.body.as_str(), c.line, c.end_line, c.ends_line), (" fact: N ", 1, 1, true));
    assert_eq!(c.lead, "At most `3` tries.");
    let leaves: Vec<_> = c.before.iter().map(|l| (l.kind, l.value.as_str(), l.line)).collect();
    assert_eq!(
        leaves,
        vec![(LeafKind::Text, "At most ", 1), (LeafKind::Code, "3", 1), (LeafKind::Text, " tries. ", 1)]
    );
}

#[test]
fn the_first_sentence_of_each_paragraph_and_heading_as_a_comment_at_its_end_would_see_it() {
    let page = parse_page("Port **`80`**, said\nthe docs. Then `81`.\n\nA \\*star\\*. More.\n\n## No end\n");
    let found: Vec<_> = page.first_sentences.iter().map(|s| (s.line, s.ends_line, s.lead.as_str())).collect();
    assert_eq!(
        found,
        vec![(2, false, "Port **`80`**, said the docs."), (4, false, "A \\*star\\*."), (6, true, "No end")]
    );
    let leaves: Vec<_> = page.first_sentences[0].before.iter().map(|l| (l.kind, l.value.as_str())).collect();
    assert_eq!(
        leaves,
        vec![
            (LeafKind::Text, "Port "),
            (LeafKind::Code, "80"),
            (LeafKind::Text, ", said"),
            (LeafKind::Text, "the docs.")
        ]
    );
}

#[test]
fn own_line_and_multi_line_comments_in_html_blocks() {
    let page = parse_page("<!-- a --> <!-- b -->\n\n<!--\n fact: X\n-->\n- item\n");
    let found: Vec<_> =
        page.comments.iter().map(|c| (trim(&c.body).to_string(), c.line, c.end_line, c.ends_line)).collect();
    assert_eq!(
        found,
        vec![("a".into(), 1, 1, true), ("b".into(), 1, 1, true), ("fact: X".into(), 3, 5, true)]
    );
    assert!(page.comments.iter().all(|c| c.alone));
    assert!(!parse_page("<!-- a --> <b>x</b>\n").comments[0].alone);
    assert!(!parse_page("Text <!-- a -->\n").comments[0].alone);
}

#[test]
fn unclosed_openings() {
    let page = parse_page("Text <!-- fact: N\n\nmore");
    assert_eq!(page.unclosed.len(), 1);
    assert_eq!((page.unclosed[0].keyword, page.unclosed[0].line), (Keyword::Fact, 1));
    let page = parse_page("<!-- cases: t");
    assert_eq!((page.unclosed[0].keyword, page.unclosed[0].line), (Keyword::Cases, 1));
    let page = parse_page("# A\n\n<!-- tests: launch = a_test.go\n\nIt runs.");
    assert_eq!((page.unclosed[0].keyword, page.unclosed[0].line), (Keyword::Tests, 3));
}

#[test]
fn comments_in_cells_know_their_table() {
    let page = parse_page("| a |\n|---|\n| `3` <!-- fact: N --> |\n");
    assert_eq!(page.comments[0].table, Some(0));
    assert!(!page.comments[0].ends_line);
}

#[test]
fn nested_blocks_in_document_order() {
    let page = parse_page("- item <!-- fact: X -->\n  - `a`\n");
    let flow: Vec<_> = page.flow.iter().map(|b| (b.type_name(), b.start)).collect();
    assert_eq!(
        flow,
        vec![("list", 1), ("other", 1), ("paragraph", 1), ("list", 2), ("other", 2), ("paragraph", 2)]
    );
}

#[test]
fn each_block_knows_the_block_before_it_in_its_container() {
    let page = parse_page("Intro.\n\n- First.\n  <!-- c -->\n- Second.\n\n> Quoted.\n");
    let previous: Vec<_> = page.flow.iter().map(|b| (b.type_name(), b.start, b.previous)).collect();
    assert_eq!(
        previous,
        vec![
            ("paragraph", 1, None),
            ("list", 3, Some(0)),
            ("other", 3, Some(0)),
            ("paragraph", 3, Some(0)),
            ("other", 4, Some(3)),
            ("other", 5, Some(0)),
            ("paragraph", 5, Some(0)),
            ("other", 7, Some(1)),
            ("paragraph", 7, Some(1)),
        ]
    );
    assert_eq!(page.comments[0].block, 4);
}

// Each case below is where markdown-rs and micromark differed; the expected values are
// micromark's.
fn top_types(text: &str) -> Vec<&'static str> {
    let page = parse_page(text);
    page.blocks.iter().map(|&k| page.flow[k].type_name()).collect()
}

#[test]
fn a_list_after_indented_code_is_checked_as_interrupting() {
    assert_eq!(top_types("    code\n\n6. Run"), vec!["code", "paragraph"]);
    assert_eq!(top_types("    code\n\n1. Run"), vec!["code", "list"]);
    assert_eq!(top_types("    code\n-"), vec!["code", "paragraph"]);
    assert_eq!(top_types("- a\n\nR\n3. x"), vec!["list", "paragraph"]);
    assert_eq!(top_types("<div>\n\n  <img src=\"x\">\n"), vec!["other", "other"]);
}

#[test]
fn table_rows_without_a_pipe_may_start_with_any_character() {
    for first in ["`x`", "*x*", "_x", "~x", "#x", "$x", "ex", "ix", "{x"] {
        let page = parse_page(&format!("a | b\n--- | ---\n{first} | y"));
        assert_eq!(page.tables.len(), 1, "{first}");
        assert_eq!(page.tables[0].rows.len(), 1, "{first}");
    }
    assert_eq!(top_types("`a` | b\n--- | ---\nx | y"), vec!["table"]);
}

#[test]
fn an_unclosed_fence_keeps_the_next_items_in_the_list() {
    let page = parse_page("- a\n  ```\n  x\n- b\n");
    let BlockKind::List { items } = &page.flow[page.blocks[0]].kind else { panic!("not a list") };
    assert_eq!(items.iter().map(|i| i.value.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
}

#[test]
fn frontmatter_that_never_closes_is_a_thematic_break() {
    assert_eq!(top_types("---\n\n> a"), vec!["other", "other"]);
    assert_eq!(top_types("+++\nx\n+++\n"), vec!["paragraph"]);
}

#[test]
fn slices_after_a_byte_order_mark_are_one_character_early() {
    let page = parse_page("\u{FEFF}`3` 1.\n\u{e9}\n");
    assert_eq!(page.flow[0].kind, BlockKind::Paragraph { text: "`3` 1.".into() });
}

#[test]
fn a_task_item_starting_with_markup_keeps_its_check_in_the_paragraph() {
    let page = parse_page("* [x] **compliant** (100%)\n");
    assert_eq!(page.flow[2].kind, BlockKind::Paragraph { text: "[x] **compliant** (100%)".into() });
}

#[test]
fn a_leading_escape_starts_the_content() {
    let page = parse_page("\\| a <!-- fact: N -->\n");
    assert_eq!(page.comments[0].lead, "\\| a");
}

#[test]
fn list_ends_exclude_trailing_blank_lines() {
    let page = parse_page("- a\n- b\n\nx");
    assert_eq!((page.flow[0].start, page.flow[0].end), (1, 2));
}

#[test]
fn literals_without_a_position_fail_the_page() {
    assert_eq!(super::parse_page("say 'www.example.com' ok"), Err(Positionless::Text));
    assert!(super::parse_page("mail a@b.com and https://x.org").is_ok());
}
