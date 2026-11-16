import * as vm from 'node:vm';
import { InternalError } from '../../api/errors.js';

/** A function compiled from generated source: arguments and result are untyped. */
export type CompiledFunction = (this: unknown, ...args: unknown[]) => unknown;

/**
 * Compile a function expression in this worker's realm. This is the one place untyped code
 * becomes typed: the source is a wrapper docattest generated around transpiled code, so it
 * is a function by construction, and whatever it returns is treated as unknown.
 */
export function compileFunction(source: string, filename: string): CompiledFunction {
  const value: unknown = vm.runInThisContext(source, { filename });
  if (typeof value !== 'function') {
    throw new InternalError(`vm did not return a function for ${filename}`);
  }
  return value as CompiledFunction;
}
