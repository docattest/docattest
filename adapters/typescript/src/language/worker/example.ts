import assert from 'node:assert/strict';
import type { ExampleJob, Verdict } from '../run/protocol.js';
import { freshRegistry, type WorkerContext } from './context.js';
import { describeError, lineIn } from './describe.js';
import { offline } from './network.js';

/** Run one example block in a fresh registry, with `assert` in scope. */
export async function runExample(job: ExampleJob, ctx: WorkerContext): Promise<Verdict> {
  const registry = freshRegistry(ctx, { assert });
  const run = () => registry.runBlock(job.file, job.code);
  try {
    const { strays } = await ctx.strays.during(() => (job.offline ? offline(run) : run()));
    if (strays.length) {
      return {
        status: 'fail',
        message: `unhandled error from attested code: ${strays.join('; ')}`,
      };
    }
    const message = job.weak
      ? 'ran without throwing, but asserts nothing (weak)'
      : 'all assertions held';
    return { status: 'pass', message };
  } catch (err) {
    const failure = describeError(err, lineIn(err, job.file, job.lines), job.source);
    return { status: 'fail', ...failure };
  }
}
