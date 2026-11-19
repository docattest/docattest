//! What `mutate` changes of the files facts read. The core reads them, so the core changes
//! what a call gives, in no language's terms: a line of a file's text, what a regex matched,
//! a value of a manifest. Each change goes to the session under a path of its own (the
//! call's path with a suffix), so that every change of every file is evaluated in one
//! session, by facts whose call names that path instead.

use serde_json::Value;

use super::calls::Call;
use super::{FactFiles, Named};
use crate::altered::{altered_scalar, altered_text};
use crate::attestations::field::shown_path;

/// One change to what one call gives, as if the file it reads said something else there.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadMutant {
    /// The call as the facts make it.
    pub call: Call,
    /// What a fact's call writes after the path to read the changed value instead.
    pub suffix: String,
    /// What changed, for the report: `Dockerfile:2: USER 10001 → USER 10002`.
    pub description: String,
}

impl FactFiles {
    /// At most `limit` changes of what each call that gives something gives, each added to
    /// what the session is given under its own path.
    pub fn add_mutants(&mut self, limit: usize) -> Vec<ReadMutant> {
        let mut mutants = Vec::new();
        let mut added = Vec::new();
        for named in &self.named {
            let Ok(value) = &named.read else { continue };
            for (changed, description) in changes_of(&named.call, value).into_iter().take(limit) {
                let suffix = format!("#mutant{}", mutants.len() + 1);
                let call = named.call.with_path(&format!("{}{suffix}", named.call.path()));
                added.push(Named { call, file: named.file.clone(), read: Ok(changed) });
                mutants.push(ReadMutant { call: named.call.clone(), suffix, description });
            }
        }
        self.named.extend(added);
        mutants
    }
}

/// Each changed value of what `call` gives, with what changed.
fn changes_of(call: &Call, value: &Value) -> Vec<(Value, String)> {
    let path = call.path();
    match (call, value) {
        (Call::File { .. }, Value::String(text)) => line_changes(path, text),
        (Call::Matched { pattern, .. }, Value::String(text)) => {
            let changed = altered_text(text);
            let description = format!("{path}: what \"{pattern}\" matches: {text} → {changed}");
            vec![(Value::String(changed), description)]
        }
        (Call::ManifestField { keys, .. }, _) => data_changes(value)
            .into_iter()
            .map(|change| (change.applied_to(value), format!("{path}: {}", change.described(keys))))
            .collect(),
        (Call::Manifest { .. }, _) => data_changes(value)
            .into_iter()
            .map(|change| (change.applied_to(value), format!("{path}: {}", change.described(""))))
            .collect(),
        (Call::File { .. } | Call::Matched { .. }, _) => Vec::new(),
    }
}

/// A file's text with one line changed, for each line that is not blank, in order.
fn line_changes(path: &str, text: &str) -> Vec<(Value, String)> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = Vec::new();
    for (k, line) in lines.iter().enumerate() {
        let content = line.trim_end_matches(['\r', '\n']);
        if content.trim().is_empty() {
            continue;
        }
        let changed = altered_text(content);
        let ending = &line[content.len()..];
        let mut whole: String = lines[..k].concat();
        whole.push_str(&changed);
        whole.push_str(ending);
        whole.push_str(&lines[k + 1..].concat());
        out.push((Value::String(whole), format!("{path}:{}: {content} → {changed}", k + 1)));
    }
    out
}

/// One change at one place in some data: a scalar replaced, or a list's last item removed.
struct DataChange {
    /// The keys from the top of the data to the place, a list's index as its digits.
    at: Vec<String>,
    was: Value,
    /// The new scalar; None when the last item of the list is removed.
    now: Option<Value>,
}

/// Every change of `data`, in document order: a list offers its last item removed before
/// the changes of its items.
fn data_changes(data: &Value) -> Vec<DataChange> {
    let mut out = Vec::new();
    collect_changes(data, &mut Vec::new(), &mut out);
    out
}

fn collect_changes(value: &Value, at: &mut Vec<String>, out: &mut Vec<DataChange>) {
    match value {
        Value::Array(items) => {
            if let Some(last) = items.last() {
                out.push(DataChange { at: at.clone(), was: last.clone(), now: None });
            }
            for (k, item) in items.iter().enumerate() {
                at.push(k.to_string());
                collect_changes(item, at, out);
                at.pop();
            }
        }
        Value::Object(entries) => {
            for (key, entry) in entries {
                at.push(key.clone());
                collect_changes(entry, at, out);
                at.pop();
            }
        }
        scalar => {
            if let Some(now) = altered_scalar(scalar) {
                out.push(DataChange { at: at.clone(), was: scalar.clone(), now: Some(now) });
            }
        }
    }
}

impl DataChange {
    /// `data` with this change made.
    fn applied_to(&self, data: &Value) -> Value {
        let mut changed = data.clone();
        let mut place = &mut changed;
        for key in &self.at {
            place = match place {
                Value::Array(items) => &mut items[key.parse::<usize>().expect("an index the walk wrote")],
                Value::Object(entries) => entries.get_mut(key).expect("a key the walk found"),
                _ => unreachable!("the walk goes only into lists and objects"),
            };
        }
        match (&self.now, place) {
            (Some(now), place) => *place = now.clone(),
            (None, Value::Array(items)) => {
                items.pop();
            }
            (None, _) => unreachable!("only a list has its last item removed"),
        }
        changed
    }

    /// What changed, the place written as `field=` writes a path, below `above` (the keys
    /// the call itself names, or none).
    fn described(&self, above: &str) -> String {
        let below = shown_path(&self.at);
        let place = match (above.is_empty(), below.is_empty()) {
            (true, true) => "the whole value".to_string(),
            (true, false) => below,
            (false, true) => above.to_string(),
            (false, false) => format!("{above}.{below}"),
        };
        match &self.now {
            Some(now) => format!("{place}: {} → {now}", self.was),
            None => format!("{place}: its last item, {}, removed", self.was),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(path: &str) -> Call {
        Call::File { path: path.into() }
    }

    #[test]
    fn a_text_changes_one_line_at_a_time_and_keeps_its_line_endings() {
        let changes = changes_of(&file("Dockerfile"), &json!("FROM node:22\r\n\nUSER app\n"));
        let texts: Vec<&str> = changes.iter().map(|(text, _)| text.as_str().unwrap()).collect();
        assert_eq!(texts, ["FROM node:23\r\n\nUSER app\n", "FROM node:22\r\n\nUSER appx\n"]);
        assert_eq!(changes[0].1, "Dockerfile:1: FROM node:22 → FROM node:23");
        assert_eq!(changes[1].1, "Dockerfile:3: USER app → USER appx");
    }

    #[test]
    fn a_match_and_a_field_change_as_values_of_their_kind() {
        let matched = Call::Matched { path: "Dockerfile".into(), pattern: r"^USER (\d+)$".into() };
        assert_eq!(
            changes_of(&matched, &json!("10001")),
            [(json!("10002"), r#"Dockerfile: what "^USER (\d+)$" matches: 10001 → 10002"#.to_string())]
        );
        let field = Call::ManifestField { path: "ci.yml".into(), keys: "deploy.replicas".into() };
        assert_eq!(changes_of(&field, &json!(3)), [(json!(4), "ci.yml: deploy.replicas: 3 → 4".to_string())]);
    }

    #[test]
    fn a_manifest_changes_each_scalar_and_drops_the_last_item_of_each_list() {
        let manifest = Call::Manifest { path: "package.json".into() };
        let data =
            json!({ "engines": { "node": ">=22" }, "files": ["dist", "bin"], "private": true, "x": null });
        let described: Vec<String> = changes_of(&manifest, &data).into_iter().map(|(_, d)| d).collect();
        assert_eq!(
            described,
            [
                r#"package.json: engines.node: ">=22" → ">=23""#,
                r#"package.json: files: its last item, "bin", removed"#,
                r#"package.json: files.0: "dist" → "distx""#,
                r#"package.json: files.1: "bin" → "binx""#,
                "package.json: private: true → false",
            ]
        );
        let (dropped, _) = &changes_of(&manifest, &data)[1];
        assert_eq!(dropped["files"], json!(["dist"]));
        assert_eq!(dropped["engines"], data["engines"]);
    }

    #[test]
    fn each_change_is_given_under_a_path_of_its_own_up_to_the_limit() {
        let mut fact_files = FactFiles {
            named: vec![Named { call: file("V"), file: Some("V".into()), read: Ok(json!("1\n2\n3\n")) }],
        };
        let mutants = fact_files.add_mutants(2);
        let suffixes: Vec<&str> = mutants.iter().map(|m| m.suffix.as_str()).collect();
        assert_eq!(suffixes, ["#mutant1", "#mutant2"]);
        assert_eq!(
            fact_files.session_params()["texts"],
            json!({ "V": "1\n2\n3\n", "V#mutant1": "2\n2\n3\n", "V#mutant2": "1\n3\n3\n" })
        );
    }
}
