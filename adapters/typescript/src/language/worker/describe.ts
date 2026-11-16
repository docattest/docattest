import * as util from 'node:util';
import { isObject } from '../../json.js';

/** A thrown value described for a result: message, and both sides of a comparison. */
interface Failure {
  message: string;
  expected?: string;
  actual?: string;
}

interface AssertionLike {
  message: string;
  operator: string;
  generatedMessage: boolean;
  expected: unknown;
  actual: unknown;
}

const OPERATOR_NAMES: Record<string, string> = {
  '==': 'ok',
  strictEqual: 'equal',
  notStrictEqual: 'notEqual',
  deepStrictEqual: 'deepEqual',
  notDeepStrictEqual: 'notDeepEqual',
};
const COMPARISONS = new Set([
  'strictEqual',
  'deepStrictEqual',
  'equal',
  'deepEqual',
  '===',
  'partialDeepStrictEqual',
  'match',
]);
const NEGATIONS = new Set([
  'notStrictEqual',
  'notDeepStrictEqual',
  'notEqual',
  'notDeepEqual',
  '!=',
  '!==',
  'doesNotMatch',
]);

/** A value for a message: compact, one line, bounded. */
export function show(value: unknown): string {
  if (value instanceof Error) return `[${value.name}: ${firstLine(value.message)}]`;
  const options = {
    depth: 4,
    breakLength: Infinity,
    compact: true,
    maxArrayLength: 50,
    maxStringLength: 300,
  };
  const text = util.inspect(value, options);
  return text.length > 400 ? `${text.slice(0, 397)}...` : text;
}

/**
 * Describe a thrown value. `line` is the block line it passed through, if known; `source`
 * is the block as written, used to name the assert call that failed.
 */
export function describeError(err: unknown, line?: number, source?: string): Failure {
  const at = (call?: string) => {
    if (line) return ` (${call ? `${call}, ` : ''}block line ${line})`;
    return call ? ` (${call})` : '';
  };
  if (isAssertion(err)) return describeAssertion(err, assertCall(err, line, source), at);
  if (err instanceof Error) return { message: `${err.name}: ${firstLine(err.message)}${at()}` };
  return { message: `threw ${show(err)}${at()}` };
}

function describeAssertion(
  e: AssertionLike,
  call: string | undefined,
  at: (call?: string) => string,
): Failure {
  const sides = { expected: show(e.expected), actual: show(e.actual) };
  const versus = `expected ${sides.expected}, got ${sides.actual}`;
  const first = firstLine(e.message);
  let text: string;
  if (!e.generatedMessage) text = e.message;
  else if (COMPARISONS.has(e.operator)) text = versus;
  else if (NEGATIONS.has(e.operator)) {
    text = `expected anything but ${sides.expected}, got ${sides.actual}`;
  } else if (first.endsWith(':')) text = `${first} ${versus}`;
  else return { message: `AssertionError: ${first}${at(call)}` };
  return { message: `AssertionError: ${text}${at(call)}`, ...sides };
}

// Name the call as written on the failing line (`assert.match`), else from the operator.
function assertCall(
  e: AssertionLike,
  line: number | undefined,
  source: string | undefined,
): string | undefined {
  if (line && source) {
    const written = /\bassert(?:\s*\.\s*(\w+))?\s*\(/.exec(source.split('\n')[line - 1] ?? '');
    return written?.[0].replace(/\s+|\($/g, '');
  }
  return `assert.${OPERATOR_NAMES[e.operator] ?? e.operator}`;
}

function isAssertion(err: unknown): err is AssertionLike {
  return (
    isObject(err) &&
    err.code === 'ERR_ASSERTION' &&
    typeof err.message === 'string' &&
    typeof err.operator === 'string'
  );
}

function firstLine(text: string): string {
  return text.split('\n')[0] ?? '';
}

/** The source line (1-based) within `file` that an error's stack passed through, if any. */
export function lineIn(
  err: unknown,
  file: string,
  lines: readonly (number | undefined)[],
): number | undefined {
  const stack = err instanceof Error ? (err.stack ?? '') : '';
  const pattern = new RegExp(`${file.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}:(\\d+):\\d+`);
  const match = pattern.exec(stack);
  if (!match) return undefined;
  for (let generated = Number(match[1]) - 1; generated >= 0; generated--) {
    const original = lines[generated];
    if (original !== undefined) return original + 1;
  }
  return undefined;
}
