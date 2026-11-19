import type { ChangeType, Kind, Status } from './constants.js';

/** The verdict on one attestation (or one row of a cases table). */
export interface Result {
  page: string;
  line: number;
  kind: Kind;
  status: Status;
  /** Surface symbols the attestation exercises. */
  symbols: string[];
  /** The page's side of a comparison, when there was one. */
  expected?: string;
  /** The code's side of a comparison, when there was one. */
  actual?: string;
  message: string;
}

/** Page coverage: which living pages carry attestations. */
export interface Coverage {
  living: number;
  covered: number;
  uncovered: string[];
  /**
   * Living pages marked as having nothing to prove (`prose` in the config, or frontmatter
   * `docattest: prose`) that carry no attestation. Only when there are some.
   */
  prose?: string[];
  frozen: string[];
  /** Results on living pages, one per attestation or row of a cases table: `proofs + cited`. */
  attestations: number;
  /** The attestations docattest runs or evaluates itself: every one that is not a citation. */
  proofs: number;
  /** Proofs that are `check` examples asserting nothing. */
  weak: number;
  /** `test` citations, broken ones too: sentences tied to a test that docattest does not run. */
  cited: number;
  /** With `check --run-cited`, the citations whose test ran, whatever it gave; absent otherwise. */
  citedRun?: number;
}

/** Where a surface resolved: the file examples and facts actually ran against. */
export interface SurfaceResolution {
  /** The specifier examples and facts import. */
  import: string;
  /** The entry file, relative to the project root when it lies inside it. */
  entry: string;
  /**
   * The package the entry file belongs to. `display` is how the language writes a package
   * and its version (`name@1.2.0` here); the text report prints it as given.
   */
  package?: { name: string; version?: string; display: string };
}

/** One surface in the report: symbol coverage and resolution. */
export interface SurfaceCoverage {
  symbols: number;
  attested: number;
  /**
   * Exported names no attestation against this surface references, less those an
   * `undocumented` glob matches.
   */
  unattested: string[];
  /**
   * Exported names no attestation references that an `undocumented` glob matches. Only when
   * the config gives the surface such a glob.
   */
  undocumented?: string[];
  /** Absent when the surface could not be resolved. */
  resolution?: SurfaceResolution;
  /** Problems worth a look that never change the exit code. */
  warnings: string[];
}

/** The age of one snapshot. */
export interface SnapshotStatus {
  name: string;
  source: string;
  fetchedAt: string;
  ageDays: number;
  /** Older than `snapshots.maxAgeDays`. */
  warning: boolean;
}

/** Everything `check` found. The command line prints it, or with `--json`, this object. */
export interface Report {
  /** Ordered by page, then line. */
  results: Result[];
  coverage: Coverage;
  /** Keyed by surface name. */
  surfaces: Record<string, SurfaceCoverage>;
  snapshots: SnapshotStatus[];
  /**
   * What the config names to no effect: a glob that matches no file, a setup module that is
   * not there or that nothing reads. Never changes the exit code. Only when there is one.
   */
  warnings?: string[];
  /**
   * 1 when any result is `fail` or `error`, any change is a `fail`, or any member's report
   * has exit code 1.
   */
  exitCode: 0 | 1;
  /** What the working tree did to the attestations of the base. Only with `base`. */
  changes?: Change[];
  /** The reports of the projects the config lists as `members`. Only when it lists any. */
  members?: MemberReport[];
}

/** The report of a member project. */
export interface MemberReport {
  /** Its directory, from the project that lists it. */
  path: string;
  /** The adapter its config names. */
  adapter: string;
  report: Report;
}

/** One thing a change did to the attestations that existed at the base. */
export interface Change {
  type: ChangeType;
  /** The page as it is now. */
  page: string;
  /** The line now: the claim's, the block's or the fact's; 1 for a whole page. */
  line: number;
  /** `fail` sets exit code 1; a `warning` never changes it. */
  severity: 'fail' | 'warning';
  message: string;
}

/** One place a symbol is attested, with the sentence the proof defends. */
export interface Reference {
  page: string;
  line: number;
  kind: Kind;
  claim: string;
}
