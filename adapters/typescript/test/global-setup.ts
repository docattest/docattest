import { execFileSync } from 'node:child_process';
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Build dist once per test run, so acceptance tests always exercise current sources.
 *
 * The build goes to a fresh directory and replaces dist only when it differs. Test runs can
 * overlap (`docattest check --run-cited` starts one per cited file, side by side), and a run
 * that deleted dist while another read it would break both; runs over the same sources build
 * the same files, so they leave dist alone.
 */
export default function setup(): void {
  const root = fileURLToPath(new URL('..', import.meta.url));
  const dist = join(root, 'dist');
  const built = build(root);
  if (existsSync(dist) && sameTree(built, dist)) {
    rmSync(built, { recursive: true, force: true });
    return;
  }
  rmSync(dist, { recursive: true, force: true });
  renameSync(built, dist);
}

/** Build into a new directory beside dist, as `npm run build` leaves dist, and return it. */
function build(root: string): string {
  const into = mkdtempSync(join(root, '.dist-'));
  const tsc = createRequire(import.meta.url).resolve('typescript/bin/tsc');
  execFileSync(process.execPath, [tsc, '-p', 'tsconfig.build.json', '--outDir', into], {
    cwd: root,
    stdio: 'inherit',
  });
  // The protocol's schemas and the pinned core version are copied in, and the adapter
  // server and the launcher are bins.
  execFileSync(process.execPath, ['../../protocol/sync.ts', into], { cwd: root, stdio: 'inherit' });
  chmodSync(join(into, 'protocol/main.js'), 0o755);
  chmodSync(join(into, 'launcher/main.js'), 0o755);
  return into;
}

/** Whether two directories hold the same files with the same bytes. */
function sameTree(a: string, b: string): boolean {
  const files = filesUnder(a);
  const others = filesUnder(b);
  if (files.length !== others.length || files.some((file, k) => file !== others[k])) return false;
  return files.every((file) => readFileSync(join(a, file)).equals(readFileSync(join(b, file))));
}

/** Every file under a directory, relative to it, sorted. */
function filesUnder(dir: string): string[] {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => relative(dir, join(entry.parentPath, entry.name)))
    .sort();
}
