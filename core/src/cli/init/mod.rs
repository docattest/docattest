//! `docattest init`: detect language, docs pages and default surface, write
//! `docattest.config.json`, optionally wire agent/git integrations, and run an initial gap audit.

mod detect;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::args::Options;
use super::{Io, agents, finish, hooks, load, network, skill};
use crate::api::errors::{Error, Result};
use crate::audit::report_json;
use crate::audit::run::run_audit;
use crate::audit::text::format_audit;
use crate::config::{CONFIG_FILE, load_config};
use crate::files::source::DiskFiles;
use crate::json::pretty;
use detect::{DetectedConfig, detect};

/// Outcome of the config step of `init`.
enum ConfigStep {
    Wrote { path: PathBuf, detected: DetectedConfig, json: Value },
    Kept { path: PathBuf, adapter: String },
}

impl ConfigStep {
    fn adapter(&self) -> &str {
        match self {
            ConfigStep::Wrote { detected, .. } => &detected.adapter,
            ConfigStep::Kept { adapter, .. } => adapter,
        }
    }

    fn wrote(&self) -> bool {
        matches!(self, ConfigStep::Wrote { .. })
    }
}

/// `init [--force] [--dry-run] [--no-audit] [--agents] [--hooks [--local]] [--git-hook] [--skill [--global]] [--agent <name>] [--json]`.
pub(super) fn init(root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    validate_flags(options)?;
    let has_integrations = options.write_agents || options.hooks || options.git_hook || options.skill;
    let step = prepare_config(root, options, has_integrations)?;
    print_config_step(&step, options, io);
    let installed = install_integrations(root, step.adapter(), options, io)?;
    let audit_json = maybe_audit(root, &step, options, io);
    if options.json {
        io.out(&pretty(&json_output(&step, options.dry_run, installed, audit_json)));
    }
    Ok(0)
}

fn print_config_step(step: &ConfigStep, options: &Options, io: &mut Io) {
    if options.json {
        return;
    }
    match step {
        ConfigStep::Wrote { path, detected, .. } => {
            io.out(&format_init(path, detected, options.dry_run));
        }
        ConfigStep::Kept { path, .. } if options.dry_run => {
            io.out(&format!("docattest init: keeping existing {}", path.display()));
        }
        ConfigStep::Kept { .. } => {}
    }
}

fn install_integrations(
    root: &Path,
    adapter: &str,
    options: &Options,
    io: &mut Io,
) -> Result<Map<String, Value>> {
    let mut installed = Map::new();
    if options.write_agents {
        installed.insert("agents".into(), json!(agents::install_agents(root, options, io)?));
    }
    if options.hooks {
        installed.insert("hooks".into(), json!(hooks::install_agent_hooks(root, options, io)?));
    }
    if options.git_hook {
        installed.insert("gitHook".into(), json!(hooks::install_git(root, options, io)?));
    }
    if options.skill {
        installed.insert("skill".into(), json!(skill::install_project_skill(root, adapter, options, io)?));
    }
    Ok(installed)
}

fn maybe_audit(root: &Path, step: &ConfigStep, options: &Options, io: &mut Io) -> Option<Value> {
    if !step.wrote() || options.dry_run || options.no_audit {
        return None;
    }
    run_initial_audit(root, options.json, io)
}

fn validate_flags(options: &Options) -> Result<()> {
    if options.local && options.git_hook {
        return Err(Error::user("--local cannot be used with --git-hook"));
    }
    if options.local && !options.hooks {
        return Err(Error::user("--local requires --hooks"));
    }
    if options.global && !options.skill {
        return Err(Error::user("--global requires --skill"));
    }
    Ok(())
}

fn prepare_config(root: &Path, options: &Options, has_integrations: bool) -> Result<ConfigStep> {
    let config_path = root.join(CONFIG_FILE);
    if config_path.exists() {
        if has_integrations {
            return Ok(ConfigStep::Kept { path: config_path, adapter: existing_adapter(root) });
        }
        if !options.force {
            return Err(Error::user(format!(
                "{CONFIG_FILE} already exists in {} (pass --force to overwrite, or --agents / --hooks / --git-hook / --skill to add integrations)",
                root.display()
            )));
        }
    }
    let detected = detect(root);
    let config_json = detected.to_config_json();
    if !options.dry_run {
        let content = format!("{}\n", pretty(&config_json));
        fs::write(&config_path, &content)
            .map_err(|e| Error::user(format!("cannot write {}: {e}", config_path.display())))?;
    }
    Ok(ConfigStep::Wrote { path: config_path, detected, json: config_json })
}

fn existing_adapter(root: &Path) -> String {
    load_config(&DiskFiles::new(root.to_path_buf()))
        .map(|cfg| cfg.adapter)
        .unwrap_or_else(|_| detect(root).adapter)
}

fn run_initial_audit(root: &Path, json_mode: bool, io: &mut Io) -> Option<Value> {
    match initial_audit(root) {
        Ok(suggestions) => {
            if json_mode {
                return Some(report_json(&suggestions));
            }
            io.out("");
            io.out(&format_audit(&suggestions));
            if suggestions.iter().any(|s| s.status == crate::audit::Status::Match) {
                io.out("tip: run `docattest audit --apply` to insert matching directives");
            }
            None
        }
        Err(err) => {
            if json_mode {
                return Some(json!({ "skipped": err.to_string() }));
            }
            io.out(&format!("audit skipped: {err}"));
            None
        }
    }
}

fn initial_audit(root: &Path) -> Result<Vec<crate::audit::Suggestion>> {
    let state = load(root)?;
    let suggestions = run_audit(&state, network())?;
    finish(state)?;
    Ok(suggestions)
}

fn json_output(
    step: &ConfigStep,
    dry_run: bool,
    installed: Map<String, Value>,
    audit: Option<Value>,
) -> Value {
    let mut out = Map::new();
    match step {
        ConfigStep::Wrote { path, json, .. } => {
            out.insert("file".into(), json!(path.to_string_lossy()));
            out.insert("dryRun".into(), json!(dry_run));
            out.insert("config".into(), json.clone());
        }
        ConfigStep::Kept { path, .. } => {
            out.insert("file".into(), json!(path.to_string_lossy()));
            out.insert("dryRun".into(), json!(dry_run));
            out.insert("kept".into(), json!(true));
        }
    }
    if !installed.is_empty() {
        out.insert("installed".into(), Value::Object(installed));
    }
    if let Some(audit) = audit {
        out.insert("audit".into(), audit);
    }
    Value::Object(out)
}

fn format_init(path: &Path, detected: &DetectedConfig, dry_run: bool) -> String {
    let mut lines = Vec::new();
    let verb = if dry_run { "would write" } else { "wrote" };
    lines.push(format!("docattest init: {verb} {}", path.display()));
    lines.push(format!("  adapter: {}", detected.adapter));
    lines.push(format!("  pages: {}", detected.pages.join(", ")));
    match &detected.surface {
        Some(surface) => lines.push(format!("  surface default: {surface}")),
        None => lines.push("  surfaces: none (add surfaces later to bind facts to exported code)".into()),
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::super::{Io, run};
    use crate::git::tests::Temp;
    use std::fs;

    fn run_cli(root: &std::path::Path, args: &[&str]) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut io = Io { out: &mut out, err: &mut err, cwd: root.to_path_buf() };
        let code = run(&argv, &mut io);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    #[test]
    fn dry_run_previews_integrations_and_keeps_existing_config_without_writing() {
        let project = Temp::new("init-dry-run");
        let original = "{\n  \"pages\": [\"README.md\"],\n  \"adapter\": \"go\"\n}\n";
        project.write("docattest.config.json", original);

        let (code, stdout, stderr) = run_cli(
            &project.0,
            &["init", "--agents", "--hooks", "--local", "--skill", "--force", "--dry-run"],
        );
        assert_eq!(code, 0, "{stderr}");
        assert!(stdout.contains("docattest init: keeping existing "), "{stdout}");
        assert!(stdout.contains("docattest agents: would add to "), "{stdout}");
        assert!(stdout.contains("docattest hooks: would add to "), "{stdout}");
        assert!(stdout.contains("docattest skill would install "), "{stdout}");
        assert_eq!(fs::read_to_string(project.0.join("docattest.config.json")).unwrap(), original);
        assert!(!project.0.join("AGENTS.md").exists());

        let (code, json_out, stderr) = run_cli(
            &project.0,
            &["init", "--agents", "--hooks", "--local", "--skill", "--force", "--dry-run", "--json"],
        );
        assert_eq!(code, 0, "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(&json_out).unwrap();
        assert_eq!(parsed["dryRun"], true);
        assert_eq!(parsed["kept"], true);
        assert!(parsed["installed"]["agents"].is_string());
        assert!(parsed["installed"]["hooks"].is_string());
        assert!(parsed["installed"]["skill"].is_string());
        assert!(!project.0.join("AGENTS.md").exists());
    }

    #[test]
    fn subcommand_help_is_focused_on_that_command() {
        let project = Temp::new("subcommand-help");
        let (code, init_help, _) = run_cli(&project.0, &["init", "--help"]);
        assert_eq!(code, 0);
        assert!(init_help.starts_with("usage: docattest init [options]"), "{init_help}");
        assert!(init_help.contains("--dry-run"), "{init_help}");

        let (code, check_help, _) = run_cli(&project.0, &["help", "check"]);
        assert_eq!(code, 0);
        assert!(check_help.starts_with("usage: docattest check "), "{check_help}");
        assert!(check_help.contains("--member <dir>"), "{check_help}");

        let (code, top_help, _) = run_cli(&project.0, &["--help"]);
        assert_eq!(code, 0);
        assert!(top_help.starts_with("usage: docattest <command> [options]"), "{top_help}");
    }
}
