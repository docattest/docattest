use super::*;

fn path(keys: &[&str]) -> JsonPath {
    keys.iter().map(|key| key.to_string()).collect()
}

#[test]
fn every_shipped_descriptor_reads_and_is_named_as_its_file_is() {
    assert!(!SHIPPED.is_empty());
    assert!(SHIPPED.iter().any(|(name, _)| *name == DEFAULT));
    for (name, _) in SHIPPED {
        let loaded = load_agent(Some(name), |file| PathBuf::from(file)).unwrap();
        assert_eq!(loaded.agent.name, *name);
        assert_eq!((loaded.file, loaded.is_default), (None, *name == DEFAULT));
        assert!(loaded.agent.impact.is_some() || loaded.agent.check.is_some(), "{name}");
    }
    let default = load_agent(None, |file| PathBuf::from(file)).unwrap();
    assert!(default.is_default && default.agent.name == DEFAULT);
}

/// What only an agent's descriptor may say: where its settings and its skill are, how it
/// writes the project root, and every longer name of its events, their fields and its
/// answers. (An agent's own name may be an ordinary word, and is not looked for.)
fn particulars(descriptor: &str) -> Vec<String> {
    fn longer(value: &Value, found: &mut Vec<String>) {
        match value {
            Value::String(text) => found.push(text.clone()),
            Value::Array(items) => items.iter().for_each(|item| longer(item, found)),
            Value::Object(object) => {
                for (key, inner) in object.iter() {
                    found.push(key.to_string());
                    longer(inner, found);
                }
            }
            _ => {}
        }
    }
    let top: Map<String, Value> = serde_json::from_str(descriptor).unwrap();
    let mut found = Vec::new();
    longer(top.get("hooks").unwrap(), &mut found);
    found.retain(|text| text.len() >= 10 && text != "continuing");
    let agent = read_agent(&Value::Object(top), "shipped").unwrap();
    found.extend(agent.settings.file.split('/').next().map(String::from));
    found.extend([Some(agent.settings.file), agent.skill, agent.project_root].into_iter().flatten());
    found
}

#[test]
fn no_source_file_of_the_core_knows_what_a_descriptor_says() {
    fn sources(dir: &std::path::Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() { sources(&path, found) } else { found.push(path) }
        }
    }
    let mut files = Vec::new();
    sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    assert!(files.len() > 20);
    let particulars: Vec<String> = SHIPPED.iter().flat_map(|(_, text)| particulars(text)).collect();
    assert!(particulars.len() >= 5 * SHIPPED.len(), "{particulars:?}");
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for particular in &particulars {
            assert!(!text.contains(particular.as_str()), "{} says {particular}", file.display());
        }
    }
}

#[test]
fn a_name_is_not_a_path() {
    for name in ["a", "acme", "my-agent_2"] {
        assert!(is_name(name), "{name}");
    }
    for file in ["", "./acme", "acme.json", "Acme", "2a", "-a", "a/b", "é"] {
        assert!(!is_name(file), "{file}");
    }
}

const ACME: &str = r#"{
      "name": "acme", "docs": "https://example.invalid", "projectRoot": "\"$W\"", "skill": "acme/skill.md",
      "settings": { "file": "acme/config.json", "hooks": "automation", "command": "exec" },
      "hooks": {
        "impact": { "event": "file.saved", "file": ["change", "path"], "reply": { "fixed": { "ok": true, "notes": { "level": "info" } }, "text": ["notes", "body"] } },
        "check": { "event": "turn.ending", "continuing": ["turn", "retries"], "reply": { "text": ["message"] } }
      }
    }"#;

fn read(text: &str) -> Result<Agent> {
    read_agent(&parse_json(text).unwrap(), "mine.json")
}

#[test]
fn reads_a_descriptor_and_refuses_what_the_format_does_not_know() {
    let agent = read(ACME).unwrap();
    assert_eq!(agent.root_as(), "\"$W\"");
    assert_eq!(agent.settings.group, None);
    assert_eq!(agent.binding("impact").and_then(|b| b.file.clone()), Some(path(&["change", "path"])));
    assert_eq!(agent.binding("check").and_then(|b| b.continuing.clone()), Some(path(&["turn", "retries"])));
    let wrong = |from: &str, to: &str| {
        assert!(ACME.contains(from), "{from}");
        read(&ACME.replace(from, to)).unwrap_err().to_string()
    };
    assert_eq!(
        wrong("\"name\": \"acme\"", "\"name\": \"Acme\""),
        "docattest: mine.json: \"name\" must be lower-case letters, digits, \"-\" and \"_\", starting with a letter"
    );
    assert_eq!(wrong("\"docs\"", "\"title\""), "docattest: mine.json: unknown key \"title\"");
    assert_eq!(
        wrong("\"command\": \"exec\"", "\"command\": 1"),
        "docattest: mine.json: \"settings.command\" must be a string that is not empty"
    );
    assert_eq!(
        wrong("\"hooks\": \"automation\", ", ""),
        "docattest: mine.json: \"settings.hooks\" must be a string that is not empty"
    );
    assert_eq!(
        wrong("[\"change\", \"path\"]", "[]"),
        "docattest: mine.json: \"hooks.impact.file\" must be a list of keys that is not empty"
    );
    assert_eq!(wrong("\"continuing\"", "\"file\""), "docattest: mine.json: unknown key \"hooks.check.file\"");
    assert_eq!(
        wrong("\"text\": [\"message\"]", "\"text\": \"message\""),
        "docattest: mine.json: \"hooks.check.reply.text\" must be a list of keys that is not empty"
    );
    assert_eq!(read("[]").unwrap_err().to_string(), "docattest: mine.json: it does not hold a JSON object");
    assert_eq!(
        read("{}").unwrap_err().to_string(),
        "docattest: mine.json: \"name\" must be a string that is not empty"
    );
}

#[test]
fn an_answer_is_the_fixed_part_with_the_text_at_its_place() {
    let agent = read(ACME).unwrap();
    let reply = &agent.impact.as_ref().unwrap().reply;
    assert_eq!(
        reply_with(reply, "a \"b\"\n".into()),
        r#"{"ok":true,"notes":{"level":"info","body":"a \"b\"\n"}}"#
    );
    assert_eq!(reply_with(&agent.check.as_ref().unwrap().reply, "x".into()), r#"{"message":"x"}"#);
    let through = Reply { fixed: reply.fixed.clone(), text: path(&["ok", "deep", "er"]) };
    assert_eq!(reply_with(&through, "x".into()), r#"{"ok":{"deep":{"er":"x"}},"notes":{"level":"info"}}"#);
}

#[test]
fn a_value_is_found_by_its_path_or_not_at_all() {
    let event: Map<String, Value> = serde_json::from_str(r#"{"a":{"b":{"c":7}},"d":"x"}"#).unwrap();
    assert_eq!(value_at(&event, &path(&["a", "b", "c"])), Some(&Value::from(7)));
    assert_eq!(value_at(&event, &path(&["d"])), Some(&Value::String("x".into())));
    assert_eq!(value_at(&event, &path(&["d", "length"])), None);
    assert_eq!(value_at(&event, &path(&["a", "x", "c"])), None);
    assert_eq!(value_at(&event, &[]), None);
}
