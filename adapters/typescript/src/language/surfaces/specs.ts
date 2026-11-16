import * as path from 'node:path';
import type { ConfiguredSurface } from '../../api/adapter.js';
import { normalise, type FileSource } from '../../files/source.js';
import { projectObject } from '../../json.js';

/** A configured surface, normalised: what examples import and, optionally, where it is. */
export interface SurfaceSpec {
  name: string;
  /** The specifier examples and facts import. */
  import: string;
  /** A project-relative file it resolves to; absent to resolve `import` like the runtime. */
  path?: string;
}

/** What in-memory projects, and projects without a package name, import path surfaces as. */
const FIXTURE_PACKAGE = 'docattest-fixture';

/**
 * Normalise configured surfaces. A `./path` is imported under the project's package name
 * (`<name>/<surface>` for surfaces other than `default`); a bare specifier is imported as
 * itself and resolved by the runtime; the object form names both.
 */
export function surfaceSpecs(
  surfaces: readonly ConfiguredSurface[],
  files: FileSource,
): SurfaceSpec[] {
  const pkg = packageName(files);
  return surfaces.map(({ name, entry }): SurfaceSpec => {
    if (typeof entry !== 'string') {
      return { name, import: entry.import, path: normalise(entry.path) };
    }
    if (!isPathLike(entry)) return { name, import: entry };
    const specifier = name === 'default' ? pkg : `${pkg}/${name}`;
    return { name, import: specifier, path: path.posix.normalize(normalise(entry)) };
  });
}

function isPathLike(entry: string): boolean {
  return entry.startsWith('.') || entry.startsWith('/');
}

function packageName(files: FileSource): string {
  if (files.memory) return FIXTURE_PACKAGE;
  const name = projectObject(files.read('package.json'), 'package.json')?.name;
  return typeof name === 'string' && name ? name : FIXTURE_PACKAGE;
}
