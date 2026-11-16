// The bench's committed baseline: which development scenarios passed when it was last
// recorded. A run fails when a scenario that passed then fails now, so a change cannot
// quietly lose a detection; one that starts passing is reported, to be recorded.
import { readFileSync, writeFileSync } from 'node:fs';

/** The ids of the scenarios that passed, sorted. */
export interface Baseline {
  passing: string[];
}

/** A scenario's id and whether its expectation held in this run. */
export interface Ran {
  id: string;
  pass: boolean;
}

/** How a run differs from the baseline, over the scenarios it ran. */
export interface Drift {
  regressed: string[];
  improved: string[];
}

/** Compare a run with the baseline. Scenarios the run did not include are not judged. */
export function compareWithBaseline(baseline: Baseline, ran: readonly Ran[]): Drift {
  const before = new Set(baseline.passing);
  return {
    regressed: ran.filter((r) => !r.pass && before.has(r.id)).map((r) => r.id),
    improved: ran.filter((r) => r.pass && !before.has(r.id)).map((r) => r.id),
  };
}

export function readBaseline(file: string): Baseline {
  const parsed: unknown = JSON.parse(readFileSync(file, 'utf8'));
  const passing: unknown = (parsed as { passing?: unknown } | null)?.passing;
  if (!Array.isArray(passing) || !passing.every((id) => typeof id === 'string')) {
    throw new Error(`${file}: expected { "passing": [<scenario id>, ...] }`);
  }
  return { passing };
}

export function writeBaseline(file: string, ran: readonly Ran[]): void {
  const passing = ran
    .filter((r) => r.pass)
    .map((r) => r.id)
    .sort();
  writeFileSync(file, `${JSON.stringify({ passing }, null, 2)}\n`);
}
