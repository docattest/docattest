// Worker entry: runs jobs the session sends, one at a time, and reports progress. It never
// loads TypeScript; modules arrive as CommonJS from the session.
import { parentPort, workerData } from 'node:worker_threads';
import { InternalError } from '../../api/errors.js';
import {
  isJobRequest,
  isWorkerData,
  type JobRequest,
  type WorkerListening,
  type WorkerMessage,
} from '../run/protocol.js';
import { runCases } from './cases.js';
import type { WorkerContext } from './context.js';
import { runExample } from './example.js';
import { runFacts } from './facts.js';
import { HostClient } from './host-client.js';
import { Strays } from './strays.js';

if (!parentPort || !isWorkerData(workerData)) {
  throw new InternalError('worker started without its data');
}
const port = parentPort;
const data = workerData;
// The job running now: a stray error is reported against it. Empty before the first job.
let current = '';

const post = (message: WorkerMessage) => {
  port.postMessage(message);
};
const strays = new Strays((message) => {
  post({ type: 'stray', id: current, message });
});
const ctx: WorkerContext = {
  host: new HostClient(data.port, data.signal),
  strays,
  snapshot,
  manifest,
  post,
};

// Escaped errors are attested code's, never the worker's: record them and keep going.
process.on('unhandledRejection', (reason) => {
  strays.add(reason);
});
process.on('uncaughtException', (err) => {
  strays.add(err);
});

port.on('message', (raw: unknown) => {
  if (isJobRequest(raw)) void run(raw);
});
port.postMessage({ type: 'listening' } satisfies WorkerListening);

async function run({ id, job, start }: JobRequest): Promise<void> {
  current = id;
  if (job.type === 'example') {
    post({ type: 'ready', id });
    const verdict = await runExample(job, ctx);
    post({ type: 'item', id, outcome: { kind: 'verdict', verdict } });
  } else if (job.type === 'facts') await runFacts(id, job, start, ctx);
  else await runCases(id, job, start, ctx);
  post({ type: 'done', id });
}

function snapshot(name: string): unknown {
  if (!Object.hasOwn(data.snapshots.data, name)) {
    const dir = data.snapshots.dir ?? '(no snapshots dir configured)';
    throw new Error(`snapshot "${name}" not found in ${dir}`);
  }
  return structuredClone(data.snapshots.data[name]);
}

// The core reads a manifest only when a fact writes its path out; one built at run time
// was never read.
function manifest(path: string): unknown {
  if (!Object.hasOwn(data.manifests, path)) {
    throw new Error(
      `manifest "${path}" was not read: name it in the fact itself, as manifest("${path}")`,
    );
  }
  return structuredClone(data.manifests[path]);
}
