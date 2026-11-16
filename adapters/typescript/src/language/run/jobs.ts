import { attempt, type AttemptEnd } from './attempt.js';
import type { WorkerPool } from './pool.js';
import type { ItemOutcome, Job } from './protocol.js';

/** What happened to one item of a job. */
export type ItemResult =
  | { kind: 'outcome'; outcome: ItemOutcome }
  | { kind: 'timeout' }
  | { kind: 'lost'; message: string };

/** A job's items, or why it could not start. */
export interface JobResult {
  /** Set when preparation (loading the surface or setup module) failed; nothing ran. */
  failure?: { message: string; timedOut: boolean };
  items: ItemResult[];
  strays: string[];
}

/**
 * Run a job of `count` items to the end. When an item times out or kills its worker, it is
 * recorded and the job resumes from the next item on a fresh worker.
 */
export async function runJob(
  pool: WorkerPool,
  job: Job,
  count: number,
  timeoutMs: number,
): Promise<JobResult> {
  const result: JobResult = { items: [], strays: [] };
  const { items } = result;
  for (;;) {
    const { outcomes, strays, end } = await attempt(pool, job, items.length, timeoutMs);
    result.strays.push(...strays);
    items.push(...outcomes.map((outcome) => ({ kind: 'outcome' as const, outcome })));
    if (end.kind === 'done') return result;
    if (end.kind === 'failed' || !end.prepared) return abandoned(result, end, count, timeoutMs);
    if (items.length >= count) return result;
    items.push(
      end.kind === 'timeout' ? { kind: 'timeout' } : { kind: 'lost', message: end.message },
    );
    if (items.length >= count) return result;
  }
}

/**
 * The job, once an attempt failed or could not prepare: when nothing ran, the job failed;
 * when a resumed attempt could not prepare again, the remaining items never ran.
 */
function abandoned(
  result: JobResult,
  end: Exclude<AttemptEnd, { kind: 'done' }>,
  count: number,
  timeoutMs: number,
): JobResult {
  const message = end.kind === 'timeout' ? `timed out after ${timeoutMs} ms` : end.message;
  if (result.items.length === 0) {
    return { failure: { message, timedOut: end.kind === 'timeout' }, ...result };
  }
  while (result.items.length < count) result.items.push({ kind: 'lost', message });
  return result;
}
