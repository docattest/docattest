import { randomUUID } from 'node:crypto';
import type { Claim, WorkerPool } from './pool.js';
import {
  isWorkerListening,
  isWorkerMessage,
  type ItemOutcome,
  type Job,
  type JobRequest,
  type WorkerMessage,
} from './protocol.js';

/** How one attempt at a job ended. */
export type AttemptEnd =
  | { kind: 'done' }
  | { kind: 'failed'; message: string }
  | { kind: 'timeout'; prepared: boolean }
  | { kind: 'crashed'; prepared: boolean; message: string };

/** The items an attempt finished, in order from its start, and how it ended. */
interface Attempt {
  outcomes: ItemOutcome[];
  /** Unhandled rejections and exceptions the worker reported outside any item. */
  strays: string[];
  end: AttemptEnd;
}

/** Why work that outlived its session did not run: the pool was closed under it. */
const CLOSED = 'the session was closed before this could run';

/**
 * Run a job on one worker from item `start`. Every phase (preparing, then each item) gets
 * the full time budget, and starting a new worker is none of them; when one overruns, the
 * worker is terminated, which stops the code wherever it is, at an await or in a busy loop
 * alike. On a closed pool the attempt fails at once, so a job cut short by `close` ends
 * there and is not resumed.
 */
export async function attempt(
  pool: WorkerPool,
  job: Job,
  start: number,
  timeoutMs: number,
): Promise<Attempt> {
  const claim = await pool.acquire();
  if (!claim) return { outcomes: [], strays: [], end: { kind: 'failed', message: CLOSED } };
  return new JobAttempt(pool, claim, timeoutMs).run(job, start);
}

class JobAttempt {
  // Tells this attempt's messages from those a worker still sends for an earlier one.
  private readonly id = randomUUID();
  private readonly outcomes: ItemOutcome[] = [];
  private readonly strays: string[] = [];
  private prepared = false;
  private timer: NodeJS.Timeout | undefined;
  private settle: (attempt: Attempt) => void = () => undefined;

  constructor(
    private readonly pool: WorkerPool,
    private readonly claim: Claim,
    private readonly timeoutMs: number,
  ) {}

  /** On a fresh worker the clock starts once it listens; a crash while starting ends it. */
  run(job: Job, start: number): Promise<Attempt> {
    return new Promise((resolve) => {
      this.settle = resolve;
      this.listen('on');
      if (!this.claim.fresh) this.arm();
      this.claim.worker.postMessage({ id: this.id, job, start } satisfies JobRequest);
    });
  }

  private readonly onMessage = (raw: unknown): void => {
    if (isWorkerListening(raw)) this.arm();
    if (isWorkerMessage(raw) && raw.id === this.id) this.onOwnMessage(raw);
  };

  /** A message about this attempt: each phase that ends restarts the clock for the next. */
  private onOwnMessage(message: WorkerMessage): void {
    switch (message.type) {
      case 'ready':
        this.prepared = true;
        this.arm();
        break;
      case 'item':
        this.outcomes.push(message.outcome);
        this.arm();
        break;
      case 'stray':
        this.strays.push(message.message);
        break;
      case 'failed':
        this.finish({ kind: 'failed', message: message.message }, true);
        break;
      case 'done':
        this.finish({ kind: 'done' }, true);
        break;
    }
  }

  private readonly onError = (err: Error): void => {
    this.finish(
      { kind: 'crashed', prepared: this.prepared, message: `the worker crashed: ${err.message}` },
      false,
    );
  };

  private readonly onExit = (code: number): void => {
    this.finish(
      { kind: 'crashed', prepared: this.prepared, message: `the worker exited with code ${code}` },
      false,
    );
  };

  private arm(): void {
    clearTimeout(this.timer);
    const timedOut = () => {
      this.finish({ kind: 'timeout', prepared: this.prepared }, false);
    };
    this.timer = setTimeout(timedOut, this.timeoutMs);
  }

  private finish(end: AttemptEnd, healthy: boolean): void {
    clearTimeout(this.timer);
    this.listen('off');
    if (healthy) this.pool.release(this.claim.worker);
    else this.pool.discard(this.claim.worker);
    this.settle({ outcomes: this.outcomes, strays: this.strays, end });
  }

  private listen(how: 'on' | 'off'): void {
    this.claim.worker[how]('message', this.onMessage);
    this.claim.worker[how]('error', this.onError);
    this.claim.worker[how]('exit', this.onExit);
  }
}
