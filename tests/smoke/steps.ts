// The smoke test's steps: each is shown with its command, where it ran and everything it
// printed, numbered in the order it ran, and its exit code is held to.
import process from 'node:process';
import { spawnSync, type SpawnSyncReturns } from 'node:child_process';
import { relative } from 'node:path';
import { WINDOWS } from './machine.ts';

/**
 * Whether a program runs through the shell: on Windows, a command found on the PATH (npm
 * and npx are `.cmd` scripts, which only the shell runs). A program given by its `.exe`
 * path runs directly, so its arguments arrive as given: the shell does not quote them, and
 * one with a space or a quote would be split.
 */
function needsShell(program: string): boolean {
  return WINDOWS && !program.toLowerCase().endsWith('.exe');
}

/** Where a step runs and how it must end: `from` is where paths are shown relative to. */
export interface StepOptions {
  cwd: string;
  env: NodeJS.ProcessEnv;
  code?: number;
  from?: string;
}

/** What every part of the smoke test runs its steps with: the steps, an environment, and where paths are shown from. */
export interface StepContext {
  steps: Steps;
  env: NodeJS.ProcessEnv;
  from: string;
}

/** The steps of one smoke test, numbered as they run. */
export class Steps {
  #count = 0;

  /** How many steps ran. */
  get count(): number {
    return this.#count;
  }

  /**
   * Run one step: show the command, where it ran and everything it printed, and fail the
   * smoke test unless it exited with `code`.
   */
  run(
    title: string,
    program: string,
    args: readonly string[],
    { cwd, env, code = 0, from = cwd }: StepOptions,
  ): SpawnSyncReturns<string> {
    const number = ++this.#count;
    process.stdout.write(`\n## ${number}. ${title}\n$ ${[program, ...args].join(' ')}`);
    process.stdout.write(`    (in ${relative(from, cwd) || '.'})\n`);
    const done = spawnSync(program, args, {
      cwd,
      env,
      encoding: 'utf8',
      shell: needsShell(program),
    });
    if (done.error) throw new Error(`cannot run ${program}: ${done.error.message}`);
    const shown = `${done.stdout}${done.stderr}`.trimEnd();
    if (shown) process.stdout.write(`${shown}\n`);
    process.stdout.write(`→ exit ${String(done.status)}\n`);
    if (done.status !== code) {
      throw new Error(`step ${number} exited ${String(done.status)}, not ${code}`);
    }
    return done;
  }
}
