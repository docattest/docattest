/** The kinds of attestation a result can carry, as the protocol's `Result` schema has them. */
export const KINDS = ['example', 'fact', 'cases', 'snapshot', 'test'] as const;

/** What a result attests: one of {@link KINDS}. */
export type Kind = (typeof KINDS)[number];

/** The verdicts a result can carry, as the protocol's `Result` schema has them. */
export const STATUSES = ['pass', 'fail', 'error', 'skipped'] as const;

/** A result's verdict: one of {@link STATUSES}. */
export type Status = (typeof STATUSES)[number];

/**
 * What `check --base` can find a change did to the attestations. The core owns these; a test
 * holds this list to what it reports (test/unit/api/constants.test.ts).
 */
export const CHANGE_TYPES = ['orphaned-claim', 'frozen-page', 'downgrade', 'weakened'] as const;

/** One of {@link CHANGE_TYPES}. */
export type ChangeType = (typeof CHANGE_TYPES)[number];

/** Subcommands of the `docattest` command line. */
export const COMMANDS = [
  'check',
  'where',
  'uncovered',
  'index',
  'refresh',
  'skill',
  'audit',
  'inventory',
  'impact',
  'hooks',
  'agents',
  'conformance',
] as const;

/**
 * The keys a `docattest.config.json` may have, in the order the core lists them. The core owns
 * this list (`KEYS` in core/src/config/mod.rs) and refuses any other key; a test holds this copy
 * to the keys it names when it does.
 */
export const CONFIG_KEYS = [
  '$schema',
  'pages',
  'frozen',
  'sources',
  'surfaces',
  'setup',
  'snapshots',
  'adapter',
  'runtime',
  'ignoreTags',
  'members',
  'commands',
  'undocumented',
] as const;

/**
 * Time budget for one block, one fact or one table row, in milliseconds. The core owns this
 * default (`DEFAULT_TIMEOUT_MS` in core/src/config/mod.rs); a test holds this value to the
 * one it sends the adapter.
 */
export const DEFAULT_TIMEOUT_MS = 10_000;

/**
 * Age after which a snapshot is reported as stale, in days. The core owns this default
 * (`DEFAULT_SNAPSHOT_MAX_AGE_DAYS` in core/src/config/mod.rs); a test holds this value to
 * the age at which it calls a snapshot old.
 */
export const DEFAULT_SNAPSHOT_MAX_AGE_DAYS = 90;
