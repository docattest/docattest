/** A surface as written in the config: a bare specifier, a `./path`, or both. */
export type SurfaceEntry = string | { import: string; path: string };

/** The contents of `docattest.config.json`. */
export interface Config {
  /** Globs of pages to check. */
  pages: string[];
  /** Globs of pages that are listed but never parsed. */
  frozen?: string[] | undefined;
  /** Globs of source files whose doc comments may hold examples. */
  sources?: string[] | undefined;
  /** Surfaces by name; `default` is required. */
  surfaces: Record<string, SurfaceEntry>;
  /** The setup module: case runners and named facts. */
  setup?: string | undefined;
  snapshots?: { dir: string; maxAgeDays?: number | undefined } | undefined;
  adapter: 'typescript';
  runtime?: { timeoutMs?: number | undefined; workers?: number | undefined } | undefined;
  /** Info-string words added by a docs renderer (e.g. `twoslash`), accepted and ignored. */
  ignoreTags?: string[] | undefined;
  /** Projects below this one that `check` checks with it, each with a config of its own. */
  members?: Member[] | undefined;
  /**
   * The programs the console blocks run, by the name a command line calls each, with globs
   * of the files each is built from: `impact` lists the blocks that run one whose files changed.
   */
  commands?: Record<string, string[]> | undefined;
  /**
   * By surface, globs of the symbol names that are not meant to be documented: `uncovered`
   * leaves out those no page attests, and the report counts them apart.
   */
  undocumented?: Record<string, string[]> | undefined;
}

/**
 * A member: its directory, below the project's, and perhaps the command that starts its
 * adapter (program and arguments, read from the project's directory).
 */
export type Member = string | { path: string; adapterCommand?: string[] | undefined };
