// A tool's command line, read with Node's own parser: options it does not declare, and
// values that are missing, stop it with the parser's message and the tool's usage.
import { parseArgs, type ParseArgsConfig } from 'node:util';
import { fail } from './fail.ts';

/** The options and positional arguments `config` declares, as `parseArgs` reads them. */
export function commandLine<const T extends ParseArgsConfig>(
  config: T,
  usage: string,
): ReturnType<typeof parseArgs<T>> {
  try {
    return parseArgs(config);
  } catch (error) {
    return fail(`${error instanceof Error ? error.message : String(error)}\n${usage}`);
  }
}
