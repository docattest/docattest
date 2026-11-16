import type { CasesJob, Verdict } from '../run/protocol.js';
import { freshRegistry, type WorkerContext } from './context.js';
import { describeError } from './describe.js';
import { offline } from './network.js';

type Runner = (row: Record<string, string>) => unknown;

/**
 * Load the setup module, check the runner and its declared columns, then run each row from
 * `start`. Problems before any row runs fail the whole table.
 */
export async function runCases(
  id: string,
  job: CasesJob,
  start: number,
  ctx: WorkerContext,
): Promise<void> {
  const runner = await prepare(job, ctx);
  if (typeof runner === 'string') {
    ctx.post({ type: 'failed', id, message: runner });
    return;
  }
  ctx.post({ type: 'ready', id });
  for (const row of job.rows.slice(start)) {
    const verdict = await runRow(runner, row.values, `${job.page}:${row.line} ${row.name}`, ctx);
    ctx.post({ type: 'item', id, outcome: { kind: 'verdict', verdict } });
  }
}

async function prepare(job: CasesJob, ctx: WorkerContext): Promise<Runner | string> {
  const at = `cases "${job.runner}" at L${job.line}`;
  let cases: unknown;
  try {
    const registry = freshRegistry(ctx, { snapshot: ctx.snapshot });
    cases = property(await offline(() => registry.load(job.setup.file)), 'cases');
  } catch (err) {
    const reason = describeError(err).message;
    return `${at}: setup module ${job.setup.label} failed to load: ${reason}`;
  }
  if (typeof cases !== 'object' || cases === null) {
    return `${at}: ${job.setup.label} has no \`cases\` export`;
  }
  const runner = property(cases, job.runner);
  if (typeof runner !== 'function') {
    const known = Object.keys(cases).join(', ') || 'none';
    const runnerAt = `cases runner "${job.runner}" at L${job.line}`;
    return `${runnerAt} is not exported by ${job.setup.label} (known: ${known})`;
  }
  const missing = columnsOf(runner).filter((c) => !job.header.includes(c));
  if (missing.length) {
    const columns = missing.map((m) => `"${m}"`).join(', ');
    const noun = missing.length > 1 ? 'columns' : 'column';
    return `${at}: the table is missing ${noun} ${columns} declared by the runner`;
  }
  return (row) => Reflect.apply(runner, undefined, [row]) as unknown;
}

function property(value: unknown, key: string): unknown {
  if ((typeof value !== 'object' && typeof value !== 'function') || value === null) {
    return undefined;
  }
  return Object.hasOwn(value, key) ? (Reflect.get(value, key) as unknown) : undefined;
}

function columnsOf(runner: object): string[] {
  const columns = property(runner, 'columns');
  return Array.isArray(columns) ? columns.map(String) : [];
}

async function runRow(
  runner: Runner,
  values: Record<string, string>,
  name: string,
  ctx: WorkerContext,
): Promise<Verdict> {
  try {
    const { strays } = await ctx.strays.during(() => offline(() => runner({ ...values })));
    if (!strays.length) return { status: 'pass', message: name };
    return {
      status: 'fail',
      message: `${name}: unhandled error from attested code: ${strays.join('; ')}`,
    };
  } catch (err) {
    const failure = describeError(err);
    return { status: 'fail', ...failure, message: `${name}: ${failure.message}` };
  }
}
