//! `hooks install`: a coding agent's hooks, merged into its settings file as its descriptor says,
//! and `hooks install --git`.

use super::git_hook::HookOutcome;
use std::path::Path;

use super::args::Options;
use super::command::{docattest_command, is_ours};
use super::integration::{Action, Integration};
use super::{Io, relative, resolve};
use crate::agents::{ACTIONS, Agent, Binding, Loaded, Settings, load_agent};
use crate::api::errors::{Error, Result};
use crate::console::shell::shell_word;
use crate::files::text_file;
use crate::git::ignored::is_ignored;
use crate::json::{parse_json, pretty};
use serde_json::{Map, Value};

/// What to do when a settings file already has a docattest hook for an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingHooks {
    Keep,
    Replace,
}

/// A hook that was added or updated: the agent's event, its filter, the command written for it.
#[derive(Debug, PartialEq, Eq)]
pub struct Added {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
    pub updated: bool,
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
    install_hooks_into(root, &agent.settings.file, agent, ExistingHooks::Keep, command)
}

/// Compute the hooks to add or update in `settings_file` (relative to `root`), without writing.
pub fn plan_hooks_into(
    root: &Path,
    settings_file: &str,
    agent: &Agent,
    existing: ExistingHooks,
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
    let plan = HookPlan { agent, existing, file: &file };
    let mut changed = Vec::new();
    for action in ACTIONS {
        if let Some(hook) = add_or_update_hook(&mut hooks, &plan, action, command(action))? {
            changed.push(hook);
        }
    }
    if changed.is_empty() {
        return Ok((file, changed, None));
    }
    add_defaults(settings, &layout.defaults);
    settings.insert(layout.hooks.clone(), Value::Object(hooks));
    Ok((file, changed, Some(format!("{}\n", pretty(&read)))))
}

/// Merge the agent's hooks into `settings_file` (relative to `root`).
pub fn install_hooks_into(
    root: &Path,
    settings_file: &str,
    agent: &Agent,
    existing: ExistingHooks,
    command: impl Fn(&str) -> String,
) -> Result<(String, Vec<Added>)> {
    let (file, changed, text) = plan_hooks_into(root, settings_file, agent, existing, command)?;
    if let Some(text) = text {
        text_file::write_text(&root.join(settings_file), &text).map_err(Error::user)?;
    }
    Ok((file, changed))
}

/// The hooks the settings hold already, by event.
fn existing_hooks(settings: &Map<String, Value>, key: &str, file: &str) -> Result<Map<String, Value>> {
    match settings.get(key) {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(hooks)) => Ok(hooks.clone()),
        Some(_) => Err(Error::user(format!("{file}: \"{key}\" is not an object"))),
    }
}

struct HookPlan<'a> {
    agent: &'a Agent,
    existing: ExistingHooks,
    file: &'a str,
}

/// Adds the action's hook to its event's list, or updates its command in place when
/// `plan.existing == ExistingHooks::Replace`.
fn add_or_update_hook(
    hooks: &mut Map<String, Value>,
    plan: &HookPlan<'_>,
    action: &str,
    next: String,
) -> Result<Option<Added>> {
    let Some(binding) = plan.agent.binding(action) else {
        return Ok(None);
    };
    let mut entries = event_entries(hooks, &plan.agent.settings.hooks, binding, plan.file)?;
    if entries.iter().any(|entry| has_command(entry, plan.agent, action)) {
        if plan.existing == ExistingHooks::Keep
            || !replace_in_entries(&mut entries, plan.agent, action, &next)
        {
            return Ok(None);
        }
        hooks.insert(binding.event.clone(), Value::Array(entries));
        return Ok(Some(Added {
            event: binding.event.clone(),
            matcher: binding.matcher.clone(),
            command: next,
            updated: true,
        }));
    }
    entries.push(Value::Object(hook_entry(&plan.agent.settings, binding, &next)));
    hooks.insert(binding.event.clone(), Value::Array(entries));
    Ok(Some(Added {
        event: binding.event.clone(),
        matcher: binding.matcher.clone(),
        command: next,
        updated: false,
    }))
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

/// Replace any docattest command for `action` inside `entries` with `next`. Returns `true`
/// when at least one command changed.
fn replace_in_entries(entries: &mut [Value], agent: &Agent, action: &str, next: &str) -> bool {
    let mut changed = false;
    for entry in entries {
        if replace_in_entry(entry, agent, action, next) {
            changed = true;
        }
    }
    changed
}

fn replace_in_entry(entry: &mut Value, agent: &Agent, action: &str, next: &str) -> bool {
    let key = &agent.settings.command;
    let Some(group) = &agent.settings.group else {
        return replace_handler_command(entry, key, action, next);
    };
    let Value::Object(obj) = entry else { return false };
    let Some(Value::Array(handlers)) = obj.get_mut(group) else { return false };
    let mut changed = false;
    for handler in handlers {
        if replace_handler_command(handler, key, action, next) {
            changed = true;
        }
    }
    changed
}

fn replace_handler_command(handler: &mut Value, key: &str, action: &str, next: &str) -> bool {
    let Value::Object(map) = handler else { return false };
    let Some(Value::String(current)) = map.get(key) else { return false };
    if !is_ours(current, action) || current == next {
        return false;
    }
    map.insert(key.to_string(), Value::String(next.to_string()));
    true
}

/// Install (or preview under `--dry-run`) coding-agent hooks (`init --hooks [--agent <name or file>] [--local] [--force]`).
pub(super) fn install_agent_hooks(root: &Path, options: &Options, io: &mut Io) -> Result<Integration> {
    let loaded = load_agent(options.agent.as_deref(), |given| resolve(&io.cwd, given))?;
    let target = settings_target(&loaded.agent, options.local)?;
    let run = docattest_command(loaded.agent.root_as())?;
    let which = agent_argument(&loaded, root);
    let build = |action: &str| format!("{run} {action} --hook{which}");
    let existing = if options.force { ExistingHooks::Replace } else { ExistingHooks::Keep };
    let (file, changed) = if options.dry_run {
        let (file, changed, _) = plan_hooks_into(root, target, &loaded.agent, existing, build)?;
        (file, changed)
    } else {
        install_hooks_into(root, target, &loaded.agent, existing, build)?
    };
    if !options.json {
        print_installed(io, &file, &changed, options.dry_run);
        warn_if_unignored(io, root, target, options.local);
    }
    let action = hooks_action(&changed);
    Ok(Integration { file, action })
}

fn hooks_action(changed: &[Added]) -> Action {
    if changed.is_empty() {
        Action::Unchanged
    } else if changed.iter().all(|h| h.updated) {
        Action::Update
    } else {
        Action::Add
    }
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

fn print_installed(io: &mut Io, file: &str, changed: &[Added], dry_run: bool) {
    if changed.is_empty() {
        io.out(&format!("docattest hooks: already installed in {file}"));
        return;
    }
    let updated_only = changed.iter().all(|h| h.updated);
    let verb = match (updated_only, dry_run) {
        (true, true) => "would update in",
        (true, false) => "updated in",
        (false, true) => "would add to",
        (false, false) => "added to",
    };
    io.out(&format!("docattest hooks: {verb} {file}"));
    for hook in changed {
        let matcher = hook.matcher.as_deref().map(|m| format!(" ({m})")).unwrap_or_default();
        io.out(&format!("  {}{matcher}: {}", hook.event, hook.command));
    }
}

/// Install (or preview under `--dry-run`) the git pre-commit hook (`init --git-hook`).
pub(super) fn install_git(root: &Path, options: &Options, io: &mut Io) -> Result<Integration> {
    let command = docattest_command(".")?;
    let hook = if options.dry_run {
        super::git_hook::plan_git_hook(root, &command)?.0
    } else {
        super::git_hook::install_git_hook(root, &command)?
    };
    let action = match hook.outcome {
        HookOutcome::AlreadyThere => Action::Unchanged,
        HookOutcome::Added => Action::Add,
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
    Ok(Integration { file: hook.file, action })
}

#[cfg(test)]
pub(crate) mod tests;
