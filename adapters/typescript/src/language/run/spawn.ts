import { MessageChannel, Worker } from 'node:worker_threads';
import { messageOf } from '../../api/errors.js';
import type { ModuleHost } from './host.js';
import type { HostReply, WorkerData } from './protocol.js';

const WORKER = new URL('../worker/main.js', import.meta.url);

/**
 * Start a worker wired to a module host. The worker blocks on `signal` after each request;
 * the host posts its reply and then flips the signal, so the reply is always waiting.
 */
export function spawnWorker(
  host: ModuleHost,
  root: string,
  read: Pick<WorkerData, 'snapshots' | 'manifests' | 'texts' | 'matches' | 'manifestFields'>,
): Worker {
  const { port1, port2 } = new MessageChannel();
  const signal = new Int32Array(new SharedArrayBuffer(4));
  port1.on('message', (request: unknown) => {
    port1.postMessage(answer(host, request));
    Atomics.store(signal, 0, 1);
    Atomics.notify(signal, 0);
  });
  const workerData: WorkerData = { root, port: port2, signal, ...read };
  // Attested code may print; its output must not mix with the report or its JSON.
  const worker = new Worker(WORKER, {
    workerData,
    transferList: [port2],
    stdout: true,
    stderr: true,
  });
  worker.stdout.resume();
  worker.stderr.resume();
  worker.once('exit', () => {
    port1.close();
  });
  return worker;
}

// A worker is blocked until it gets a reply, so a failure must still produce one.
function answer(host: ModuleHost, request: unknown): HostReply {
  try {
    return host.handle(request);
  } catch (err) {
    return { op: 'error', message: messageOf(err) };
  }
}
