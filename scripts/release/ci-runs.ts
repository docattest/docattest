// What CI's runs of one commit say about publishing it: passed, failed, still running, or
// never run. Kept apart from asking GitHub, so the rule is tested without a network.

/** A run of the CI workflow, as GitHub's API describes it. */
export interface CiRun {
  status: string;
  conclusion: string | null;
}

/** What the runs of a commit say. */
export type CiVerdict = 'passed' | 'failed' | 'running' | 'none';

/**
 * A commit has passed when one of its CI runs succeeded, so a run re-run after a flaky
 * failure counts. It has failed when every run finished and none succeeded, and is still
 * running while one has not finished.
 */
export function ciVerdict(runs: readonly CiRun[]): CiVerdict {
  if (runs.some((run) => run.status === 'completed' && run.conclusion === 'success')) {
    return 'passed';
  }
  if (runs.length === 0) return 'none';
  return runs.some((run) => run.status !== 'completed') ? 'running' : 'failed';
}
