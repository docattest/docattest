import type { Worker } from 'node:worker_threads';

/** A worker the pool handed out; a fresh one was just spawned and may still be starting. */
export interface Claim {
  worker: Worker;
  fresh: boolean;
}

/**
 * A pool of reusable workers, spawned lazily up to `size`. A worker that timed out is
 * discarded (terminated) and a fresh one takes its place when work is waiting.
 *
 * Closing is final: work still in flight loses its worker, and nothing it does afterwards
 * (resuming, waiting its turn, starting) gets another, so no worker outlives `close`.
 */
export class WorkerPool {
  private readonly idle: Worker[] = [];
  private readonly busy = new Set<Worker>();
  private readonly waiting: ((claim: Claim | undefined) => void)[] = [];
  private readonly stopping = new Set<Promise<number>>();
  private closed = false;

  constructor(
    private readonly size: number,
    private readonly spawn: () => Worker,
  ) {}

  /** An idle worker, a new one, or the next one released; none once the pool is closed. */
  async acquire(): Promise<Claim | undefined> {
    if (this.closed) return undefined;
    const idle = this.idle.pop();
    if (idle) return { worker: this.claim(idle), fresh: false };
    if (this.busy.size < this.size) return { worker: this.claim(this.spawn()), fresh: true };
    return new Promise((resolve) => this.waiting.push(resolve));
  }

  /** Return a healthy worker for reuse. */
  release(worker: Worker): void {
    // Closing already terminated it; its last message was still on its way.
    if (this.closed) return;
    const next = this.waiting.shift();
    if (next) {
      next({ worker, fresh: false });
      return;
    }
    this.busy.delete(worker);
    this.idle.push(worker);
  }

  /** Terminate a worker that cannot be trusted any more, and replace it if work waits. */
  discard(worker: Worker): void {
    this.busy.delete(worker);
    this.stop(worker);
    if (this.closed) return;
    const next = this.waiting.shift();
    if (next) next({ worker: this.claim(this.spawn()), fresh: true });
  }

  /** Terminate every worker, for good: whoever waits for one, or asks later, gets none. */
  async close(): Promise<void> {
    this.closed = true;
    for (const worker of [...this.idle, ...this.busy]) this.stop(worker);
    this.idle.length = 0;
    this.busy.clear();
    for (const next of this.waiting.splice(0)) next(undefined);
    await Promise.all(this.stopping);
  }

  private claim(worker: Worker): Worker {
    this.busy.add(worker);
    return worker;
  }

  private stop(worker: Worker): void {
    const stopped = worker.terminate();
    this.stopping.add(stopped);
    void stopped.finally(() => this.stopping.delete(stopped));
  }
}
