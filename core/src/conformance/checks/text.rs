//! Checks of the questions about text, which need no session.

use std::collections::HashSet;

use serde_json::{Value, json};

use super::answer::items;
use crate::conformance::expect::{Checked, all_hold, ensure, same};
use crate::conformance::kit::{DocComment, Kit, KitExample};
use crate::conformance::requests::{hello, request};
use crate::conformance::suite::{Check, Suite};
use crate::conformance::talk::{Conversation, Line};

pub const CHECKS: [Check; 1] = [Check {
    name: "questions about text take many items and answer one for each, in order",
    run: answers_each,
}];

fn answers_each(suite: &Suite) -> Checked<Conversation> {
    let kit = suite.kit;
    let t = suite.talk(questions(kit))?;
    let doc_comments =
        t.result(9).and_then(|found| expect_doc_comments(found, t.result(11)?, &kit.doc_comment));
    all_hold([
        t.answered(2, &json!([]), "assertions of no examples"),
        t.answered(3, &json!([false, true, false]), "a block that asserts nothing is weak"),
        t.answered(4, &json!([]), "shapes of no expressions"),
        t.result(5).and_then(expect_shapes),
        t.answered(6, &json!([]), "mentions of no spans"),
        t.answered(
            7,
            &json!([kit.mention.symbol, null]),
            "the symbol a span names, and null for one that names none",
        ),
        t.answered(8, &json!([]), "doc comments of no files"),
        doc_comments,
        t.answered(10, &json!([]), "doc prose of no files"),
    ])?;
    Ok(t)
}

/// Each question about text twice, of nothing and of several items, from hello on.
fn questions(kit: &Kit) -> Vec<Line> {
    let (examples, expressions, mention) = (&kit.examples, &kit.expressions, &kit.mention);
    let block = |example: &KitExample| json!({ "lang": example.lang, "code": example.code });
    let blocks = [block(&examples.weak), block(&examples.passes), block(&examples.weak)];
    let doc_comment = json!({ "page": kit.doc_comment.page, "text": kit.doc_comment.text });
    let other = json!({ "page": kit.not_source, "text": kit.doc_comment.text });
    let asked = [
        ("assertions", json!({ "examples": [] })),
        ("assertions", json!({ "examples": blocks })),
        ("expressions", json!({ "expressions": [] })),
        ("expressions", json!({ "expressions": [expressions.one, expressions.renamed, expressions.other] })),
        ("mentions", json!({ "spans": [], "symbols": [mention.symbol] })),
        ("mentions", json!({ "spans": [mention.span, mention.other], "symbols": [mention.symbol] })),
        ("docComments", json!({ "files": [] })),
        ("docComments", json!({ "files": [other, doc_comment, other] })),
        ("docProse", json!({ "files": [] })),
        ("docProse", json!({ "files": [other, doc_comment] })),
    ];
    let numbered =
        asked.into_iter().zip(2..).map(|((method, params), id)| Line::from(request(id, method, params)));
    std::iter::once(Line::from(hello())).chain(numbered).collect()
}

/// The three shapes asked for: the same expression renamed shares its shape, another does not.
fn expect_shapes(answer: &Value) -> Checked {
    let [one, renamed, different] = items(answer) else {
        return Err(format!("three expressions, three shapes: got {answer}"));
    };
    same(&renamed["shape"], &one["shape"], "the same expression with other names has the same shape")?;
    ensure(one["names"] != renamed["names"], "an expression renamed has other names")?;
    ensure(one["shape"] != different["shape"], "another expression has another shape")
}

/// The examples and prose of the kit's doc comment, and none of a file that is not source.
fn expect_doc_comments(examples: &Value, prose_answer: &Value, doc_comment: &DocComment) -> Checked {
    let found = items(examples);
    let said = "the examples of the source file, and none for a file that is not source";
    same(&json!(found.len()), &json!(doc_comment.examples), said)?;
    for example in found {
        same(&example["page"], &json!(doc_comment.page), "an example's page")?;
    }
    let ids: HashSet<String> = found.iter().map(|example| example["id"].to_string()).collect();
    ensure(ids.len() == found.len(), "ids are unique")?;
    let [none, prose] = items(prose_answer) else {
        return Err(format!("two files, two answers: got {prose_answer}"));
    };
    same(none, &json!([]), "a file that is not source has no doc comments")?;
    let prose = items(prose);
    ensure(
        !prose.is_empty() && prose.iter().all(|p| p["start"].as_u64() <= p["end"].as_u64()),
        "a doc comment's description has a start and an end after it",
    )?;
    ensure(
        prose.iter().any(|p| p["text"] == doc_comment.prose.as_str()),
        format!("a doc comment's description reads {}", json!(doc_comment.prose)),
    )
}
