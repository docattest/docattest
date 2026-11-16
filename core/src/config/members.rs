//! `members`: the projects below this one that `check` checks with it.

use serde_json::Value;

use super::Fields;

/// What an entry may be.
const SHAPE: &str = "a directory below this one, such as \"packages/api\", or \
                     { \"path\": \"<directory>\", \"adapterCommand\": [\"<program>\", \"<argument>\", …] }";

/// A project below this one, with a config of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Relative to this project's root, `/`-separated, with no `.` or `..` in it.
    pub path: String,
    /// The command that starts the member's adapter, read from this project's root; without
    /// one, the adapter is found as every adapter is.
    pub adapter_command: Option<Vec<String>>,
}

/// The members the config lists; none when it lists none.
pub(super) fn read_members(fields: &mut Fields) -> Vec<Member> {
    let Some(entries) = fields.object.get("members") else {
        return Vec::new();
    };
    let Value::Array(entries) = entries else {
        fields.problems.push("\"members\" must be an array".into());
        return Vec::new();
    };
    let mut members = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        match member(entry) {
            Some(member) => members.push(member),
            None => fields.problems.push(format!("\"members[{index}]\" must be {SHAPE}")),
        }
    }
    members
}

fn member(entry: &Value) -> Option<Member> {
    match entry {
        Value::String(path) => Some(Member { path: below(path)?, adapter_command: None }),
        Value::Object(object) => {
            if object.keys().any(|key| key != "path" && key != "adapterCommand") {
                return None;
            }
            let path = below(object.get("path")?.as_str()?)?;
            let adapter_command = match object.get("adapterCommand") {
                None => None,
                Some(words) => Some(command(words)?),
            };
            Some(Member { path, adapter_command })
        }
        _ => None,
    }
}

/// `path` when it names a directory strictly below the config's own.
fn below(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    let steps_down = trimmed.split('/').all(|step| !step.is_empty() && step != "." && step != "..");
    steps_down.then(|| trimmed.to_string())
}

/// A program and its arguments, each a non-empty string.
fn command(words: &Value) -> Option<Vec<String>> {
    let words: Vec<String> = words
        .as_array()?
        .iter()
        .map(|word| word.as_str().filter(|word| !word.is_empty()).map(str::to_string))
        .collect::<Option<_>>()?;
    (!words.is_empty()).then_some(words)
}

#[cfg(test)]
mod tests {
    use super::Member;
    use crate::config::parse_config;
    use serde_json::json;

    fn members(value: serde_json::Value) -> Result<Vec<Member>, String> {
        let config = json!({ "pages": [], "surfaces": { "default": "x" }, "adapter": "x", "members": value });
        parse_config(&config.to_string()).map(|c| c.members).map_err(|e| e.to_string())
    }

    fn member(path: &str, adapter_command: Option<&[&str]>) -> Member {
        let adapter_command = adapter_command.map(|words| words.iter().map(|w| w.to_string()).collect());
        Member { path: path.into(), adapter_command }
    }

    #[test]
    fn a_member_is_a_directory_below_with_perhaps_the_command_of_its_adapter() {
        let read =
            members(json!(["core", "packages/api/", { "path": "go", "adapterCommand": ["go", "run", "."] }]));
        assert_eq!(
            read.unwrap(),
            vec![member("core", None), member("packages/api", None), member("go", Some(&["go", "run", "."]))]
        );
        assert_eq!(members(json!([])).unwrap(), vec![]);
    }

    #[test]
    fn a_member_that_is_not_below_or_has_no_command_is_refused() {
        for bad in [
            json!("."),
            json!(""),
            json!("../other"),
            json!("/abs"),
            json!("a//b"),
            json!("a/./b"),
            json!(3),
            json!({ "adapterCommand": ["x"] }),
            json!({ "path": "a", "adapterCommand": [] }),
            json!({ "path": "a", "adapterCommand": ["x", ""] }),
            json!({ "path": "a", "adapterCommand": "x y" }),
            json!({ "path": "a", "adapter": "go" }),
        ] {
            let said = members(json!([bad])).unwrap_err();
            assert!(said.contains("\"members[0]\" must be a directory below this one"), "{bad}: {said}");
        }
        assert!(members(json!("core")).unwrap_err().contains("\"members\" must be an array"));
    }
}
