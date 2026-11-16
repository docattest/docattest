import type { SurfaceSpec } from './specs.js';
import type { FileSource } from '../../files/source.js';
import {
  nearestPackage,
  projectPath,
  sameDirectory,
  workspacePackages,
  type PackageInfo,
} from './packages.js';

/**
 * A warning when a bare-specifier surface resolved to an installed copy of a workspace
 * package rather than the workspace package itself, and the versions differ: the pages
 * are then checked against old code. A symlink to the workspace (the normal install) is
 * the same directory and never warns.
 */
export function staleCopyWarning(
  files: FileSource,
  spec: SurfaceSpec,
  entry: string,
): string | undefined {
  const installed = nearestPackage(files, entry);
  if (!installed) return undefined;
  const workspace = workspacePackages(files).find((p) => p.name === installed.name);
  if (!workspace || sameDirectory(files, installed.dir, workspace.dir)) return undefined;
  if (installed.version === workspace.version) return undefined;
  const show = (p: PackageInfo) => `${projectPath(files, p.dir)} (${p.version ?? 'no version'})`;
  return (
    `"${spec.import}" resolves to ${show(installed)}, ` +
    `not the workspace package ${show(workspace)}; ` +
    'pages are checked against the installed copy. Reinstall so it links to the workspace, ' +
    'or point the surface at the build with { "import", "path" }'
  );
}
