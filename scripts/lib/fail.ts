// How a tool stops on a mistake it can name: the message, and exit code 1, with no stack.
import process from 'node:process';

/** Stop with a message. */
export function fail(message: string): never {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}
