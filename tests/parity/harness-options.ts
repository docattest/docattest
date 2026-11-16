// The options every parity harness shares: `--candidate`, `--update`, `--recordings`,
// `--only`, and `--source`, what a recording written now says it was recorded from. A
// harness declares them with its own (HARNESS_OPTIONS spread into its parseArgs options) and
// reads them with `harnessOptions`.

/** The shared options, as Node's parseArgs declares them. */
export const HARNESS_OPTIONS = {
  candidate: { type: 'string' },
  update: { type: 'boolean' },
  recordings: { type: 'string' },
  only: { type: 'string' },
  source: { type: 'string' },
} as const;

/** The shared options' values, as parseArgs reads them. */
export interface HarnessValues {
  candidate?: string | undefined;
  update?: boolean | undefined;
  recordings?: string | undefined;
  only?: string | undefined;
  source?: string | undefined;
}

/** The shared options, settled. */
export interface HarnessOptions {
  /** The core's command line, split at spaces. */
  candidate: string[];
  update: boolean;
  dir: string;
  only: string | undefined;
  source: string;
}

/** The shared options, with `defaultDir` for the recordings when `--recordings` is not given. */
export function harnessOptions(values: HarnessValues, defaultDir: string): HarnessOptions {
  const candidate = (values.candidate ?? '').split(' ').filter(Boolean);
  if (!candidate.length) throw new Error('pass --candidate "<command line>"');
  const update = values.update ?? false;
  return {
    candidate,
    update,
    dir: values.recordings ?? defaultDir,
    only: values.only,
    source: recordedFrom(values.source, update),
  };
}

/**
 * What a recording written now says it was recorded from: `--source`, which `--update`
 * requires, because the header is committed and should say what changed (never a path of
 * the machine it was recorded on).
 */
export function recordedFrom(source: string | undefined, update: boolean): string {
  if (update && source === undefined) {
    throw new Error('pass --source "<what changed>" with --update; it heads every recording');
  }
  return source ?? 'the docattest core';
}
