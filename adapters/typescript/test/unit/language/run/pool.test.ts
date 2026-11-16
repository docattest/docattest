import { Worker } from 'node:worker_threads';
import { afterEach, describe, expect, it } from 'vitest';
import { runJob } from '../../../../src/language/run/jobs.js';
import { WorkerPool } from '../../../../src/language/run/pool.js';
import type { Job } from '../../../../src/language/run/protocol.js';

// A worker that speaks the job protocol and does what each item says: pass, hang at an
// await, spin in a busy loop or exit. `prepare` can fail or hang before any item, and
// `startMs` in its worker data keeps it busy that long before it listens.
const FAKE = `
const { parentPort, threadId, workerData } = require('node:worker_threads');
const startUntil = Date.now() + (workerData?.startMs ?? 0);
while (Date.now() < startUntil) {}
parentPort.on('message', async ({ id, job, start }) => {
  if (job.prepare === 'fail') return parentPort.postMessage({ type: 'failed', id, message: 'no setup' });
  if (job.prepare === 'hang') return;
  parentPort.postMessage({ type: 'ready', id });
  for (let k = start; k < job.items.length; k++) {
    const item = job.items[k];
    if (item === 'hang') await new Promise(() => {});
    if (item === 'spin') for (;;) {}
    if (item === 'exit') process.exit(3);
    const verdict = { status: 'pass', message: 'item ' + k + ' on ' + threadId };
    parentPort.postMessage({ type: 'item', id, outcome: { kind: 'verdict', verdict } });
  }
  parentPort.postMessage({ type: 'done', id });
});
parentPort.postMessage({ type: 'listening' });`;

const pools: WorkerPool[] = [];
const pool = (size: number) => {
  const created = new WorkerPool(size, () => new Worker(FAKE, { eval: true }));
  pools.push(created);
  return created;
};
const job = (items: string[], prepare?: string) => ({ items, prepare }) as unknown as Job;
const messages = (result: Awaited<ReturnType<typeof runJob>>) =>
  result.items.map((i) =>
    i.kind === 'outcome' && i.outcome.kind === 'verdict' ? i.outcome.verdict.message : i.kind,
  );

afterEach(async () => {
  await Promise.all(pools.splice(0).map((p) => p.close()));
});

describe('runJob', () => {
  it('runs every item on one reused worker', async () => {
    const p = pool(1);
    const first = messages(await runJob(p, job(['ok', 'ok']), 2, 1000));
    const second = messages(await runJob(p, job(['ok']), 1, 1000));
    expect(first).toEqual([
      expect.stringMatching(/^item 0 on (\d+)$/),
      expect.stringMatching(/^item 1 on/),
    ]);
    expect(second[0]?.split(' on ')[1]).toBe(first[0]?.split(' on ')[1]);
  });

  it('stops an item stuck at an await or in a busy loop and resumes on a fresh worker', async () => {
    const result = messages(await runJob(pool(1), job(['ok', 'hang', 'spin', 'ok']), 4, 100));
    expect(result.map((m) => m.replace(/ on \d+$/, ''))).toEqual([
      'item 0',
      'timeout',
      'timeout',
      'item 3',
    ]);
    expect(
      new Set(result.filter((m) => m.startsWith('item')).map((m) => m.split(' on ')[1])).size,
    ).toBe(2);
  });

  it("does not count a new worker's start-up against the job's time", async () => {
    const slow = new WorkerPool(
      1,
      () => new Worker(FAKE, { eval: true, workerData: { startMs: 300 } }),
    );
    pools.push(slow);
    const result = messages(await runJob(slow, job(['ok']), 1, 100));
    expect(result).toEqual([expect.stringMatching(/^item 0 on/)]);
  });

  it('records a worker that exits as a lost item and carries on', async () => {
    const result = await runJob(pool(1), job(['exit', 'ok']), 2, 1000);
    expect(result.items[0]).toEqual({ kind: 'lost', message: 'the worker exited with code 3' });
    expect(result.items[1]?.kind).toBe('outcome');
  });

  it('reports a preparation that fails or hangs as a failure of the whole job', async () => {
    expect((await runJob(pool(1), job(['ok'], 'fail'), 1, 1000)).failure).toEqual({
      message: 'no setup',
      timedOut: false,
    });
    expect((await runJob(pool(1), job([], 'hang'), 0, 100)).failure).toEqual({
      message: 'timed out after 100 ms',
      timedOut: true,
    });
  });
});

describe('WorkerPool', () => {
  it('spawns lazily up to its size, then hands released workers to waiters', async () => {
    let spawned = 0;
    const p = new WorkerPool(2, () => {
      spawned++;
      return new Worker(FAKE, { eval: true });
    });
    pools.push(p);
    const a = await p.acquire();
    await p.acquire();
    expect(spawned).toBe(2);
    expect(a?.fresh).toBe(true);
    const waiting = p.acquire();
    if (!a) throw new Error('an open pool gave no worker');
    p.release(a.worker);
    expect(await waiting).toEqual({ worker: a.worker, fresh: false });
    expect(spawned).toBe(2);
  });

  // A caller that gives up (one surface failed to load) closes the session while other
  // jobs still run. A job resumed after that used to spawn a worker nobody would ever
  // terminate, and the process never exited.
  it('spawns nothing once closed: a job cut short by close is not resumed', async () => {
    let spawned = 0;
    const exits: Promise<unknown>[] = [];
    const p = new WorkerPool(1, () => {
      spawned++;
      const worker = new Worker(FAKE, { eval: true });
      exits.push(new Promise((resolve) => worker.once('exit', resolve)));
      return worker;
    });
    pools.push(p);
    // The first item passes, so the job is prepared and under way when the pool closes.
    const running = runJob(p, job(['ok', 'hang', 'ok']), 3, 10_000);
    const queued = runJob(p, job(['ok']), 1, 10_000);
    await new Promise((resolve) => setTimeout(resolve, 200));
    await p.close();
    const [cut, never] = await Promise.all([running, queued]);
    const closed = 'the session was closed before this could run';
    expect(messages(cut).map((m) => m.replace(/ on \d+$/, ''))).toEqual(['item 0', 'lost', 'lost']);
    expect(cut.items[2]).toEqual({ kind: 'lost', message: closed });
    expect(never.failure).toEqual({ message: closed, timedOut: false });
    expect(await p.acquire()).toBeUndefined();
    expect(spawned).toBe(1);
    await Promise.all(exits);
  });
});
