// Messages between the session (main side) and its workers. Both ends are docattest, so
// guards only check the discriminant: they exist to keep `unknown` from leaking into code.
import type { MessagePort } from 'node:worker_threads';
import type { Evaluation } from '../../api/adapter.js';
import type { CasesRow } from '../../api/attestation.js';
import { isObject } from '../../json.js';

/** The free variable `import.meta` becomes in transpiled modules; the registry passes it. */
export const IMPORT_META = '__docattest_import_meta';

/** What every worker starts with. */
export interface WorkerData {
  /** Project root, for paths in messages. */
  root: string;
  /** Where the worker asks the session for modules, synchronously. */
  port: MessagePort;
  /** A one-slot lock the session flips when a reply is waiting on `port`. */
  signal: Int32Array;
  snapshots: { dir: string | undefined; data: Record<string, unknown> };
  /** Manifest data by the path facts write it. */
  manifests: Record<string, unknown>;
  /** The text of each file facts read with `file`, by the path facts write it. */
  texts: Record<string, string>;
  /** What facts' regexes match with `matched`, by the path, then the regex, as facts write them. */
  matches: Record<string, Readonly<Record<string, string>>>;
  /** The fields facts read with `manifestField`, by the path, then the keys, as written. */
  manifestFields: Record<string, Readonly<Record<string, unknown>>>;
}

/** Run one example block. */
export interface ExampleJob {
  type: 'example';
  file: string;
  /** Transpiled code, and for each generated line the source line it starts. */
  code: string;
  lines: (number | undefined)[];
  /** The block as written, to name the assert call that failed. */
  source: string;
  offline: boolean;
  weak: boolean;
}

/** One fact expression, compiled to a CommonJS module whose default export evaluates it. */
export interface FactItem {
  expression: string;
  code: string;
  identifiers: string[];
}

/** Evaluate the facts of one page against one surface, in one fresh registry. */
export interface FactsJob {
  type: 'facts';
  surface: string;
  /** The surface's module, to load; undefined when every fact reads only its constants. */
  file: string | undefined;
  /** The constants of the surface's module the facts read, folded from its source. */
  constants: [string, unknown][];
  setup: { file: string; label: string } | undefined;
  facts: FactItem[];
}

/** Run the rows of one cases table. */
export interface CasesJob {
  type: 'cases';
  setup: { file: string; label: string };
  runner: string;
  page: string;
  line: number;
  header: string[];
  rows: CasesRow[];
}

/** A unit of work for one worker. */
export type Job = ExampleJob | FactsJob | CasesJob;

/** A job, starting at item `start` (later than 0 when resuming after a timeout). */
export interface JobRequest {
  id: string;
  job: Job;
  start: number;
}

/** The verdict on one example or row. */
export interface Verdict {
  status: 'pass' | 'fail';
  message: string;
  expected?: string;
  actual?: string;
}

/** What one item produced. */
export type ItemOutcome =
  { kind: 'verdict'; verdict: Verdict } | { kind: 'evaluation'; evaluation: Evaluation };

/** Progress of a job, from the worker. */
export type WorkerMessage =
  | { type: 'ready'; id: string }
  | { type: 'item'; id: string; outcome: ItemOutcome }
  | { type: 'failed'; id: string; message: string }
  | { type: 'done'; id: string }
  | { type: 'stray'; id: string; message: string };

/**
 * Sent once by a new worker when it can take jobs. Starting a worker is docattest's time,
 * not a job's, so a job on a new worker is timed from here.
 */
export interface WorkerListening {
  type: 'listening';
}

/** A synchronous question from a worker about a module. */
export type HostRequest =
  { op: 'resolve'; specifier: string; from: string } | { op: 'source'; file: string };

/** How a specifier resolves: a project module, a dependency loaded natively, or nothing. */
export type Resolution =
  | { kind: 'project'; file: string }
  | { kind: 'external'; base: string }
  | { kind: 'missing'; message: string };

/** A project module's code, ready for the registry. */
export type ModuleSource = { format: 'script'; code: string } | { format: 'json'; text: string };

/** The session's answer to a {@link HostRequest}; `error` when answering it threw. */
export type HostReply =
  | { op: 'resolve'; resolution: Resolution }
  | { op: 'source'; source: ModuleSource | undefined }
  | { op: 'error'; message: string };

/** Whether a message is a {@link WorkerMessage}. */
export function isWorkerMessage(value: unknown): value is WorkerMessage {
  return isObject(value) && typeof value.type === 'string' && typeof value.id === 'string';
}

/** Whether a message is a {@link WorkerListening}. */
export function isWorkerListening(value: unknown): value is WorkerListening {
  return isObject(value) && value.type === 'listening';
}

/** Whether a message is a {@link JobRequest}. */
export function isJobRequest(value: unknown): value is JobRequest {
  return isObject(value) && typeof value.id === 'string' && isObject(value.job);
}

/** Whether a message is a {@link HostRequest}. */
export function isHostRequest(value: unknown): value is HostRequest {
  return isObject(value) && (value.op === 'resolve' || value.op === 'source');
}

/** Whether a message is a {@link HostReply}. */
export function isHostReply(value: unknown): value is HostReply {
  return (
    isObject(value) && (value.op === 'resolve' || value.op === 'source' || value.op === 'error')
  );
}

/** Whether worker data has the shape the session sends. */
export function isWorkerData(value: unknown): value is WorkerData {
  return (
    isObject(value) &&
    typeof value.root === 'string' &&
    value.signal instanceof Int32Array &&
    isObject(value.snapshots) &&
    isObject(value.manifests) &&
    isObject(value.texts) &&
    isObject(value.matches) &&
    isObject(value.manifestFields)
  );
}

/** Why a fact cannot use a name that is both a surface export and a setup fact. */
export function ambiguous(name: string, surface: string): string {
  const what = `"${name}" is both an export of surface "${surface}" and a setup fact`;
  return `${what}; rename the setup fact`;
}
