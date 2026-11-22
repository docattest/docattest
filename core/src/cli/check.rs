//! `check [pages…] [--base <ref>] [--run-cited]`: run the attestations of the project, and
//! of each of its members when no page is named, and print the reports.

use std::path::{Path, PathBuf};

use super::args::Options;
use super::ci_reports::ReportFiles;
use super::{Io, detail, finish, network, pretty, relative};
use crate::adapter::{Launch, Start};
use crate::api::errors::{Error, Result};
use crate::changes::BaseReading;
use crate::config::{Member, ProjectConfig, load_config};
use crate::files::source::DiskFiles;
use crate::project::check::{CitedTests, run_check};
use crate::project::state::{State, launch, select_pages};
use crate::report::Report;
use crate::report::members::{Checked, MemberReport};
use crate::snapshots::now_ms;

/// `check`: the reports, and exit code 1 when any project fails.
/// With `--report`, also writes the report for CI services.
pub(super) fn check(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let report_files = ReportFiles::asked(&options.reports, &io.cwd, root)?;
    let cited = if options.run_cited { CitedTests::Run } else { CitedTests::Declared };
    let asked = Asked { base: options.base.as_deref(), cited };
    let checked = match options.member.as_deref() {
        Some(member) => check_selected_member(args, root, member, asked)?,
        None => check_projects(args, root, asked)?,
    };
    if options.json {
        io.out(&pretty(&checked.to_json()));
    } else {
        io.out(&checked.format(detail(options, args)));
    }
    report_files.write(&checked)?;
    Ok(checked.exit_code())
}

/// What a check is asked besides its pages, the same for the project and every member: the
/// git ref whose change it judges, and whether it runs the tests citations name.
#[derive(Debug, Clone, Copy)]
pub(super) struct Asked<'a> {
    pub base: Option<&'a str>,
    pub cited: CitedTests,
}

/// The project at `root`, then, unless pages are named, each of its members, and theirs.
pub(super) fn check_projects(args: &[String], root: &Path, asked: Asked) -> Result<Checked> {
    let (top, config) = check_project(args, root, &Launch::Found, asked)?;
    let members =
        if args.is_empty() { check_members(root, None, &config.members, asked)? } else { Vec::new() };
    Ok(Checked { top, members })
}

/// Only the configured member named by `--member <dir>`, using its configured adapter launch,
/// plus its own sub-members when no pages are named.
fn check_selected_member(args: &[String], root: &Path, wanted: &str, asked: Asked) -> Result<Checked> {
    let target = resolve_member(root, wanted)?;
    let in_member = |why: Error| why.within(format!("member {}", target.path));
    let (top, config) = check_project(args, &target.dir, &target.launch, asked).map_err(in_member)?;
    let members = if args.is_empty() {
        check_members(&target.dir, Some(&target.path), &config.members, asked)?
    } else {
        Vec::new()
    };
    Ok(Checked { top, members })
}

#[derive(Debug)]
struct FoundMember {
    path: String,
    dir: PathBuf,
    launch: Launch,
}

fn resolve_member(root: &Path, wanted: &str) -> Result<FoundMember> {
    let config = load_config(&DiskFiles::new(root.to_path_buf()))?;
    if config.members.is_empty() {
        return Err(Error::user("no members are configured in docattest.config.json"));
    }
    let wanted_norm = normalise_member(wanted);
    let mut configured = Vec::new();
    if let Some(found) = find_member(root, None, &config.members, &wanted_norm, &mut configured)? {
        return Ok(found);
    }
    Err(Error::user(format!("unknown member \"{wanted}\" (configured: {})", configured.join(", "))))
}

fn find_member(
    dir: &Path,
    prefix: Option<&str>,
    members: &[Member],
    wanted: &str,
    configured: &mut Vec<String>,
) -> Result<Option<FoundMember>> {
    for member in members {
        let path = prefix.map_or_else(|| member.path.clone(), |above| format!("{above}/{}", member.path));
        configured.push(path.clone());
        let launch = Launch::of_member(member.adapter_command.as_deref(), dir);
        let in_member = |why: Error| why.within(format!("member {path}"));
        if path == wanted {
            let member_dir = below(dir, &member.path).map_err(in_member)?;
            return Ok(Some(FoundMember { path, dir: member_dir, launch }));
        }
        if let Ok(member_dir) = below(dir, &member.path)
            && let Ok(sub_cfg) = load_config(&DiskFiles::new(member_dir.clone()))
            && let Some(found) = find_member(&member_dir, Some(&path), &sub_cfg.members, wanted, configured)?
        {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

fn normalise_member(raw: &str) -> String {
    let mut s = raw.replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    s.trim_end_matches('/').to_string()
}

/// Each member of the project in `dir` (at `path` from the top, the top when None), each
/// followed by its own members. Members share nothing and each has its own adapter, so they
/// are checked at the same time; the reports, and the first problem, keep the listed order.
fn check_members(
    dir: &Path,
    path: Option<&str>,
    members: &[Member],
    asked: Asked,
) -> Result<Vec<MemberReport>> {
    std::thread::scope(|scope| {
        let checks: Vec<_> = members
            .iter()
            .map(|member| scope.spawn(move || check_member(dir, path, member, asked)))
            .collect();
        let mut out = Vec::new();
        for check in checks {
            out.extend(check.join().map_err(|_| Error::internal("a member's check panicked"))??);
        }
        Ok(out)
    })
}

/// One member's report, then its own members'.
fn check_member(dir: &Path, path: Option<&str>, member: &Member, asked: Asked) -> Result<Vec<MemberReport>> {
    let path = path.map_or_else(|| member.path.clone(), |above| format!("{above}/{}", member.path));
    let launch = Launch::of_member(member.adapter_command.as_deref(), dir);
    let in_member = |why: Error| why.within(format!("member {path}"));
    let member_dir = below(dir, &member.path).map_err(in_member)?;
    let (report, config) = check_project(&[], &member_dir, &launch, asked).map_err(in_member)?;
    let below_it = check_members(&member_dir, Some(&path), &config.members, asked)?;
    Ok(std::iter::once(MemberReport { path, adapter: config.adapter, report }).chain(below_it).collect())
}

/// The directory of a member, which must be below `dir`: a link that leads back up would
/// check the same projects for ever.
fn below(dir: &Path, member: &str) -> Result<PathBuf> {
    let joined = dir.join(member);
    let (Ok(inner), Ok(outer)) = (joined.canonicalize(), dir.canonicalize()) else {
        return Err(Error::user(format!("there is no directory {}", joined.display())));
    };
    if inner == outer || !inner.starts_with(&outer) {
        return Err(Error::user(format!("{} leads out of {}", joined.display(), dir.display())));
    }
    Ok(joined)
}

/// One project's report, and its config.
fn check_project(
    args: &[String],
    root: &Path,
    launched: &Launch,
    asked: Asked,
) -> Result<(Report, ProjectConfig)> {
    let pages: Vec<String> = args.iter().map(|given| page_argument(root, given)).collect();
    let (state, reading) = load_with_base(root, launched, asked.base)?;
    let selected = if pages.is_empty() {
        state.living.iter().cloned().collect()
    } else {
        select_pages(&pages, &state.living, &state.frozen)?
    };
    // The base is found first, so a bad ref or an invalid base config fails before anything
    // runs. Only its files are still being read while the check runs.
    if let Some(reading) = &reading {
        reading.found()?;
    }
    let checked = run_check(&state, network(), &selected, asked.cited);
    let report = with_base_changes(checked, reading, &state)?;
    let config = state.config.clone();
    finish(state)?;
    Ok((report, config))
}

/// A page as `check` was given it: as written when relative, else made relative to the
/// project. An absolute path and the project's root are both followed through symbolic
/// links first (macOS's `/var` is `/private/var`), so the path names its page however its
/// directories were reached.
pub(super) fn page_argument(root: &Path, given: &str) -> String {
    if !Path::new(given).has_root() {
        return given.to_string();
    }
    let real = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    relative(&real(root), &real(Path::new(given)).to_string_lossy())
}

/// Loads the project; with a base, git reads it from here on, while the project loads and
/// while the check runs. Its files are read once the adapter has said which names are never
/// the project's own.
fn load_with_base(
    root: &Path,
    launched: &Launch,
    base: Option<&str>,
) -> Result<(State, Option<BaseReading>)> {
    let started = launch(DiskFiles::new(root.to_path_buf()), launched, Start::Warm)?;
    let reading = base.map(|reference| crate::changes::read_base(root, reference));
    let greeted = started.greet()?;
    if let Some(reading) = &reading {
        reading.ignore(&greeted.adapter.ignored);
    }
    Ok((greeted.load(now_ms())?, reading))
}

/// The check's report, with what the head did to the base's attestations when there is a
/// base. The base is loaded before an error of the check is reported.
fn with_base_changes(checked: Result<Report>, reading: Option<BaseReading>, state: &State) -> Result<Report> {
    let base = match reading {
        Some(reading) => Some(crate::changes::load_base(state, reading)?),
        None => None,
    };
    let mut report = checked?;
    if let Some(base) = &base {
        crate::changes::with_changes(&mut report, base, state)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;

    #[test]
    fn a_member_is_a_directory_below_the_project_and_its_problems_name_it() {
        let top = Temp::new("members");
        top.write("core/docattest.config.json", "{ not a config");
        assert_eq!(below(&top.0, "core").unwrap(), top.0.join("core"));
        let missing = below(&top.0, "gone").unwrap_err().to_string();
        assert_eq!(missing, format!("docattest: there is no directory {}", top.0.join("gone").display()));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&top.0, top.0.join("loop")).unwrap();
            assert!(below(&top.0, "loop").unwrap_err().to_string().contains("leads out of"));
        }
        let member = Member { path: "core".into(), adapter_command: None };
        let asked = Asked { base: None, cited: CitedTests::Declared };
        let said = check_members(&top.0, None, &[member], asked).unwrap_err().to_string();
        assert!(
            said.starts_with("docattest: member core: docattest.config.json is not valid JSON"),
            "{said}"
        );
    }

    #[test]
    fn resolve_member_finds_direct_and_nested_members_and_rejects_unknown_ones() {
        let top = Temp::new("resolve-member");
        top.write(
            "docattest.config.json",
            r#"{"pages":["README.md"],"adapter":"none","members":["packages/a"]}"#,
        );
        top.write(
            "packages/a/docattest.config.json",
            r#"{"pages":["README.md"],"adapter":"none","members":["sub"]}"#,
        );
        top.write("packages/a/sub/docattest.config.json", r#"{"pages":["README.md"],"adapter":"none"}"#);

        assert_eq!(resolve_member(&top.0, "./packages/a/").unwrap().path, "packages/a");
        assert_eq!(resolve_member(&top.0, "packages/a/sub").unwrap().path, "packages/a/sub");
        let missing = resolve_member(&top.0, "nope").unwrap_err().to_string();
        assert!(
            missing.contains("unknown member \"nope\" (configured: packages/a, packages/a/sub)"),
            "{missing}"
        );

        let solo = Temp::new("no-members");
        solo.write("docattest.config.json", r#"{"pages":["README.md"],"adapter":"none"}"#);
        let none = resolve_member(&solo.0, "packages/a").unwrap_err().to_string();
        assert!(none.contains("no members are configured"), "{none}");
    }
}
