/** Declarations docattest puts in scope for every example. Small and permissive. */
export const GLOBALS = `
interface AttestAssert {
  (value: unknown, message?: string | Error): asserts value;
  ok(value: unknown, message?: string | Error): asserts value;
  equal(actual: unknown, expected: unknown, message?: string | Error): void;
  notEqual(actual: unknown, expected: unknown, message?: string | Error): void;
  strictEqual(actual: unknown, expected: unknown, message?: string | Error): void;
  notStrictEqual(actual: unknown, expected: unknown, message?: string | Error): void;
  deepEqual(actual: unknown, expected: unknown, message?: string | Error): void;
  notDeepEqual(actual: unknown, expected: unknown, message?: string | Error): void;
  deepStrictEqual(actual: unknown, expected: unknown, message?: string | Error): void;
  notDeepStrictEqual(actual: unknown, expected: unknown, message?: string | Error): void;
  partialDeepStrictEqual(actual: unknown, expected: unknown, message?: string | Error): void;
  match(value: string, regExp: RegExp, message?: string | Error): void;
  doesNotMatch(value: string, regExp: RegExp, message?: string | Error): void;
  throws(block: () => unknown, error?: unknown, message?: string | Error): void;
  doesNotThrow(block: () => unknown, error?: unknown, message?: string | Error): void;
  rejects(
    block: Promise<unknown> | (() => Promise<unknown>),
    error?: unknown,
    message?: string | Error,
  ): Promise<void>;
  doesNotReject(
    block: Promise<unknown> | (() => Promise<unknown>),
    error?: unknown,
    message?: string | Error,
  ): Promise<void>;
  fail(message?: string | Error): never;
  ifError(value: unknown): void;
}
declare const assert: AttestAssert;
`;

/** Without @types/node, node: imports still type-check, as `any`. */
export const NODE_FALLBACK = `declare module 'node:*';\n`;

/** File name of the globals module, at the project root. */
export const GLOBALS_FILE = '__docattest_globals__.d.ts';
