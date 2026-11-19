import * as path from 'node:path';
import type { FileSource } from '../../files/source.js';
import { isObject, objectOrNothing, projectObject } from '../../json.js';
import type { TestRunner } from '../citations/each-titles.js';
import { projectPath } from '../surfaces/packages.js';
import type { Runner } from './invocation.js';

/** The runner of a test file, or why there is none to use. */
export type RunnerChoice = { ok: true; runner: Runner } | { ok: false; message: string };

/** How messages name each runner. */
export const RUNNER_NAMES: Readonly<Record<TestRunner, string>> = {
  vitest: 'Vitest',
  jest: 'Jest',
  'node:test': 'node:test',
};

/** The runners a package can declare, in the order one that declares both is read. */
const DECLARABLE = ['vitest', 'jest'] as const;
type Declarable = (typeof DECLARABLE)[number];

/**
 * The runner of a project-relative test file: the one it imports its test functions from
 * (`imported`), else the one the nearest `package.json` declares among its dependencies.
 * It runs from the directory of the nearest `package.json` that declares it (where its
 * config is), else of the nearest one, else from the root. Vitest and Jest are the package
 * installed in `node_modules` from there up to the root; node:test is the adapter's Node.
 */
export function chooseRunner(
  files: FileSource,
  file: string,
  imported: TestRunner | undefined,
): RunnerChoice {
  const dirs = directoriesUp(files.root, path.dirname(path.join(files.root, file)));
  const manifests = dirs.flatMap((dir) => {
    const manifest = path.join(dir, 'package.json');
    const read = projectObject(files.readAbsolute(manifest), projectPath(files, manifest));
    return read ? [{ dir, declared: declaredRunner(read, imported) }] : [];
  });
  const nearest = manifests[0]?.dir ?? files.root;
  if (imported === 'node:test') return { ok: true, runner: { kind: imported, dir: nearest } };
  const declaring = manifests.find((m) => m.declared);
  const kind = declaring?.declared ?? imported;
  if (!kind) {
    const message =
      `${file} imports its test functions from none of vitest, @jest/globals and ` +
      'node:test, and no package.json declares vitest or jest';
    return { ok: false, message };
  }
  const dir = declaring?.dir ?? nearest;
  const bin = installedBin(files, directoriesUp(files.root, dir), kind);
  if (bin) return { ok: true, runner: { kind, dir, bin } };
  const from = projectPath(files, dir) || '.';
  const message =
    `${RUNNER_NAMES[kind]} is not installed: no node_modules/${kind} from ${from} ` +
    "up to the project's root";
  return { ok: false, message };
}

// The runner a manifest declares: the imported one, or either when the file imports none.
function declaredRunner(
  manifest: Record<string, unknown>,
  imported: TestRunner | undefined,
): Declarable | undefined {
  const wanted = imported === undefined ? DECLARABLE : DECLARABLE.filter((r) => r === imported);
  const { dependencies, devDependencies } = manifest;
  const declares = (name: string) =>
    [dependencies, devDependencies].some((deps) => isObject(deps) && name in deps);
  return wanted.find(declares);
}

// `dir`, then each directory above it up to and including `root`.
function directoriesUp(root: string, dir: string): string[] {
  const parent = path.dirname(dir);
  if (dir === root || parent === dir) return [dir];
  return [dir, ...directoriesUp(root, parent)];
}

// The JavaScript file the installed package declares as its command. It is run with the
// adapter's Node, which works where `node_modules/.bin` holds shell scripts (Windows).
function installedBin(
  files: FileSource,
  dirs: readonly string[],
  name: string,
): string | undefined {
  for (const dir of dirs) {
    const pkg = path.join(dir, 'node_modules', name);
    const bin = objectOrNothing(files.readAbsolute(path.join(pkg, 'package.json')))?.bin;
    const command = isObject(bin) ? bin[name] : bin;
    if (typeof command === 'string') return path.join(pkg, command);
  }
  return undefined;
}
