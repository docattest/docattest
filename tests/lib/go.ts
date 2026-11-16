// The Go adapter, for the suites that run it: built from adapters/go when there is a go
// command, which is how it is run from a checkout.
import { spawnSync } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ROOT } from '../../scripts/lib/repository.ts';

/**
 * The command that starts the Go adapter, built into a temporary directory; `undefined`
 * when there is no go command to build it with. The caller removes `dir` when done.
 */
export function buildGoAdapter(): { command: string[]; dir: string } | undefined {
  if (spawnSync('go', ['version']).status !== 0) return undefined;
  const dir = mkdtempSync(join(tmpdir(), 'docattest-go-adapter-'));
  const built = spawnSync('go', ['build', '-o', `${dir}/`, './cmd/docattest-adapter-go'], {
    cwd: join(ROOT, 'adapters/go'),
    encoding: 'utf8',
  });
  if (built.status !== 0) throw new Error(`the Go adapter does not build:\n${built.stderr}`);
  return { command: [join(dir, 'docattest-adapter-go')], dir };
}
