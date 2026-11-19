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
  file,
  matched,
  manifestField,
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

// The core reads a file only when a fact writes its path out, as it does a manifest.
function file(path: string): string {
  const text = Object.hasOwn(data.texts, path) ? data.texts[path] : undefined;
  if (text === undefined) {
    throw new Error(`file "${path}" was not read: name it in the fact itself, as file("${path}")`);
  }
  return text;
}

// The core matches a regex, and looks up a field, only when a fact writes both arguments out.
function matched(path: string, pattern: string): string {
  const text = Object.hasOwn(data.matches, path) ? data.matches[path] : undefined;
  const found = text !== undefined && Object.hasOwn(text, pattern) ? text[pattern] : undefined;
  if (found === undefined) throw new Error(notRead('matched', path, pattern));
  return found;
}

function manifestField(path: string, keys: string): unknown {
  const fields = Object.hasOwn(data.manifestFields, path) ? data.manifestFields[path] : undefined;
  if (fields === undefined || !Object.hasOwn(fields, keys)) {
    throw new Error(notRead('manifestField', path, keys));
  }
  return structuredClone(fields[keys]);
}

function notRead(name: string, path: string, argument: string): string {
  const call = `${name}(${JSON.stringify(path)}, ${JSON.stringify(argument)})`;
  return `${call} was not read: write both arguments out in the fact itself, as string literals`;
}
