import type { Observation } from '../../api/adapter.js';

/** Observe a runtime value: everything a fact comparison needs, as plain data. */
export function observe(value: unknown): Observation {
  const items = toItems(value);
  const observation: Observation = { text: safeString(value), description: describe(value) };
  if (typeof value === 'string') observation.kind = 'text';
  return items ? { ...observation, items } : observation;
}

// Strings are iterable but are scalars on a page; anything else iterable is a list.
function toItems(value: unknown): string[] | undefined {
  if (typeof value === 'string' || value === null || value === undefined) return undefined;
  if (!isIterable(value)) return undefined;
  return Array.from(value, safeString);
}

function isIterable(value: unknown): value is Iterable<unknown> {
  if ((typeof value !== 'object' && typeof value !== 'function') || value === null) return false;
  return Symbol.iterator in value && typeof value[Symbol.iterator] === 'function';
}

function safeString(value: unknown): string {
  try {
    return String(value);
  } catch {
    return Object.prototype.toString.call(value);
  }
}

function describe(value: unknown): string {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'an array';
  if (typeof value === 'object') return 'an object';
  return `${typeof value} ${safeString(value)}`;
}
