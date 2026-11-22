//! The command line's help text, word for word as the recordings hold it.

/// The usage text.
pub const USAGE: &str = r#"usage: docattest <command> [options]

Workflow:
  init                            detect language, pages and default surface, write
                                  docattest.config.json, and run an initial gap audit.
                                  --force, --dry-run, --no-audit, --json
                                  --agents: write the docattest section of AGENTS.md
                                  --hooks [--local]: install coding-agent hooks (--agent)
                                  --git-hook: install a git pre-commit hook
                                  --skill [--global]: install the language-specific agent skill
  check [pages or globs...]       run attestations (all living pages, or just these);
                                  exit 1 on fail or error. --json, --quiet
                                  without pages, also each project the config lists as
                                  members, with its own adapter
                                  --member <dir>: check only that member project (and
                                  optionally named pages inside it)
                                  --base <ref>: judge what the change since the git ref
                                  did to the attestations (deleted, frozen, downgraded,
                                  weakened); exit 1 on an orphaned claim
                                  --report <format>=<path>: write a CI report (junit or
                                  codequality); once for each file
                                  --run-cited: also run the test each citation names
  audit                           lists, tables and blocks that look copied from a surface
                                  and carry no directive; advisory, exit 0 (1 when a
                                  surface cannot load). --json
                                  --apply: insert match directives into pages
                                  --include-drift: with --apply, also insert drift directives
                                  --dry-run: with --apply, show edits without writing files
                                  --inventory [text]: list observable values a fact could read
  impact [--base <ref>]           symbols the diff from <ref> (default HEAD) affects,
                                  and the pages, lines and sentences that name them. --json
                                  --file <path>: asked after editing that file; prints
                                  nothing unless it is a source file and something is affected

Inspection:
  explain <page>[:<line>]         what each directive on the line, or on the page, binds
                                  and why, the sentence it defends, and what checking it
                                  gives. --json
  where [symbol]                  pages, lines and claims that attest a symbol, or the
                                  whole symbol → references map when no symbol is given. --json
  uncovered [--surface <name>]    public symbols of a surface no page attests. --json

Maintenance:
  refresh [name]                  refetch snapshots from their source and rewrite them
  conformance run <kit.json> -- <adapter command>
                                  hold a language adapter to the adapter protocol
                                  (also: conformance record, conformance validate)

Options:
  --root <dir>                    project root (default: nearest directory at or above
                                  cwd holding docattest.config.json)
  --agent <name or file>          the coding agent: a shipped descriptor by name, or a
                                  descriptor file of your own (docs/agents.md)
  --version                       the version; with --json, also the adapter protocol spoken"#;

/// Focused help for a single subcommand (`docattest <cmd> --help` / `docattest help <cmd>`),
/// falling back to the top-level usage when no known command is named.
pub fn help_for(command: Option<&str>) -> &'static str {
    match command {
        Some("init") => INIT_HELP,
        Some("check") => CHECK_HELP,
        Some("audit") => AUDIT_HELP,
        Some("impact") => IMPACT_HELP,
        Some("explain") => EXPLAIN_HELP,
        Some("where") => WHERE_HELP,
        Some("uncovered") => UNCOVERED_HELP,
        Some("refresh") => REFRESH_HELP,
        Some("conformance") => CONFORMANCE_HELP,
        _ => USAGE,
    }
}

const INIT_HELP: &str = r#"usage: docattest init [options]

Detect language, pages and default surface, write docattest.config.json, run an initial
gap audit, and optionally wire coding-agent or git integrations.

Options:
  --agents                 write or update the docattest section of AGENTS.md
  --hooks                  install coding-agent hooks (impact --hook, check --hook)
  --local                  with --hooks, write to the agent's untracked local settings file
  --git-hook               install a git pre-commit hook running `docattest check --quiet`
  --skill                  install a lean, single-language authoring skill (SKILL.md)
  --global                 with --skill, install in the user's home directory
  --agent <name or file>   target a specific coding agent descriptor (with --hooks / --skill)
  --force                  overwrite an existing docattest.config.json (when no integration
                           flags are given) or replace an edited SKILL.md (with --skill;
                           an existing docattest.config.json is kept when integration flags
                           are passed)
  --dry-run                preview config and integration changes without writing any files
  --no-audit               skip the initial gap audit after creating docattest.config.json
  --json                   print structured JSON output
  --root <dir>             project root directory

Examples:
  docattest init
  docattest init --agents --hooks --local --skill
  docattest init --agents --hooks --skill --force --dry-run"#;

const CHECK_HELP: &str = r#"usage: docattest check [pages or globs...] [options]

Run attestations on all living pages (and configured members when no pages are named),
or only the named pages/globs. Exits 0 when everything holds, 1 on fail or error.

Options:
  --quiet                     print only failures, warnings, stale snapshots, and verdict
  --json                      print the full report as JSON
  --member <dir>              check only that configured member project (and optional pages)
  --base <ref>                compare against git <ref> for deleted/frozen/downgraded/weakened
                              proofs; exits 1 on orphaned claims
  --run-cited                 also run the tests named by <!-- test: ... --> citations
  --report <format>=<path>    write a CI report (junit or codequality); repeatable
  --hook                      run as a coding-agent turn-end hook (reads JSON event on stdin)
  --agent <name or file>      coding agent descriptor (with --hook)
  --root <dir>                project root directory

Examples:
  docattest check --quiet
  docattest check docs/retries.md
  docattest check --member packages/api README.md
  docattest check --base origin/main --run-cited"#;

const AUDIT_HELP: &str = r#"usage: docattest audit [options]
       docattest audit --inventory [text] [options]

Scan pages for lists, tables, blocks and values that look copied from a surface without a
directive. Advisory: exits 0 regardless of findings (1 if a surface fails to load).

Options:
  --apply            insert matching directives directly into .md and .mdx pages
  --include-drift    with --apply, also insert directives on drifted claims (without editing prose)
  --dry-run          with --apply, preview edits without writing files
  --inventory [text] list observable values a fact could read (optionally filtered by text)
  --json             print structured JSON output
  --root <dir>       project root directory

Examples:
  docattest audit
  docattest audit --apply --dry-run
  docattest audit --inventory timeout"#;

const IMPACT_HELP: &str = r#"usage: docattest impact [options]

List documented symbols affected by changes since <ref> (default HEAD) and the pages,
lines and sentences that prove or mention them.

Options:
  --base <ref>             git ref to diff against (default: HEAD)
  --file <path>            check impact of a single edited file; silent if unaffected
  --hook                   run as a coding-agent post-edit hook (reads JSON event on stdin)
  --agent <name or file>   coding agent descriptor (with --hook)
  --json                   print structured JSON output
  --root <dir>             project root directory"#;

const EXPLAIN_HELP: &str = r#"usage: docattest explain <page>[:<line>] [options]

Show what each directive on a page (or line) binds and why, the sentence it defends, and
the result of checking it.

Options:
  --json         print structured JSON output
  --root <dir>   project root directory"#;

const WHERE_HELP: &str = r#"usage: docattest where [symbol] [options]

With <symbol>, print the pages, lines and claims that attest it. Without a symbol, print
the entire symbol → references map.

Options:
  --json         print structured JSON output
  --root <dir>   project root directory"#;

const UNCOVERED_HELP: &str = r#"usage: docattest uncovered [options]

List public symbols of a surface that no living page attests (excluding symbols matched
by `undocumented` globs; internal surfaces return empty).

Options:
  --surface <name>   surface to inspect (default: default)
  --json             print a JSON array of symbol names
  --root <dir>       project root directory"#;

const REFRESH_HELP: &str = r#"usage: docattest refresh [name] [options]

Refetch all snapshots (or the named snapshot) from their configured `source` URLs and
rewrite the snapshot files with today's date.

Options:
  --root <dir>   project root directory"#;

const CONFORMANCE_HELP: &str = r#"usage: docattest conformance run <kit.json> -- <adapter command>
       docattest conformance record <log> -- <adapter command>
       docattest conformance validate <log>...

Test a language adapter against the docattest adapter protocol and JSON schemas."#;
