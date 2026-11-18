import * as path from 'node:path';
import type { AdapterContext } from '../../api/adapter.js';
import type { CasesAttestation, ExampleAttestation } from '../../api/attestation.js';
import type { Result } from '../../api/report.js';
import { resultFor } from './result.js';
import { blockFile } from '../languages.js';
import type { ResolvedSurface } from '../surfaces/resolve.js';
import { transpile } from './transpile.js';
import { ModuleHost } from './host.js';
import { runJob, type ItemResult, type JobResult } from './jobs.js';
import { WorkerPool } from './pool.js';
import type { Job, Verdict } from './protocol.js';
import { spawnWorker } from './spawn.js';

const NETWORK_OFF =
  'network example skipped: the network is off (set DOCATTEST_NETWORK=1 to run it)';

/** Runs examples and cases tables on the session's worker pool, created on first use. */
export class Runner {
  private pool: WorkerPool | undefined;
  private readonly host: ModuleHost;

  constructor(
    private readonly ctx: AdapterContext,
    surfaces: readonly ResolvedSurface[],
  ) {
    const entries = surfaces.flatMap((s) =>
      s.entry === undefined ? [] : [[s.spec.import, s.entry] as const],
    );
    this.host = new ModuleHost(ctx.files, new Map(entries));
  }

  /** Run a job to the end on the pool. */
  async execute(job: Job, count: number): Promise<JobResult> {
    return runJob(this.getPool(), job, count, this.ctx.timeoutMs);
  }

  /** Run one example block. */
  async example(e: ExampleAttestation): Promise<Result> {
    if (e.network && !this.ctx.network) return resultFor(e, 'skipped', NETWORK_OFF);
    const file = blockFile(this.ctx.files.root, e);
    const { code, lines } = transpile(file, e.code);
    const job: Job = {
      type: 'example',
      file,
      code,
      lines,
      source: e.code,
      offline: !e.network,
      weak: e.weak,
    };
    const { items, strays } = await this.execute(job, 1);
    const [item] = items;
    const verdict = item
      ? this.verdict(item)
      : { status: 'fail' as const, message: 'the block produced no result' };
    return resultFor(e, ...withStrays(verdict, strays));
  }

  /** Run every row of a cases table; an unusable table is one `error` at its line. */
  async cases(c: CasesAttestation): Promise<Result[]> {
    const at = `cases "${c.runner}" at L${c.line}`;
    const setup = this.ctx.setup;
    if (setup === undefined) {
      return [
        resultFor(
          c,
          'error',
          `${at}: no setup module is configured ("setup" in docattest.config.json)`,
        ),
      ];
    }
    if (!this.ctx.files.exists(setup)) {
      return [resultFor(c, 'error', `${at}: setup module ${setup} not found`)];
    }
    const { runner, page, line, header, rows } = c;
    const label = { file: path.join(this.ctx.files.root, setup), label: setup };
    const job: Job = { type: 'cases', setup: label, runner, page, line, header, rows };
    const { failure, items, strays } = await this.execute(job, rows.length);
    if (failure) {
      const message = failure.timedOut
        ? `${at}: setup module ${setup} failed to load: ${failure.message}`
        : failure.message;
      return [resultFor(c, 'error', message)];
    }
    return rows.map((row, k) => {
      const item = items[k];
      const name = `${page}:${row.line} ${row.name}`;
      const verdict = item
        ? this.verdict(item, name)
        : { status: 'fail' as const, message: `${name}: did not run` };
      const [status, message, detail] =
        k === rows.length - 1 ? withStrays(verdict, strays) : withStrays(verdict, []);
      return resultFor(c, status, message, { ...detail, line: row.line });
    });
  }

  /** Terminate the pool's workers. */
  async close(): Promise<void> {
    await this.pool?.close();
  }

  private verdict(item: ItemResult, name?: string): Verdict {
    const prefix = name === undefined ? '' : `${name}: `;
    if (item.kind === 'timeout') {
      return { status: 'fail', message: `${prefix}timed out after ${this.ctx.timeoutMs} ms` };
    }
    if (item.kind === 'lost') return { status: 'fail', message: `${prefix}${item.message}` };
    if (item.outcome.kind === 'verdict') return item.outcome.verdict;
    return { status: 'fail', message: `${prefix}the worker returned an evaluation for a row` };
  }

  private getPool(): WorkerPool {
    const snapshots = {
      dir: this.ctx.snapshots.dir,
      data: Object.fromEntries(this.ctx.snapshots.data),
    };
    const manifests = Object.fromEntries(this.ctx.manifests);
    const texts = Object.fromEntries(this.ctx.texts);
    this.pool ??= new WorkerPool(this.ctx.workers, () =>
      spawnWorker(this.host, this.ctx.files.root, { snapshots, manifests, texts }),
    );
    return this.pool;
  }
}

type VerdictParts = [Verdict['status'], string, { expected?: string; actual?: string }];

// Errors that escaped the attested code after its block or row finished still fail it.
function withStrays(verdict: Verdict, strays: readonly string[]): VerdictParts {
  const { status, message, expected, actual } = verdict;
  const detail = {
    ...(expected === undefined ? {} : { expected }),
    ...(actual === undefined ? {} : { actual }),
  };
  if (!strays.length) return [status, message, detail];
  const escaped = `unhandled error from attested code: ${strays.join('; ')}`;
  return ['fail', status === 'pass' ? escaped : `${message}; ${escaped}`, detail];
}
