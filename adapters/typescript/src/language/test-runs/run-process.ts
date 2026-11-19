import { spawn } from 'node:child_process';
import type { Invocation } from './invocation.js';

/** How a runner's process ended: on its own, with the tail of what it printed, or killed. */
export type ProcessEnd = { kind: 'exited'; output: string } | { kind: 'timedOut' };

/** How much of a runner's output is kept: its last lines say why it stopped. */
const OUTPUT_TAIL = 16 * 1024;

/** On POSIX the runner leads a process group, so a timeout kills its workers with it. */
const GROUPS = process.platform !== 'win32';

/**
 * Run a command to its end, or kill it once it has run `timeoutMs`. What it prints is
 * captured, never passed on: the adapter's stdout carries protocol responses only.
 */
export function runProcess(invocation: Invocation, timeoutMs: number): Promise<ProcessEnd> {
  return new Promise((resolve, reject) => {
    const child = spawn(invocation.command, invocation.args, {
      cwd: invocation.cwd,
      stdio: ['ignore', 'pipe', 'pipe'],
      detached: GROUPS,
    });
    let output = '';
    let exited = false;
    let timedOut = false;
    const keep = (chunk: Buffer) => {
      output = (output + chunk.toString('utf8')).slice(-OUTPUT_TAIL);
    };
    child.stdout.on('data', keep);
    child.stderr.on('data', keep);
    child.once('exit', () => {
      exited = true;
    });
    const timer = setTimeout(() => {
      if (exited) return;
      timedOut = true;
      if (GROUPS && child.pid !== undefined) process.kill(-child.pid, 'SIGKILL');
      else child.kill('SIGKILL');
    }, timeoutMs);
    child.once('error', (err) => {
      clearTimeout(timer);
      reject(err);
    });
    child.once('close', () => {
      clearTimeout(timer);
      resolve(timedOut ? { kind: 'timedOut' } : { kind: 'exited', output });
    });
  });
}
