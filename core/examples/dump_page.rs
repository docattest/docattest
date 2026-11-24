//! Development tool: print the page model of each Markdown file named on the command line
//! as one JSON line.

use docattest::debug::model::{BlockKind, LeafKind, Page};
use docattest::debug::parse_page;
use serde_json::{Value, json};

fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        let page = match parse_page(&text) {
            Ok(page) => dump(&page),
            Err(e) => json!({ "error": format!("docattest internal error: {e}") }),
        };
        println!("{}", json!({ "path": path, "page": page }));
    }
}

fn dump(page: &Page) -> Value {
    let block = |b: &docattest::debug::model::Block| {
        let mut v = json!({ "type": b.type_name(), "start": b.start, "end": b.end, "previous": b.previous });
        match &b.kind {
            BlockKind::Heading { text, .. } | BlockKind::Paragraph { text } => v["text"] = json!(text),
            BlockKind::List { items } => {
                v["items"] = items.iter().map(|i| json!({"line": i.line, "value": i.value})).collect()
            }
            BlockKind::Table(t) => v["table"] = json!(t),
            BlockKind::Code { info, value } => {
                v["info"] = json!(info);
                v["value"] = json!(value);
            }
            BlockKind::Other => {}
        }
        v
    };
    json!({
        "frozen": page.is_frozen(),
        "flow": page.flow.iter().map(block).collect::<Vec<_>>(),
        "blocks": page.blocks,
        "tables": page.tables.iter().map(|t| json!({
            "header": t.header,
            "rows": t.rows.iter().map(|r| json!({
                "line": r.line,
                "cells": r.cells.iter().map(|c| json!({"raw": c.raw, "values": c.values})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "code": page.code.iter().map(|c| json!({"block": c.block})).collect::<Vec<_>>(),
        "comments": page.comments.iter().map(|c| json!({
            "body": c.body, "line": c.line, "endLine": c.end_line, "endsLine": c.ends_line,
            "block": c.block, "lead": c.lead, "table": c.table,
            "before": c.before.iter().map(|l| json!({
                "kind": if l.kind == LeafKind::Code { "code" } else { "text" },
                "value": l.value, "line": l.line,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "unclosed": page.unclosed.iter().map(|u| json!({
            "keyword": u.keyword.as_str(), "line": u.line, "block": u.block, "table": u.table,
        })).collect::<Vec<_>>(),
    })
}
