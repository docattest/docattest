//! Agent descriptors: what one coding agent's hooks look like, as data (`src/agent/`).
//!
//! The shipped ones are `protocol/agents/<name>.json`, carried in the binary by `build.rs`;
//! `protocol/schema/agent.json` defines the format and `docs/agents.md` explains it. Nothing
//! else in the core knows an agent.

use std::path::PathBuf;

use crate::api::errors::{Error, Result};
use crate::json::parse_json;
use serde_json::{Map, Value};

include!(concat!(env!("OUT_DIR"), "/agents.rs"));

/// Keys from the top of a JSON object down to one value.
pub(crate) type JsonPath = Vec<String>;

/// The one JSON object a hook prints: `fixed`, with the text put at `text`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reply {
    pub fixed: Map<String, Value>,
    pub text: JsonPath,
}

/// An event of the agent, bound to a docattest action.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Binding {
    pub event: String,
    pub matcher: Option<String>,
    /// `impact`: where the edited file's path is in the event.
    pub file: Option<JsonPath>,
    /// `check`: where the event says the agent is already continuing because of a hook.
    pub continuing: Option<JsonPath>,
    pub reply: Reply,
}

/// The actions an agent's events can be bound to, in the order they are installed.
pub const ACTIONS: [&str; 2] = ["impact", "check"];

/// The agent's settings file, and where a hook goes in it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Settings {
    pub file: String,
    pub local_file: Option<String>,
    pub hooks: String,
    pub group: Option<String>,
    pub handler: Map<String, Value>,
    pub command: String,
    pub defaults: Map<String, Value>,
}

/// One coding agent, as its descriptor describes it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Agent {
    pub name: String,
    pub project_root: Option<String>,
    pub skill: Option<String>,
    pub settings: Settings,
    pub impact: Option<Binding>,
    pub check: Option<Binding>,
}

impl Agent {
    /// The agent's event for an action; an agent that has none cannot run it as a hook.
    pub fn binding(&self, action: &str) -> Option<&Binding> {
        if action == "impact" { self.impact.as_ref() } else { self.check.as_ref() }
    }

    /// How a command of this agent writes the project root.
    pub fn root_as(&self) -> &str {
        self.project_root.as_deref().unwrap_or(".")
    }
}

/// Whether `text` looks like an agent's name; anything else given as `--agent` is a file.
pub(crate) fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Reads one descriptor, saying where a problem is.
struct Reader<'a> {
    place: &'a str,
}

impl Reader<'_> {
    fn fail<T>(&self, problem: impl std::fmt::Display) -> Result<T> {
        Err(Error::user(format!("{}: {problem}", self.place)))
    }

    fn object<'v>(
        &self,
        found: Option<&'v Value>,
        at: &str,
        keys: &[&str],
    ) -> Result<&'v Map<String, Value>> {
        let Some(Value::Object(object)) = found else {
            return self.fail(format!("\"{at}\" must be an object"));
        };
        match object.keys().find(|key| !keys.contains(&key.as_str())) {
            Some(key) => self.fail(format!("unknown key \"{at}.{key}\"")),
            None => Ok(object),
        }
    }

    fn string(&self, found: Option<&Value>, at: &str) -> Result<String> {
        match found {
            Some(Value::String(text)) if !text.is_empty() => Ok(text.clone()),
            _ => self.fail(format!("\"{at}\" must be a string that is not empty")),
        }
    }

    fn optional_string(&self, found: Option<&Value>, at: &str) -> Result<Option<String>> {
        found.map(|_| self.string(found, at)).transpose()
    }

    /// Any object; none where the key is absent.
    fn fields(&self, found: Option<&Value>, at: &str) -> Result<Map<String, Value>> {
        match found {
            None => Ok(Map::new()),
            Some(Value::Object(object)) => Ok(object.clone()),
            Some(_) => self.fail(format!("\"{at}\" must be an object")),
        }
    }

    fn path(&self, found: Option<&Value>, at: &str) -> Result<JsonPath> {
        let keys = match found {
            Some(Value::Array(keys)) => keys.as_slice(),
            _ => &[],
        };
        let path: Vec<String> = keys
            .iter()
            .filter_map(|key| match key {
                Value::String(key) if !key.is_empty() => Some(key.clone()),
                _ => None,
            })
            .collect();
        if path.is_empty() || path.len() != keys.len() {
            return self.fail(format!("\"{at}\" must be a list of keys that is not empty"));
        }
        Ok(path)
    }

    fn binding(&self, found: Option<&Value>, at: &str, path: &str) -> Result<Binding> {
        let hook = self.object(found, at, &["event", "matcher", path, "reply"])?;
        let event = self.string(hook.get("event"), &format!("{at}.event"))?;
        let matcher = self.optional_string(hook.get("matcher"), &format!("{at}.matcher"))?;
        let place = match hook.get(path) {
            None if path != "file" => None,
            found => Some(self.path(found, &format!("{at}.{path}"))?),
        };
        let reply = self.reply(hook.get("reply"), &format!("{at}.reply"))?;
        let (file, continuing) = if path == "file" { (place, None) } else { (None, place) };
        Ok(Binding { event, matcher, file, continuing, reply })
    }

    fn reply(&self, found: Option<&Value>, at: &str) -> Result<Reply> {
        let reply = self.object(found, at, &["fixed", "text"])?;
        let fixed = self.fields(reply.get("fixed"), &format!("{at}.fixed"))?;
        let text = self.path(reply.get("text"), &format!("{at}.text"))?;
        Ok(Reply { fixed, text })
    }

    fn name(&self, found: Option<&Value>) -> Result<String> {
        let name = self.string(found, "name")?;
        if !is_name(&name) {
            return self.fail(
                "\"name\" must be lower-case letters, digits, \"-\" and \"_\", starting with a letter",
            );
        }
        Ok(name)
    }

    fn settings(&self, found: Option<&Value>) -> Result<Settings> {
        let keys = ["file", "localFile", "hooks", "group", "handler", "command", "defaults"];
        let settings = self.object(found, "settings", &keys)?;
        Ok(Settings {
            file: self.string(settings.get("file"), "settings.file")?,
            local_file: self.optional_string(settings.get("localFile"), "settings.localFile")?,
            hooks: self.string(settings.get("hooks"), "settings.hooks")?,
            group: self.optional_string(settings.get("group"), "settings.group")?,
            handler: self.fields(settings.get("handler"), "settings.handler")?,
            command: self.string(settings.get("command"), "settings.command")?,
            defaults: self.fields(settings.get("defaults"), "settings.defaults")?,
        })
    }

    /// The events bound to `impact` and to `check`.
    fn hooks(&self, found: Option<&Value>) -> Result<(Option<Binding>, Option<Binding>)> {
        let hooks = self.object(found, "hooks", &ACTIONS)?;
        let bound = |action: &str, path: &str| {
            let found = hooks.get(action);
            found.map(|_| self.binding(found, &format!("hooks.{action}"), path)).transpose()
        };
        Ok((bound("impact", "file")?, bound("check", "continuing")?))
    }
}

/// Read a descriptor from parsed JSON, refusing what the format does not know: a mistake in
/// a descriptor is reported where it is, never guessed around. `place` names it in messages.
pub(crate) fn read_agent(value: &Value, place: &str) -> Result<Agent> {
    let reader = Reader { place };
    let Value::Object(top) = value else {
        return reader.fail("it does not hold a JSON object");
    };
    let known = ["$schema", "name", "docs", "projectRoot", "skill", "settings", "hooks"];
    if let Some(key) = top.keys().find(|key| !known.contains(&key.as_str())) {
        return reader.fail(format!("unknown key \"{key}\""));
    }
    let name = reader.name(top.get("name"))?;
    reader.optional_string(top.get("docs"), "docs")?;
    let project_root = reader.optional_string(top.get("projectRoot"), "projectRoot")?;
    let skill = reader.optional_string(top.get("skill"), "skill")?;
    let settings = reader.settings(top.get("settings"))?;
    let (impact, check) = reader.hooks(top.get("hooks"))?;
    Ok(Agent { name, project_root, skill, settings, impact, check })
}

/// What `--agent` named, read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Loaded {
    pub agent: Agent,
    /// The descriptor's file, when `--agent` gave one; none for a shipped agent.
    pub file: Option<PathBuf>,
    /// Whether this is the agent meant when `--agent` is not given.
    pub is_default: bool,
}

/// The agent `--agent` names: a shipped one by name, the default one when nothing is given,
/// or a descriptor file of the user's own (anything that is not a bare name, so `./x.json`).
/// `resolve` makes a path given on the command line absolute.
pub(crate) fn load_agent(given: Option<&str>, resolve: impl Fn(&str) -> PathBuf) -> Result<Loaded> {
    let named = given.unwrap_or(DEFAULT);
    if !is_name(named) {
        let file = resolve(named);
        let place = file.display().to_string();
        let Ok(text) = std::fs::read(&file) else {
            return Err(Error::user(format!("{place}: cannot read this agent descriptor file")));
        };
        let agent = read_agent(&parse(&String::from_utf8_lossy(&text), &place)?, &place)?;
        return Ok(Loaded { agent, file: Some(file), is_default: false });
    }
    let Some((_, text)) = SHIPPED.iter().find(|(name, _)| *name == named) else {
        let known: Vec<&str> = SHIPPED.iter().map(|(name, _)| *name).collect();
        return Err(Error::user(format!(
            "unknown agent \"{named}\" (known: {}); for another agent pass its descriptor file, as in --agent ./{named}.json",
            known.join(", ")
        )));
    };
    let place = format!("agent \"{named}\"");
    let agent = read_agent(&parse(text, &place)?, &place)?;
    Ok(Loaded { agent, file: None, is_default: named == DEFAULT })
}

/// Every shipped agent descriptor, with whether it is the default agent.
pub(crate) fn shipped_agents() -> Vec<(Agent, bool)> {
    SHIPPED
        .iter()
        .filter_map(|(name, text)| {
            let place = format!("agent \"{name}\"");
            let agent = read_agent(&parse(text, &place).ok()?, &place).ok()?;
            Some((agent, *name == DEFAULT))
        })
        .collect()
}

fn parse(text: &str, place: &str) -> Result<Value> {
    parse_json(text).map_err(|m| Error::user(format!("{place} is not valid JSON ({m})")))
}

/// The value at a path in a JSON object, or none where the path leaves it.
pub(crate) fn value_at<'a>(object: &'a Map<String, Value>, path: &[String]) -> Option<&'a Value> {
    let (last, inner) = path.split_last()?;
    let mut at = object;
    for key in inner {
        let Some(Value::Object(next)) = at.get(key) else {
            return None;
        };
        at = next;
    }
    at.get(last)
}

/// The answer a hook prints: the reply's fixed part, with `text` at its place.
pub(crate) fn reply_with(reply: &Reply, text: String) -> String {
    fn put(object: &mut Map<String, Value>, path: &[String], text: String) {
        let Some((key, rest)) = path.split_first() else {
            return;
        };
        if rest.is_empty() {
            object.insert(key.clone(), Value::String(text));
            return;
        }
        if !matches!(object.get(key), Some(Value::Object(_))) {
            object.insert(key.clone(), Value::Object(Map::new()));
        }
        if let Some(Value::Object(inner)) = object.get_mut(key) {
            put(inner, rest, text);
        }
    }
    let mut out = reply.fixed.clone();
    put(&mut out, &reply.text, text);
    Value::Object(out).to_string()
}

#[cfg(test)]
mod tests;
