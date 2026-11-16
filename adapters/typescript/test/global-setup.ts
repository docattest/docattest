import { execFileSync } from 'node:child_process';
import { chmodSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

/** Build dist once per test run, so acceptance tests always exercise current sources. */
export default function setup(): void {
  const root = fileURLToPath(new URL('..', import.meta.url));
  const tsc = createRequire(import.meta.url).resolve('typescript/bin/tsc');
  rmSync(new URL('../dist', import.meta.url), { recursive: true, force: true });
  execFileSync(process.execPath, [tsc, '-p', 'tsconfig.build.json'], {
    cwd: root,
    stdio: 'inherit',
  });
  // As `npm run build` leaves it: the protocol's schemas and the pinned core version are
  // copied in, and the adapter server and the launcher are bins.
  execFileSync(process.execPath, ['../../protocol/sync.ts', 'dist'], {
    cwd: root,
    stdio: 'inherit',
  });
  chmodSync(new URL('../dist/protocol/main.js', import.meta.url), 0o755);
  chmodSync(new URL('../dist/launcher/main.js', import.meta.url), 0o755);
}
