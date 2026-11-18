//! The adapter's kit: a JSON file its author writes (PROTOCOL.md §5; each adapter's
//! `conformance.json` in the repository is a model) with a small project and labelled pieces
//! of code. It is held to its schema (`kit.json` among the protocol's schemas) and read into
//! these types before any check runs; a kit that does not fit is refused with every place
//! where it does not.

use serde_json::{Map, Value};

use crate::json::parse_json;
use crate::schema::Schemas;

/// The page the suite's attestations claim to be on when the kit does not name one.
const DEFAULT_PAGE: &str = "docs/conformance.md";

/// What the suite is told about a language.
pub struct Kit {
    /// The name the adapter greets with.
    pub adapter: String,
    /// The page the suite's attestations claim to be on.
    pub page: String,
    /// The project, as `open` sends it: an object of path → text.
    pub files: Value,
    /// The surfaces the project configures, as `open` sends them: `[{ name, entry }]`.
    pub surfaces: Value,
    /// The setup module, a path among `files`.
    pub setup: Option<String>,
    /// A path that is not a source file of the language.
    pub not_source: String,
    pub doc_comment: DocComment,
    pub examples: Examples,
    pub facts: Facts,
    /// Names the default surface exports.
    pub exports: Vec<String>,
    pub expressions: Expressions,
    pub mention: Mention,
    pub in_code: InCode,
    pub test: KitTest,
    pub cases: Option<Cases>,
    pub affected: Option<Affected>,
    /// Values of a data block, for the `display` capability; none when the kit gives none.
    pub display: Vec<SameValue>,
}

/// A source file with doc comments: its text, how many examples they hold, one's prose.
pub struct DocComment {
    pub page: String,
    pub text: String,
    pub examples: usize,
    pub prose: String,
}

/// A piece of code in the kit, in a block of the language.
pub struct KitExample {
    pub lang: String,
    pub code: String,
    pub tags: Option<Vec<String>>,
}

/// Examples that pass, fail, do not compile, assert nothing, print, and take half a second.
pub struct Examples {
    pub passes: KitExample,
    pub fails: KitExample,
    pub broken: KitExample,
    pub weak: KitExample,
    pub prints: KitExample,
    pub slow: KitExample,
}

/// Fact expressions, with what they display.
pub struct Facts {
    pub scalar: ScalarFact,
    pub list: ListFact,
    pub text: TextFact,
    /// An expression that does not evaluate.
    pub invalid: String,
    /// An expression that reads the field `version` of the suite's manifest.
    pub manifest: String,
}

/// A scalar: its expression, the text it displays, and the symbol it names.
pub struct ScalarFact {
    pub expression: String,
    pub text: String,
    pub symbol: String,
}

/// A list: its expression and its items.
pub struct ListFact {
    pub expression: String,
    pub items: Vec<String>,
}

/// A piece of text: its expression and the text.
pub struct TextFact {
    pub expression: String,
    pub text: String,
}

/// Two expressions of one shape with other names, and one of another shape.
pub struct Expressions {
    pub one: String,
    pub renamed: String,
    pub other: String,
}

/// A code span that names `symbol`, and one that names nothing.
pub struct Mention {
    pub span: String,
    pub symbol: String,
    pub other: String,
}

/// A name the files hold, one they do not, and the files.
pub struct InCode {
    pub present: String,
    pub absent: String,
    pub files: Vec<String>,
}

/// A test the `tests` capability lists, and its suite if the language has them.
pub struct KitTest {
    pub file: String,
    pub title: String,
    pub suite: Option<String>,
}

/// A cases table: its runner, header, rows (header name → text), and each row's status.
pub struct Cases {
    pub runner: String,
    pub header: Vec<String>,
    pub rows: Vec<Map<String, Value>>,
    pub statuses: Vec<String>,
}

/// A change the `affected` capability sees: the files at the base, what changed, the names.
pub struct Affected {
    /// As `affected` sends it: an object of path → text.
    pub base: Value,
    pub changed: Vec<String>,
    pub names: Vec<String>,
}

/// A value of a data block as JSON holds it, and a fact expression of the same value.
pub struct SameValue {
    pub value: Value,
    pub expression: String,
}

/// The schema a kit is held to: its definition, and the file that holds it.
const KIT_DEFINITION: &str = "Kit";
const KIT_FILE: &str = "kit.json";

/// The kit `text` holds, or why the suite cannot run with it: every place it does not fit.
pub fn read_kit(text: &str, schemas: &Schemas) -> Result<Kit, String> {
    let kit = parse_json(text).map_err(|why| format!("not JSON: {why}"))?;
    let problems = schemas.check(&kit, KIT_DEFINITION, KIT_FILE);
    if !problems.is_empty() {
        return Err(problems.join("\n  "));
    }
    Ok(read_fields(&kit))
}

// What follows reads a kit that fits its schema: every required field is there, and every
// field is of its type, so a read does not check again.

fn read_fields(kit: &Value) -> Kit {
    let optional_text = |key: &str| kit.get(key).map(text);
    Kit {
        adapter: text(&kit["adapter"]),
        page: optional_text("page").unwrap_or_else(|| DEFAULT_PAGE.into()),
        files: kit["files"].clone(),
        surfaces: kit["surfaces"].clone(),
        setup: optional_text("setup"),
        not_source: text(&kit["notSource"]),
        doc_comment: doc_comment(&kit["docComment"]),
        examples: examples(&kit["examples"]),
        facts: facts(&kit["facts"]),
        exports: texts(&kit["exports"]),
        expressions: Expressions {
            one: text(&kit["expressions"]["one"]),
            renamed: text(&kit["expressions"]["renamed"]),
            other: text(&kit["expressions"]["other"]),
        },
        mention: Mention {
            span: text(&kit["mention"]["span"]),
            symbol: text(&kit["mention"]["symbol"]),
            other: text(&kit["mention"]["other"]),
        },
        in_code: InCode {
            present: text(&kit["inCode"]["present"]),
            absent: text(&kit["inCode"]["absent"]),
            files: texts(&kit["inCode"]["files"]),
        },
        test: KitTest {
            file: text(&kit["test"]["file"]),
            title: text(&kit["test"]["title"]),
            suite: kit["test"].get("suite").map(text),
        },
        cases: kit.get("cases").map(cases),
        affected: kit.get("affected").map(|affected| Affected {
            base: affected["base"].clone(),
            changed: texts(&affected["changed"]),
            names: texts(&affected["names"]),
        }),
        display: kit["display"].as_array().map_or(&[][..], Vec::as_slice).iter().map(same_value).collect(),
    }
}

fn same_value(field: &Value) -> SameValue {
    SameValue { value: field["value"].clone(), expression: text(&field["expression"]) }
}

fn doc_comment(field: &Value) -> DocComment {
    DocComment {
        page: text(&field["page"]),
        text: text(&field["text"]),
        examples: field["examples"].as_u64().map_or(0, |count| count as usize),
        prose: text(&field["prose"]),
    }
}

fn examples(field: &Value) -> Examples {
    let example = |name: &str| KitExample {
        lang: text(&field[name]["lang"]),
        code: text(&field[name]["code"]),
        tags: field[name].get("tags").map(texts),
    };
    Examples {
        passes: example("passes"),
        fails: example("fails"),
        broken: example("broken"),
        weak: example("weak"),
        prints: example("prints"),
        slow: example("slow"),
    }
}

fn facts(field: &Value) -> Facts {
    let (scalar, list, text_fact) = (&field["scalar"], &field["list"], &field["text"]);
    Facts {
        scalar: ScalarFact {
            expression: text(&scalar["expression"]),
            text: text(&scalar["text"]),
            symbol: text(&scalar["symbol"]),
        },
        list: ListFact { expression: text(&list["expression"]), items: texts(&list["items"]) },
        text: TextFact { expression: text(&text_fact["expression"]), text: text(&text_fact["text"]) },
        invalid: text(&field["invalid"]["expression"]),
        manifest: text(&field["manifest"]["expression"]),
    }
}

fn cases(field: &Value) -> Cases {
    let rows = field["rows"].as_array().map_or(&[][..], Vec::as_slice);
    Cases {
        runner: text(&field["runner"]),
        header: texts(&field["header"]),
        rows: rows.iter().filter_map(|row| row.as_object().cloned()).collect(),
        statuses: texts(&field["statuses"]),
    }
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

fn texts(value: &Value) -> Vec<String> {
    value.as_array().map_or(&[][..], Vec::as_slice).iter().map(text).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIT: &str = include_str!("../../../adapters/python/conformance.json");

    fn read(text: &str) -> Result<Kit, String> {
        read_kit(text, &Schemas::embedded())
    }

    #[test]
    fn a_kit_is_read_into_what_the_checks_need() {
        let kit = read(KIT).unwrap();
        assert_eq!(kit.adapter, "python");
        assert_eq!(kit.page, DEFAULT_PAGE);
        assert_eq!(kit.facts.list.items, ["A", "B", "C"]);
        assert_eq!(kit.doc_comment.examples, 1);
        assert_eq!(kit.cases.unwrap().rows.len(), 2);
        assert_eq!(kit.affected.unwrap().names, ["add"]);
    }

    #[test]
    fn a_kit_that_does_not_fit_its_schema_is_refused_with_every_place() {
        let without = |pointer: &str, key: &str| {
            let mut value: Value = serde_json::from_str(KIT).unwrap();
            value.pointer_mut(pointer).unwrap().as_object_mut().unwrap().remove(key);
            read(&value.to_string()).err().unwrap()
        };
        assert_eq!(without("/examples/passes", "code"), "$.examples.passes: lacks \"code\"");
        assert_eq!(without("", "inCode"), "$: lacks \"inCode\"");
        assert_eq!(without("/surfaces/0", "entry"), "$.surfaces[0]: lacks \"entry\"");
        let mut value: Value = serde_json::from_str(KIT).unwrap();
        value["files"]["lib.py"] = Value::from(1);
        value["exports"] = Value::from("N");
        assert_eq!(
            read(&value.to_string()).err().unwrap(),
            "$.files.lib.py: 1 is not of type string\n  $.exports: \"N\" is not of type array"
        );
        assert!(read("{").err().unwrap().starts_with("not JSON"));
    }
}
