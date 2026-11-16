// One adapter process driven over the protocol: requests out on stdin, responses back on
// stdout, matched by id. Written in erasable TypeScript so plain `node` can load it too.
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { UserError } from 'docattest';

/** The built server, as `npm run build` leaves it. */
export const SERVER = fileURLToPath(new URL('../../dist/protocol/main.js', import.meta.url));

/** How to start the built server with the running Node. */
export const SERVER_COMMAND: readonly string[] = [process.execPath, SERVER];

interface Waiting {
  resolve: (value: unknown) => void;
  reject: (err: Error) => void;
}

/** Bytes and time spent on JSON at this end of the pipe, for measuring the boundary. */
export interface WireStats {
  bytesOut: number;
  bytesIn: number;
  encodeMs: number;
  decodeMs: number;
}

/** A running adapter. */
export class AdapterProcess {
  readonly child: ChildProcessWithoutNullStreams;
  /** Every line the adapter wrote to stdout, in order. */
  readonly lines: string[] = [];
  /** Every request line sent, in order. */
  readonly sent: string[] = [];
  /** Everything it wrote to stderr. */
  stderr = '';
  readonly exited: Promise<number | null>;
  readonly stats: WireStats = { bytesOut: 0, bytesIn: 0, encodeMs: 0, decodeMs: 0 };
  private nextId = 1;
  private readonly waiting = new Map<number, Waiting>();
  private broken: Error | undefined;

  constructor(command: readonly string[] = SERVER_COMMAND) {
    const [file = '', ...args] = command;
    this.child = spawn(file, args, { stdio: 'pipe' });
    this.child.stderr.setEncoding('utf8');
    this.child.stderr.on('data', (text: string) => (this.stderr += text));
    this.readLines();
    this.exited = new Promise((resolve) => {
      this.child.on('close', (code) => {
        this.fail(new Error(`the adapter exited (${code}) with requests unanswered`));
        resolve(code);
      });
    });
  }

  /** Send one request; resolves with its result or rejects with its error. */
  request(method: string, params: unknown = {}): Promise<unknown> {
    if (this.broken) return Promise.reject(this.broken);
    const id = this.nextId++;
    const started = performance.now();
    const line = `${JSON.stringify({ id, method, params })}\n`;
    this.stats.encodeMs += performance.now() - started;
    this.stats.bytesOut += Buffer.byteLength(line);
    this.sent.push(line);
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.child.stdin.write(line);
    });
  }

  /** Close the adapter's input and wait for it to exit. */
  async end(): Promise<number | null> {
    this.child.stdin.end();
    return this.exited;
  }

  private readLines(): void {
    let pending = '';
    this.child.stdout.setEncoding('utf8');
    this.child.stdout.on('data', (text: string) => {
      pending += text;
      const parts = pending.split('\n');
      pending = parts.pop() ?? '';
      for (const line of parts) this.accept(line);
    });
  }

  private accept(line: string): void {
    this.lines.push(line);
    this.stats.bytesIn += Buffer.byteLength(line) + 1;
    const started = performance.now();
    let response: { id?: unknown; result?: unknown; error?: { kind: string; message: string } };
    try {
      response = JSON.parse(line) as typeof response;
    } catch {
      this.fail(new Error(`the adapter wrote a line that is not a response: ${line}`));
      return;
    }
    this.stats.decodeMs += performance.now() - started;
    const waiting = typeof response.id === 'number' ? this.waiting.get(response.id) : undefined;
    if (!waiting) {
      this.fail(new Error(`the adapter answered a request nobody sent: ${line}`));
      return;
    }
    this.waiting.delete(response.id as number);
    if (response.error) waiting.reject(errorOf(response.error));
    else waiting.resolve(response.result);
  }

  private fail(err: Error): void {
    this.broken ??= err;
    for (const waiting of this.waiting.values()) waiting.reject(err);
    this.waiting.clear();
  }
}

/** A `user` error is a UserError with the adapter's message as is; anything else, an Error. */
export function errorOf(error: { kind: string; message: string }): Error {
  if (error.kind !== 'user') return new Error(`adapter internal error: ${error.message}`);
  const err = new UserError('');
  err.message = error.message;
  return err;
}
