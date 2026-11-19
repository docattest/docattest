import { UserError } from '../api/errors.js';
import type { SymbolRef } from '../api/adapter.js';
import type { Attestation } from '../api/attestation.js';
import { isObject, isStringArray, type JsonObject } from '../json.js';

/** The protocol version this adapter speaks. */
export const PROTOCOL_VERSION = 1;

/** A request id: chosen by the core, echoed in the response. */
export type RequestId = number | string;

/** One request line, parsed. */
export interface Request {
  id: RequestId;
  method: string;
  params: JsonObject;
}

/** An error response's payload. */
interface WireError {
  kind: 'user' | 'internal';
  message: string;
}

/** One response line. `id` is null only when the request line had no readable id. */
export type Response =
  { id: RequestId | null; result: unknown } | { id: RequestId | null; error: WireError };

/**
 * The core broke the protocol: a malformed line, an unknown method, a method out of order or
 * params of the wrong shape. Reported as `internal`, since it is a bug in the core, but
 * without a stack: the message says everything.
 */
export class ProtocolError extends Error {
  override readonly name = 'ProtocolError';
}

/** The error payload for anything a handler threw. */
export function wireError(err: unknown): WireError {
  if (err instanceof UserError) return { kind: 'user', message: err.message };
  if (err instanceof ProtocolError) return { kind: 'internal', message: err.message };
  const message = err instanceof Error ? (err.stack ?? err.message) : String(err);
  return { kind: 'internal', message };
}

/** Parse one request line; a line without a readable id throws with `id` attached. */
export function parseRequest(line: string): Request {
  let value: unknown;
  try {
    value = JSON.parse(line);
  } catch {
    throw new ProtocolError(`a request line is not JSON: ${preview(line)}`);
  }
  if (!isObject(value)) throw new ProtocolError(`a request is not an object: ${preview(line)}`);
  const { id, method, params = {} } = value;
  if (typeof id !== 'number' && typeof id !== 'string') {
    throw new ProtocolError(`a request has no "id" (a number or a string): ${preview(line)}`);
  }
  if (typeof method !== 'string') {
    throw Object.assign(new ProtocolError(`request ${id} has no "method"`), { id });
  }
  if (!isObject(params)) {
    throw Object.assign(new ProtocolError(`request ${id}: "params" is not an object`), { id });
  }
  return { id, method, params };
}

/** The id a failed parse carried, when it got that far. */
export function idOf(err: unknown): RequestId | null {
  const id = isObject(err) ? err.id : undefined;
  return typeof id === 'number' || typeof id === 'string' ? id : null;
}

const preview = (line: string) => (line.length > 80 ? `${line.slice(0, 80)}…` : line);

/** Typed reads of request params; a missing or mistyped field is a protocol error. */
export class Params {
  constructor(
    private readonly method: string,
    private readonly params: JsonObject,
  ) {}

  private fail(name: string, what: string): never {
    throw new ProtocolError(`${this.method}: "${name}" must be ${what}`);
  }

  list(name: string): unknown[] {
    const value = this.params[name];
    return Array.isArray(value) ? (value as unknown[]) : this.fail(name, 'an array');
  }

  /** Attestations of one kind, each with its id, page and line (PROTOCOL.md §3). */
  attestations<K extends Attestation['kind']>(
    name: string,
    kind: K,
  ): Extract<Attestation, { kind: K }>[];
  /** Attestations of any kind, each with its id, page and line. */
  attestations(name: string): Attestation[];
  attestations(name: string, kind?: Attestation['kind']): Attestation[] {
    const items = this.list(name);
    const fits = (a: unknown) =>
      isObject(a) &&
      typeof a.id === 'string' &&
      typeof a.page === 'string' &&
      typeof a.line === 'number' &&
      (kind === undefined ? typeof a.kind === 'string' : a.kind === kind);
    const what = kind === undefined ? 'attestations' : `${kind} attestations`;
    return items.every(fits) ? (items as Attestation[]) : this.fail(name, `a list of ${what}`);
  }

  /** Symbols as `symbols` answers them: each a `name`, with a `surface` or none. */
  symbolRefs(name: string): SymbolRef[] {
    const items = this.list(name);
    const fits = (s: unknown) =>
      isObject(s) &&
      typeof s.name === 'string' &&
      (s.surface === undefined || typeof s.surface === 'string');
    return items.every(fits) ? (items as SymbolRef[]) : this.fail(name, 'a list of symbols');
  }

  strings(name: string): string[] {
    const value = this.params[name];
    return isStringArray(value) ? value : this.fail(name, 'an array of strings');
  }

  string(name: string): string {
    const value = this.params[name];
    return typeof value === 'string' ? value : this.fail(name, 'a string');
  }

  optionalString(name: string): string | undefined {
    return this.params[name] === undefined ? undefined : this.string(name);
  }

  number(name: string): number {
    const value = this.params[name];
    return typeof value === 'number' ? value : this.fail(name, 'a number');
  }

  boolean(name: string): boolean {
    const value = this.params[name];
    return typeof value === 'boolean' ? value : this.fail(name, 'a boolean');
  }

  object(name: string): JsonObject {
    const value = this.params[name];
    return isObject(value) ? value : this.fail(name, 'an object');
  }

  has(name: string): boolean {
    return this.params[name] !== undefined;
  }

  /** An object whose values are all strings. */
  textMap(name: string): Record<string, string> {
    const value = this.object(name);
    const ok = Object.values(value).every((v) => typeof v === 'string');
    return ok ? (value as Record<string, string>) : this.fail(name, 'a map of strings');
  }

  /** An object whose values are all objects. */
  objects(name: string): Record<string, JsonObject> {
    const value = this.object(name);
    const ok = Object.values(value).every(isObject);
    return ok ? (value as Record<string, JsonObject>) : this.fail(name, 'a map of objects');
  }

  /** An object whose values are all objects whose values are all strings. */
  textMaps(name: string): Record<string, Record<string, string>> {
    const value = this.objects(name);
    const texts = (map: JsonObject) => Object.values(map).every((v) => typeof v === 'string');
    const ok = Object.values(value).every(texts);
    return ok
      ? (value as Record<string, Record<string, string>>)
      : this.fail(name, 'a map of maps of strings');
  }

  /** An object whose values are strings or null. */
  textOrNullMap(name: string): Record<string, string | null> {
    const value = this.object(name);
    const ok = Object.values(value).every((v) => typeof v === 'string' || v === null);
    return ok
      ? (value as Record<string, string | null>)
      : this.fail(name, 'a map of strings or null');
  }
}
