import { messageOf, UserError } from './api/errors.js';

/** A JSON-like object, as far as a check has established. */
export type JsonObject = Record<string, unknown>;

/** Whether a value is a plain object: not an array, not null. */
export function isObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Whether a value is an array of strings. */
export function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((v) => typeof v === 'string');
}

/**
 * The object `text` holds, or undefined when there is no text, it is not JSON or it is not an
 * object. Only for files that are not the user's to fix: a dependency's manifest, a map.
 */
export function objectOrNothing(text: string | undefined): JsonObject | undefined {
  if (text === undefined) return undefined;
  try {
    const parsed: unknown = JSON.parse(text);
    return isObject(parsed) ? parsed : undefined;
  } catch {
    return undefined;
  }
}

/**
 * The object one of the project's own files holds (`text`, read from `file`), or undefined
 * when there is no such file. A file that is not an object in its format is the user's to fix.
 */
export function projectObject(
  text: string | undefined,
  file: string,
  parse: (text: string) => unknown = JSON.parse,
): JsonObject | undefined {
  if (text === undefined) return undefined;
  let parsed: unknown;
  try {
    parsed = parse(text);
  } catch (err) {
    throw new UserError(`${file} cannot be read: ${messageOf(err)}`);
  }
  if (!isObject(parsed)) throw new UserError(`${file} does not hold an object`);
  return parsed;
}
