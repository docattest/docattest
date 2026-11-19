//! The command line's help text, word for word as the recordings hold it.

/// The usage text.
pub const USAGE: &str = r#"usage: docattest <command> [options]

  check [pages or globs...]       run attestations (all living pages, or just these);
                                  exit 1 on fail or error. --json, --quiet
                                  without pages, also each project the config lists as
                                  members, with its own adapter
                                  --base <ref>: also judge what the change since the
                                  git ref did to the attestations (deleted, frozen,
                                  downgraded, weakened); exit 1 on an orphaned claim
                                  --report <format>=<path>: also write the report to a
                                  file for a CI service, junit or codequality (GitLab);
                                  once for each file
  where <symbol>                  pages, lines and claims that attest a symbol. --json
  uncovered [--surface <name>]    public symbols of a surface no page attests. --json
  index                           the whole symbol → references map (parsing only). --json
  explain <page>[:<line>]         what each directive on the line, or on the page, binds
                                  and why, the sentence it defends, and what checking it
                                  gives. --json
  refresh [name]                  refetch snapshots from their source and rewrite them
  audit                           lists, tables and blocks that look copied from a surface
                                  and carry no directive; advisory, exit 0 (1 when a
                                  surface cannot load). --json
  inventory [text]                values a fact could read, each with its expression;
                                  with text, only those containing it. --json
  skill install                   install the agent skill where the agent looks for it
                                  --agent, --global for the home directory instead of the
                                  project, --force to replace a changed file
  impact [--base <ref>]           symbols the diff from <ref> (default HEAD) affects,
                                  and the pages, lines and sentences that name them. --json
                                  --file <path>: asked after editing that file; prints
                                  nothing unless it is a source file and something is affected;
                                  exit 1 when the path names no file of the project
  hooks install                   add hooks to a coding agent's settings: impact after each
                                  edit, check before the agent stops. --agent
                                  --git: a git pre-commit hook that runs check instead
  agents install                  write the docattest section of AGENTS.md: what any coding
                                  agent runs after an edit and before it finishes
  conformance run <kit.json> -- <adapter command>
                                  hold a language adapter to the adapter protocol;
                                  also: conformance record <log> -- <adapter command>,
                                  which notes every line both ways in <log>, and
                                  conformance validate <log...>, which holds what was
                                  noted to the protocol's schemas

  --root <dir>                    project root (default: the nearest directory at or above
                                  the current one that holds a docattest.config.json)
  --agent <name or file>          the coding agent: a shipped descriptor by name, or a
                                  descriptor file of your own (docs/agents.md)
  --version                       the version; with --json, also the adapter protocol spoken"#;
