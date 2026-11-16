// Running the docattest core for the programmatic API: the core this package's launcher
// would run (PROTOCOL.md §1, "Launchers"), told the same things, with its output collected.
import { spawn } from 'node:child_process';
import { InternalError, UserError } from '../api/errors.js';
import { coreEnvironment, packageCore } from '../core-binary/package.js';

/** What one run of the core printed, and how it ended. */
export interface CoreRun {
  code: number | null;
  signal: NodeJS.Signals | null;
  stdout: string;
  stderr: string;
}

/** The core this package runs. */
export function core(): string {
  const result = packageCore();
  if ('error' in result) throw new UserError(result.error.replace(/^docattest: /, ''));
  return result.core;
}

/** Run the core `program` with `args` in `cwd`; `env` is added to this process's environment. */
export function runCore(
  program: string,
  args: readonly string[],
  cwd: string,
  env: Record<string, string>,
): Promise<CoreRun> {
  return new Promise((resolve, reject) => {
    const child = spawn(program, args, {
      cwd,
      env: coreEnvironment({ ...process.env, ...env }),
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const out: Buffer[] = [];
    const err: Buffer[] = [];
    child.stdout.on('data', (chunk: Buffer) => out.push(chunk));
    child.stderr.on('data', (chunk: Buffer) => err.push(chunk));
    child.on('error', (error) => {
      reject(new UserError(`cannot run the core (${program}): ${error.message}`));
    });
    child.on('close', (code, signal) => {
      const text = (chunks: Buffer[]) => Buffer.concat(chunks).toString('utf8');
      resolve({ code, signal, stdout: text(out), stderr: text(err) });
    });
  });
}

/**
 * The error a run that did not answer stands for: the core's own user error (exit code 1,
 * a message that begins `docattest: `), else a bug.
 */
export function failure(run: CoreRun): Error {
  const message = run.stderr.trimEnd();
  if (run.code === 1 && message.startsWith('docattest: ')) {
    return new UserError(message.slice('docattest: '.length));
  }
  const ended = run.signal ? `was killed by ${run.signal}` : `exited with code ${run.code}`;
  return new InternalError(`the docattest core ${ended}: ${message}`);
}

/** The JSON a run printed, when it ended with one of `codes`; else its failure. */
export function answer(run: CoreRun, codes: readonly number[] = [0]): unknown {
  if (run.code !== null && codes.includes(run.code) && run.stdout.trim() !== '') {
    try {
      return JSON.parse(run.stdout);
    } catch {
      // Not an answer: reported below as what the core said.
    }
  }
  throw failure(run);
}
