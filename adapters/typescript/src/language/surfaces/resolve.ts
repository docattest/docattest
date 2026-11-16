import * as fs from 'node:fs';
import * as path from 'node:path';
import type { SurfaceDescription } from '../../api/adapter.js';
import type { SurfaceSpec } from './specs.js';
import type { SurfaceResolution } from '../../api/report.js';
import type { FileSource } from '../../files/source.js';
import { resolveDependency } from '../node/dependency.js';
import { nearestPackage, projectPath } from './packages.js';
import { staleCopyWarning } from './stale.js';

/** A surface as the adapter uses it: where it runs from and where its types are. */
export interface ResolvedSurface {
  spec: SurfaceSpec;
  /** The runtime entry, absolute; undefined when the surface did not resolve. */
  entry: string | undefined;
  /** For path surfaces, the file the checker reads; bare surfaces resolve in the checker. */
  types: string | undefined;
  description: SurfaceDescription;
}

/** Resolve every configured surface once per session. */
export function resolveSurfaces(
  files: FileSource,
  specs: readonly SurfaceSpec[],
): ResolvedSurface[] {
  return specs.map((spec) => {
    const entry = runtimeEntry(files, spec);
    const types = spec.path !== undefined && entry ? typesEntry(files, entry) : undefined;
    return { spec, entry, types, description: describe(files, spec, entry) };
  });
}

function runtimeEntry(files: FileSource, spec: SurfaceSpec): string | undefined {
  if (spec.path !== undefined) {
    return files.exists(spec.path) ? path.join(files.root, spec.path) : undefined;
  }
  if (files.memory) return undefined;
  const found = resolveDependency(spec.import, path.join(files.root, 'package.json'));
  return found && fs.existsSync(found) ? fs.realpathSync(found) : found;
}

// A built entry such as dist/index.js is checked against its declarations when they exist,
// which is what a user of the package sees.
function typesEntry(files: FileSource, entry: string): string {
  const declaration = entry.replace(
    /\.(m|c)?jsx?$/,
    (_, kind: string | undefined) => `.d.${kind ?? ''}ts`,
  );
  return declaration !== entry && files.existsAbsolute(declaration) ? declaration : entry;
}

function describe(
  files: FileSource,
  spec: SurfaceSpec,
  entry: string | undefined,
): SurfaceDescription {
  if (!entry) {
    const where = spec.path === undefined ? '' : ` (${spec.path} does not exist)`;
    return {
      warnings: [`"${spec.import}" did not resolve${where}; examples and facts that use it fail`],
    };
  }
  const resolution: SurfaceResolution = { import: spec.import, entry: projectPath(files, entry) };
  const pkg = nearestPackage(files, entry);
  if (pkg) {
    // How npm writes a package and its version.
    resolution.package =
      pkg.version === undefined
        ? { name: pkg.name, display: pkg.name }
        : { name: pkg.name, version: pkg.version, display: `${pkg.name}@${pkg.version}` };
  }
  const stale = spec.path === undefined ? staleCopyWarning(files, spec, entry) : undefined;
  return { resolution, warnings: stale ? [stale] : [] };
}
