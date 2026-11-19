//! Attestations: the units of proof found on pages.

use serde_json::{Map, Value, json};

use super::field::{FieldValue, as_json_writes};
use crate::console::transcript::Step;

/// What an attestation is, and what a result is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Example,
    Fact,
    Cases,
    Snapshot,
    Test,
    /// Something the adapter finds wrong in the project that belongs to no attestation
    /// (PROTOCOL.md §3, `problems`): a result, never an attestation.
    Problem,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Example => "example",
            Kind::Fact => "fact",
            Kind::Cases => "cases",
            Kind::Snapshot => "snapshot",
            Kind::Test => "test",
            Kind::Problem => "problem",
        }
    }

    pub fn parse(word: &str) -> Option<Kind> {
        [Kind::Example, Kind::Fact, Kind::Cases, Kind::Snapshot, Kind::Test, Kind::Problem]
            .into_iter()
            .find(|k| k.as_str() == word)
    }
}

/// Where a set target's items come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetFrom {
    List,
    /// The rows of the column the directive names.
    Table {
        column: String,
    },
    /// The code spans of the sentence the directive ends. A value that is not a list is
    /// compared with the last of them, as a scalar.
    Sentence,
    /// The list at the path of a fenced block's data that `field=` names.
    Field {
        path: String,
    },
}

impl SetFrom {
    pub fn as_str(&self) -> &'static str {
        match self {
            SetFrom::List => "list",
            SetFrom::Table { .. } => "table",
            SetFrom::Sentence => "sentence",
            SetFrom::Field { .. } => "field",
        }
    }
}

/// What a fact compares its value with: the literal, list, column, block or field of a block
/// on the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactTarget {
    Scalar(String),
    Set {
        items: Vec<String>,
        from: SetFrom,
    },
    Text(String),
    /// What `field=` found at `path` in a data block, as the block holds it.
    Field {
        path: String,
        value: FieldValue,
    },
}

impl FactTarget {
    /// The target as it is compared, a field's scalars written by `spell` in order: a field
    /// that holds a scalar is that scalar, one that holds a list the set of its items. Any
    /// other target is already written as the page writes it.
    pub fn spelled(&self, mut spell: impl FnMut(&Value) -> String) -> FactTarget {
        let FactTarget::Field { path, value } = self else { return self.clone() };
        match value {
            FieldValue::Scalar(scalar) => FactTarget::Scalar(spell(scalar)),
            FieldValue::List(items) => FactTarget::Set {
                items: items.iter().map(spell).collect(),
                from: SetFrom::Field { path: path.clone() },
            },
        }
    }
}

/// `check` runs an example; `compile` only type-checks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Check,
    Compile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    pub lang: String,
    pub mode: Mode,
    pub network: bool,
    pub code: String,
    /// A `check` block with no assertion: it runs, but it is an illustration.
    pub weak: bool,
    /// The documented declaration, for a doc-comment example.
    pub owner: Option<String>,
    /// The adapter's own info-string words on the block, as written; it alone reads them.
    pub tags: Vec<String>,
}

/// A `console check` block: commands and what they print. The core runs it itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Console {
    pub code: String,
    /// The directory the commands run in, relative to the project root; None for the root.
    pub cwd: Option<String>,
    /// The commands, in order; empty when the block could not be read (its `error` says why).
    pub steps: Vec<Step>,
}

/// An `md check` block: a sample page, which the core checks inside a copy of a fixture
/// project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    /// The block's language as written: `md` or `markdown`.
    pub lang: String,
    pub code: String,
    /// The fixture project, relative to the project root.
    pub fixture: String,
    /// Where the page is written in the copy, relative to its root.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub expression: String,
    pub surface: String,
    pub ordered: bool,
    pub column: Option<String>,
    /// The path `field=` names, as written.
    pub field: Option<String>,
    /// The literal the fact defends on its page, or why none could be bound to it.
    pub target: Result<FactTarget, String>,
}

/// One row of a cases table: its line, its first cell and every cell by header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasesRow {
    pub line: usize,
    pub name: String,
    /// Header → cell, in header order; a repeated header keeps its first place and last value.
    pub values: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cases {
    pub runner: String,
    pub header: Vec<String>,
    pub rows: Vec<CasesRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Test {
    pub file: String,
    pub title: String,
    pub describes: Vec<String>,
    /// The page's alias the citation names its file by, when it does. The core's own: the
    /// adapter is sent the file.
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Example(Example),
    Console(Console),
    Sample(Sample),
    Fact(Fact),
    Cases(Cases),
    Test(Test),
}

/// One unit of proof found on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Attestation {
    /// Unique within one check.
    pub id: String,
    pub page: String,
    pub line: usize,
    /// The sentence the proof defends.
    pub claim: String,
    /// A problem found while planning; the attestation is an `error` without running.
    pub error: Option<String>,
    pub body: Body,
}

impl Attestation {
    pub fn kind(&self) -> Kind {
        match self.body {
            Body::Example(_) | Body::Console(_) | Body::Sample(_) => Kind::Example,
            Body::Fact(_) => Kind::Fact,
            Body::Cases(_) => Kind::Cases,
            Body::Test(_) => Kind::Test,
        }
    }

    /// Whether the core runs it itself: no adapter is asked about it.
    pub fn is_run_by_core(&self) -> bool {
        matches!(self.body, Body::Console(_) | Body::Sample(_))
    }

    pub fn ok(&self) -> bool {
        self.problem().is_none()
    }

    /// Why the attestation is an `error` without running: a problem planning found, or, for a
    /// fact, why no target could be bound to it.
    pub fn problem(&self) -> Option<&str> {
        match (&self.error, &self.body) {
            (Some(error), _) => Some(error),
            (None, Body::Fact(Fact { target: Err(why), .. })) => Some(why),
            _ => None,
        }
    }

    /// The JSON form, keys in the order the recordings hold.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), json!(self.id));
        m.insert("page".into(), json!(self.page));
        m.insert("line".into(), json!(self.line));
        m.insert("claim".into(), json!(self.claim));
        m.insert("kind".into(), json!(self.kind().as_str()));
        match &self.body {
            Body::Example(e) => example_json(e, &mut m),
            Body::Console(c) => console_json(c, &mut m),
            Body::Sample(s) => sample_json(s, &mut m),
            Body::Fact(f) => fact_json(f, &mut m),
            Body::Cases(c) => cases_json(c, &mut m),
            Body::Test(t) => {
                m.insert("file".into(), json!(t.file));
                m.insert("title".into(), json!(t.title));
                m.insert("describes".into(), json!(t.describes));
            }
        }
        if let Some(problem) = self.problem() {
            m.insert("error".into(), json!(problem));
        }
        Value::Object(m)
    }
}

fn example_json(e: &Example, m: &mut Map<String, Value>) {
    m.insert("lang".into(), json!(e.lang));
    let mode = if e.mode == Mode::Check { "check" } else { "compile" };
    m.insert("mode".into(), json!(mode));
    m.insert("network".into(), json!(e.network));
    m.insert("code".into(), json!(e.code));
    m.insert("weak".into(), json!(e.weak));
    if let Some(owner) = &e.owner {
        m.insert("owner".into(), json!(owner));
    }
    if !e.tags.is_empty() {
        m.insert("tags".into(), json!(e.tags));
    }
}

// A block the core runs is an example that runs, in the shape of one.
fn console_json(c: &Console, m: &mut Map<String, Value>) {
    m.insert("lang".into(), json!("console"));
    m.insert("mode".into(), json!("check"));
    m.insert("code".into(), json!(c.code));
    if let Some(cwd) = &c.cwd {
        m.insert("cwd".into(), json!(cwd));
    }
}

fn sample_json(s: &Sample, m: &mut Map<String, Value>) {
    m.insert("lang".into(), json!(s.lang));
    m.insert("mode".into(), json!("check"));
    m.insert("code".into(), json!(s.code));
    m.insert("fixture".into(), json!(s.fixture));
    m.insert("as".into(), json!(s.path));
}

fn fact_json(f: &Fact, m: &mut Map<String, Value>) {
    m.insert("expression".into(), json!(f.expression));
    m.insert("surface".into(), json!(f.surface));
    m.insert("ordered".into(), json!(f.ordered));
    if let Some(column) = &f.column {
        m.insert("column".into(), json!(column));
    }
    if let Some(field) = &f.field {
        m.insert("field".into(), json!(field));
    }
    if let Ok(target) = &f.target {
        m.insert("target".into(), target_json(target));
    }
}

fn cases_json(c: &Cases, m: &mut Map<String, Value>) {
    m.insert("runner".into(), json!(c.runner));
    m.insert("header".into(), json!(c.header));
    let rows: Vec<Value> = c
        .rows
        .iter()
        .map(|r| {
            let mut values = Map::new();
            for (k, v) in &r.values {
                values.insert(k.clone(), json!(v));
            }
            json!({ "line": r.line, "name": r.name, "values": values })
        })
        .collect();
    m.insert("rows".into(), Value::Array(rows));
}

/// A target as the protocol and the report carry it: a field's values as JSON writes them.
pub fn target_json(target: &FactTarget) -> Value {
    match target {
        FactTarget::Scalar(value) => json!({ "kind": "scalar", "value": value }),
        FactTarget::Set { items, from } => {
            json!({ "kind": "set", "items": items, "from": from.as_str() })
        }
        FactTarget::Text(text) => json!({ "kind": "text", "text": text }),
        FactTarget::Field { .. } => target_json(&target.spelled(as_json_writes)),
    }
}

/// An attestation an adapter returned (a doc-comment example), or None when it is not one.
pub fn example_from_adapter(value: &Value) -> Option<Attestation> {
    let s = |k: &str| value.get(k).and_then(Value::as_str).map(str::to_string);
    if s("kind")?.as_str() != "example" {
        return None;
    }
    let mode = match s("mode")?.as_str() {
        "check" => Mode::Check,
        "compile" => Mode::Compile,
        _ => return None,
    };
    Some(Attestation {
        id: s("id")?,
        page: s("page")?,
        line: value.get("line")?.as_u64()? as usize,
        claim: s("claim").unwrap_or_default(),
        error: s("error"),
        body: Body::Example(Example {
            lang: s("lang").unwrap_or_default(),
            mode,
            network: value.get("network").and_then(Value::as_bool).unwrap_or(false),
            code: s("code").unwrap_or_default(),
            weak: value.get("weak").and_then(Value::as_bool).unwrap_or(false),
            owner: s("owner"),
            tags: value
                .get("tags")
                .and_then(Value::as_array)
                .map(|tags| tags.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
        }),
    })
}

#[cfg(test)]
mod adapter_example_tests {
    use super::*;

    #[test]
    fn an_adapter_example_goes_back_with_its_fields_and_the_cores_verdict() {
        let given = json!({
            "id": "e1", "page": "src/a.ts", "line": 4, "claim": "Adds.", "kind": "example",
            "lang": "ts", "mode": "check", "network": false, "code": "f()", "weak": false,
            "owner": "add", "tags": ["continue"]
        });
        let mut example = example_from_adapter(&given).unwrap();
        if let Body::Example(e) = &mut example.body {
            e.weak = true;
        }
        let mut expected = given.clone();
        expected["weak"] = json!(true);
        assert_eq!(example.to_json(), expected);
    }
}
