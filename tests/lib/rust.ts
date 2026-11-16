// The Rust adapter, for the suites that run it: built from adapters/rust when there is a
// cargo, which is how it is run from a checkout.
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { ROOT } from '../../scripts/lib/repository.ts';

/**
 * The command that starts the Rust adapter, built where cargo builds it (the package's own
 * target directory); `undefined` when there is no cargo to build it with.
 */
export function buildRustAdapter(): string[] | undefined {
  if (spawnSync('cargo', ['--version']).status !== 0) return undefined;
  const dir = join(ROOT, 'adapters/rust');
  const built = spawnSync('cargo', ['build', '--quiet', '--bin', 'docattest-adapter-rust'], {
    cwd: dir,
    encoding: 'utf8',
  });
  if (built.status !== 0) throw new Error(`the Rust adapter does not build:\n${built.stderr}`);
  return [join(dir, 'target/debug/docattest-adapter-rust')];
}
