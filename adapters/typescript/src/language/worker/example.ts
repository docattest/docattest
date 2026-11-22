import assert from 'node:assert/strict';
import type { ExampleJob, Verdict } from '../run/protocol.js';
import { compareOutput, expectedOutput } from '../syntax/output.js';
import { capturingConsole } from './console.js';
import { freshRegistry, type WorkerContext } from './context.js';
import { describeError, lineIn } from './describe.js';
import { offline } from './network.js';

/** Run one example block in a fresh registry, with `assert` in scope. */
export async function runExample(job: ExampleJob, ctx: WorkerContext): Promise<Verdict> {
  const want = expectedOutput(job.source);
  const chunks: string[] = [];
  const globals: Record<string, unknown> =
    want === undefined
      ? { assert }
      : { assert, console: capturingConsole((line) => chunks.push(line)) };
  const registry = freshRegistry(ctx, globals);
  const run = () =>
    want === undefined
      ? registry.runBlock(job.file, job.code)
      : captureStdout(chunks, () => registry.runBlock(job.file, job.code));
  try {
    const { strays } = await ctx.strays.during(() => (job.offline ? offline(run) : run()));
    if (strays.length) {
      return {
        status: 'fail',
        message: `unhandled error from attested code: ${strays.join('; ')}`,
      };
    }
    if (want !== undefined) {
      return compareOutput(chunks.join(''), want);
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

async function captureStdout<T>(chunks: string[], run: () => Promise<T>): Promise<T> {
  const original = process.stdout.write.bind(process.stdout);
  const intercepted = (
    chunk: string | Uint8Array,
    encodingOrCb?: BufferEncoding | ((err?: Error | null) => void),
    cb?: (err?: Error | null) => void,
  ): boolean => {
    const text =
      typeof chunk === 'string'
        ? chunk
        : Buffer.from(chunk).toString(typeof encodingOrCb === 'string' ? encodingOrCb : 'utf8');
    chunks.push(text);
    const done = typeof encodingOrCb === 'function' ? encodingOrCb : cb;
    if (done) done(null);
    return true;
  };
  process.stdout.write = intercepted;
  try {
    return await run();
  } finally {
    process.stdout.write = original;
  }
}
