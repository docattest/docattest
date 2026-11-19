import type { AuditReport, InventoryEntry } from './audit.js';
import type { Explanation } from './explain.js';
import type { Impact, ImpactOptions } from './impact.js';
import type { MutateOptions, Mutation } from './mutate.js';
import type { Reference, Report } from './report.js';

/**
 * Where a project's files come from, and the environment checks run in. Every operation runs
 * the docattest core on the project: a project in memory is written to a temporary directory
 * for the length of each operation, since the core reads projects from disk.
 */
export interface ProjectInput {
  /** A directory on disk. Mutually exclusive with `files`. */
  root?: string | undefined;
  /**
   * An in-memory project: path → contents. Must include docattest.config.json. Surfaces
   * given as paths are imported as `docattest-fixture` when it holds no `package.json` with
   * a name.
   */
  files?: Record<string, string> | undefined;
  /**
   * Overrides for the environment the checks run in: whether `network` blocks run (else as
   * `DOCATTEST_NETWORK=1` says), and the clock snapshot ages and `refresh` are measured by
   * (`DOCATTEST_NOW` for the core).
   */
  env?: { network?: boolean | undefined; now?: Date | undefined } | undefined;
}

/** Options for one check. */
export interface CheckOptions {
  /** Paths or globs, relative to the project root, of the pages to run. Default: all. */
  pages?: readonly string[] | undefined;
  /**
   * Also judge what the project did to the attestations of a base: a git ref (read from git,
   * for a project on disk), or the base project's files (for an in-memory project; they are
   * committed with git in its temporary directory). Adds `changes` to the report.
   */
  base?: string | { files: Record<string, string> } | undefined;
}

/** A project: its pages, its surfaces and the operations over them. */
export interface Project {
  /** Run attestations and report. Coverage always describes the whole project. */
  check(options?: CheckOptions): Promise<Report>;
  /** Pages, lines and claims that attest a symbol. Parsing only. */
  where(symbol: string): Promise<Reference[]>;
  /**
   * Exports of a surface that no attestation against it references, less those the config's
   * `undocumented` says are not meant to be documented.
   */
  uncovered(surface?: string): Promise<string[]>;
  /** The whole symbol → references map. Parsing only. */
  index(): Promise<Record<string, Reference[]>>;
  /**
   * What each directive on a page, or on one line of it, binds and why, the sentence it
   * defends, and what checking it gives. Rejects when the page or line holds no directive.
   */
  explain(page: string, line?: number): Promise<Explanation>;
  /** The exports a change since `base` affects, and the prose that names them. Reads only. */
  impact(options?: ImpactOptions): Promise<Impact>;
  /**
   * Each proof run again against changes to what it reads (a literal of the code, a line of a
   * file a fact reads, a cell of a table): killed when one makes it fail, survived when none
   * does, not mutated when nothing it reads could be changed. The project is never written:
   * code is changed in copies of it.
   */
  mutate(options?: MutateOptions): Promise<Mutation>;
  /**
   * Refetch snapshots from their source over HTTP and rewrite them; returns the names
   * refreshed. The core does the fetching: there is no fetcher to pass.
   */
  refresh(name?: string): Promise<string[]>;
  /** Literals on living pages that look copied from a surface and carry no directive. */
  audit(): Promise<AuditReport>;
  /** The values a fact could read, optionally only those containing `search`. */
  inventory(search?: string): Promise<InventoryEntry[]>;
  /** Files the project has written. Empty after any number of checks. */
  writes(): string[];
}
