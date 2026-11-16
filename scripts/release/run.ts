// Running another program from a release script: its output is ours, and a failure stops us.
import process from 'node:process';
import { spawnSync, type SpawnSyncOptions, type SpawnSyncReturns } from 'node:child_process';
import { fail } from '../lib/fail.ts';

/** Run a program; its output goes to ours. Fails on a non-zero exit. */
export function run(
  program: string,
  args: readonly string[],
  settings: SpawnSyncOptions = {},
): SpawnSyncReturns<string | Buffer> {
  const done = spawnSync(program, args, {
    stdio: 'inherit',
    shell: process.platform === 'win32',
    ...settings,
  });
  if (done.error) fail(`cannot run ${program}: ${done.error.message}`);
  if (done.status !== 0) fail(`${program} ${args.join(' ')} exited with ${String(done.status)}`);
  return done;
}
