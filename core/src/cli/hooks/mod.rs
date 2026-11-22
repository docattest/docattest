//! `hooks install`: a coding agent's hooks, merged into its settings file as its descriptor says,
//! and `hooks install --git`.

use super::git_hook::HookOutcome;
use std::path::Path;

use super::args::Options;
use super::command::{docattest_command, is_ours};
use super::{Io, relative, resolve};
use crate::agents::{ACTIONS, Agent, Binding, Loaded, Settings, load_agent};
use crate::api::errors::{Error, Result};
use crate::console::shell::shell_word;
use crate::files::text_file;
use crate::git::ignored::is_ignored;
use crate::json::{parse_json, pretty};
use serde_json::{Map, Value};

/// A hook that was added: the agent's event, its filter, the command written for it.
#[derive(Debug, PartialEq, Eq)]
pub struct Added {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

/// How a hook is told which agent runs it: nothing for the agent meant by default, the name
/// of another shipped one, and a descriptor file by a path the hook can find again.
pub fn agent_argument(loaded: &Loaded, root: &Path) -> String {
    let Some(file) = &loaded.file else {
        return if loaded.is_default { String::new() } else { format!(" --agent {}", loaded.agent.name) };
    };
    let file = file.to_string_lossy();
    let rel = relative(root, &file);
    if rel.is_empty() || rel.starts_with("..") {
        format!(" --agent {}", shell_word(&file))
    } else {
        format!(" --agent {}/{}", loaded.agent.root_as(), shell_word(&rel))
    }
}

/// Merge the agent's hooks into its settings file under `root`, creating it if need be.
/// Everything already there is kept; a hook already present (a command that runs docattest
/// with the action) is not added again. `command` gives the shell command of an action's
/// hook. Returns the file and the hooks added.
#[cfg(test)]
pub fn install_hooks(
    root: &Path,
    agent: &Agent,
    command: impl Fn(&str) -> String,
) -> Result<(String, Vec<Added>)> {
    install_hooks_into(root, &agent.settings.file, agent, command)
}

/// Compute the hooks to add to `settings_file` (relative to `root`), without writing.
pub fn plan_hooks_into(
    root: &Path,
    settings_file: &str,
    agent: &Agent,
    command: impl Fn(&str) -> String,
) -> Result<(String, Vec<Added>, Option<String>)> {
    let layout = &agent.settings;
    let path = root.join(settings_file);
    let file = path.display().to_string();
    let mut read = read_settings(&path, &file)?;
    let Value::Object(settings) = &mut read else {
        return Err(Error::user(format!("{file} does not hold a JSON object")));
    };
    let mut hooks = existing_hooks(settings, &layout.hooks, &file)?;
    let mut added = Vec::new();
    for action in ACTIONS {
        if let Some(hook) = add_hook(&mut hooks, agent, action, &command, &file)? {
            added.push(hook);
        }
    }
    if added.is_empty() {
        return Ok((file, added, None));
    }
    add_defaults(settings, &layout.defaults);
    settings.insert(layout.hooks.clone(), Value::Object(hooks));
    Ok((file, added, Some(format!("{}\n", pretty(&read)))))
}

/// Merge the agent's hooks into `settings_file` (relative to `root`).
pub fn install_hooks_into(
    root: &Path,
    settings_file: &str,
    agent: &Agent,
    command: impl Fn(&str) -> String,
) -> Result<(String, Vec<Added>)> {
    let (file, added, text) = plan_hooks_into(root, settings_file, agent, command)?;
    if let Some(text) = text {
        text_file::write_text(&root.join(settings_file), &text).map_err(Error::user)?;
    }
    Ok((file, added))
}

/// The hooks the settings hold already, by event.
fn existing_hooks(settings: &Map<String, Value>, key: &str, file: &str) -> Result<Map<String, Value>> {
    match settings.get(key) {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(hooks)) => Ok(hooks.clone()),
        Some(_) => Err(Error::user(format!("{file}: \"{key}\" is not an object"))),
    }
}

/// Adds the action's hook to its event's list, unless the agent binds no event to the action
/// or the list runs it already.
fn add_hook(
    hooks: &mut Map<String, Value>,
    agent: &Agent,
    action: &str,
    command: &impl Fn(&str) -> String,
    file: &str,
) -> Result<Option<Added>> {
    let Some(binding) = agent.binding(action) else {
        return Ok(None);
    };
    let mut entries = event_entries(hooks, &agent.settings.hooks, binding, file)?;
    if entries.iter().any(|entry| has_command(entry, agent, action)) {
        return Ok(None);
    }
    let command = command(action);
    entries.push(Value::Object(hook_entry(&agent.settings, binding, &command)));
    hooks.insert(binding.event.clone(), Value::Array(entries));
    Ok(Some(Added { event: binding.event.clone(), matcher: binding.matcher.clone(), command }))
}

/// The list of an event's hooks, as the settings hold it.
fn event_entries(hooks: &Map<String, Value>, key: &str, binding: &Binding, file: &str) -> Result<Vec<Value>> {
    match hooks.get(&binding.event) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(entries)) => Ok(entries.clone()),
        Some(_) => Err(Error::user(format!(
            "{file}: \"{key}.{}\" is not a list; fix it and run again",
            binding.event
        ))),
    }
}

/// One hook, laid out as the agent's settings lay hooks out.
fn hook_entry(layout: &Settings, binding: &Binding, command: &str) -> Map<String, Value> {
    let mut handler = layout.handler.clone();
    handler.insert(layout.command.clone(), Value::String(command.to_string()));
    let mut entry = Map::new();
    if let Some(matcher) = &binding.matcher {
        entry.insert("matcher".into(), Value::String(matcher.clone()));
    }
    match &layout.group {
        Some(group) => {
            entry.insert(group.clone(), Value::Array(vec![Value::Object(handler)]));
        }
        None => {
            for (key, value) in handler.iter() {
                entry.insert(key.to_string(), value.clone());
            }
        }
    }
    entry
}

/// The agent's default settings, where the file does not set them.
fn add_defaults(settings: &mut Map<String, Value>, defaults: &Map<String, Value>) {
    for (key, value) in defaults.iter() {
        if !settings.contains_key(key) {
            settings.insert(key.to_string(), value.clone());
        }
    }
}

/// The settings as they are, or an empty object when there is no file.
fn read_settings(path: &Path, file: &str) -> Result<Value> {
    let text =
        if path.exists() { text_file::read_text(path).map_err(Error::user)? } else { "{}".to_string() };
    parse_json(&text)
        .map_err(|m| Error::user(format!("{file} is not valid JSON ({m}); fix it and run again")))
}

/// Whether an entry of an event's list already runs docattest with the action.
fn has_command(entry: &Value, agent: &Agent, action: &str) -> bool {
    let ours = |handler: &Value| {
        let Value::Object(handler) = handler else {
            return false;
        };
        matches!(handler.get(&agent.settings.command), Some(Value::String(command)) if is_ours(command, action))
    };
    let Some(group) = &agent.settings.group else {
        return ours(entry);
    };
    let Value::Object(entry) = entry else {
        return false;
    };
    let Some(Value::Array(handlers)) = entry.get(group) else {
        return false;
    };
    handlers.iter().any(ours)
}

/// Install (or preview under `--dry-run`) coding-agent hooks (`init --hooks [--agent <name or file>] [--local]`).
pub(super) fn install_agent_hooks(root: &Path, options: &Options, io: &mut Io) -> Result<String> {
    let loaded = load_agent(options.agent.as_deref(), |given| resolve(&io.cwd, given))?;
    let target = settings_target(&loaded.agent, options.local)?;
    let run = docattest_command(loaded.agent.root_as())?;
    let which = agent_argument(&loaded, root);
    let build = |action: &str| format!("{run} {action} --hook{which}");
    let (file, added) = if options.dry_run {
        let (file, added, _) = plan_hooks_into(root, target, &loaded.agent, build)?;
        (file, added)
    } else {
        install_hooks_into(root, target, &loaded.agent, build)?
    };
    if !options.json {
        print_installed(io, &file, &added, options.dry_run);
        warn_if_unignored(io, root, target, options.local);
    }
    Ok(file)
}

fn warn_if_unignored(io: &mut Io, root: &Path, target: &str, local: bool) {
    if local && is_ignored(root, target) == Some(false) {
        io.out(&format!(
            "note: {target} is not ignored by git; add it to .gitignore (or .git/info/exclude) so it is not committed"
        ));
    }
}

fn settings_target(agent: &Agent, local: bool) -> Result<&str> {
    if !local {
        return Ok(&agent.settings.file);
    }
    agent.settings.local_file.as_deref().ok_or_else(|| {
        Error::user(format!("agent \"{}\" has no local settings file: its descriptor names none", agent.name))
    })
}

fn print_installed(io: &mut Io, file: &str, added: &[Added], dry_run: bool) {
    if added.is_empty() {
        io.out(&format!("docattest hooks: already installed in {file}"));
        return;
    }
    let verb = if dry_run { "would add to" } else { "added to" };
    io.out(&format!("docattest hooks: {verb} {file}"));
    for hook in added {
        let matcher = hook.matcher.as_deref().map(|m| format!(" ({m})")).unwrap_or_default();
        io.out(&format!("  {}{matcher}: {}", hook.event, hook.command));
    }
}

/// Install (or preview under `--dry-run`) the git pre-commit hook (`init --git-hook`).
pub(super) fn install_git(root: &Path, options: &Options, io: &mut Io) -> Result<String> {
    let command = docattest_command(".")?;
    let hook = if options.dry_run {
        super::git_hook::plan_git_hook(root, &command)?.0
    } else {
        super::git_hook::install_git_hook(root, &command)?
    };
    if !options.json {
        match hook.outcome {
            HookOutcome::AlreadyThere => {
                io.out(&format!("docattest hooks: already installed in {}", hook.file))
            }
            HookOutcome::Added => {
                let verb = if options.dry_run { "would add" } else { "added" };
                io.out(&format!("docattest hooks: {verb} {}\n  pre-commit: {}", hook.file, hook.line))
            }
        }
    }
    Ok(hook.file)
}

#[cfg(test)]
pub(crate) mod tests;
