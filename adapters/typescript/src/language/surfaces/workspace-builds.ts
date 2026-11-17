import * as fs from 'node:fs';
import * as path from 'node:path';
import type { FileSource } from '../../files/source.js';
import { isTypeScriptSource } from '../program/options.js';
import { projectPath } from './packages.js';
import type { ResolvedSurface } from './resolve.js';
import { workspaceImports, type WorkspaceImport } from './workspace-imports.js';

/**
 * Warnings for the workspace packages a surface's code imports whose build is missing or
 * older than their sources. The runtime loads a workspace package's build, so pages would
 * otherwise be checked against code that is no longer in its sources, or, before the first
 * build, against a surface missing whatever it takes from the package.
 *
 * The returned function serves every surface of the project: a package that several
 * surfaces import has its files read once.
 */
export function workspaceBuildWarnings(files: FileSource): (surface: ResolvedSurface) => string[] {
  const importsOf = workspaceImports(files);
  const builds = new BuildCheck(files);
  return (surface) => importsOf(surface).flatMap((imported) => builds.warning(imported) ?? []);
}

/** A file and when it last changed. */
interface Stamped {
  file: string;
  modified: number;
}

/** Checks the builds of workspace packages, comparing each build with its sources once. */
class BuildCheck {
  private readonly newerSources = new Map<string, string | undefined>();

  constructor(private readonly files: FileSource) {}

  warning(imported: WorkspaceImport): string | undefined {
    const { files } = this;
    const where = `the workspace package ${imported.name} (${projectPath(files, imported.dir)})`;
    if (imported.entry === undefined) {
      return (
        `${where} has no build: "${imported.name}" does not resolve, so what the surface takes ` +
        `from it is missing. Build ${imported.name}`
      );
    }
    const build = buildDirectory(imported.dir, imported.entry);
    if (build === undefined) return undefined;
    const source = this.newerSource(imported.dir, build);
    if (source === undefined) return undefined;
    return (
      `${where} is loaded from its build ${projectPath(files, build)}, which is older than ` +
      `${projectPath(files, source)}: rebuild ${imported.name}`
    );
  }

  private newerSource(dir: string, build: string): string | undefined {
    if (!this.newerSources.has(build)) this.newerSources.set(build, newerSource(dir, build));
    return this.newerSources.get(build);
  }
}

/**
 * The directory of the package holding the file the runtime loads, when that file is built
 * output: a JavaScript or declaration file below a directory of its own (`dist/index.js`).
 */
function buildDirectory(dir: string, entry: string): string | undefined {
  if (isTypeScriptSource(entry)) return undefined;
  const [top, ...below] = path.relative(dir, entry).split(path.sep);
  return top === undefined || below.length === 0 ? undefined : path.join(dir, top);
}

/**
 * The package's newest TypeScript file, when it changed after its last build. A build is its
 * output and its build record (`*.tsbuildinfo`): an incremental `tsc --build` over sources
 * touched but unchanged rewrites only the record, and the outputs are still current.
 */
function newerSource(dir: string, build: string): string | undefined {
  const outside = stampedFiles(dir, {
    skip: build,
    keep: (file) => isTypeScriptSource(file) || isBuildRecord(file),
  });
  const records = outside.filter((f) => isBuildRecord(f.file));
  const built = newest([...stampedFiles(build), ...records]);
  const source = newest(outside.filter((f) => isTypeScriptSource(f.file)));
  return built && source && source.modified > built.modified ? source.file : undefined;
}

function newest(stamped: readonly Stamped[]): Stamped | undefined {
  return stamped.reduce<Stamped | undefined>(
    (a, b) => (a && a.modified >= b.modified ? a : b),
    undefined,
  );
}

/** Which files below a directory to stamp: those `keep` accepts, outside the directory `skip`. */
interface StampFilter {
  skip?: string;
  keep?: (file: string) => boolean;
}

// Every file below `dir` the filter keeps, skipping hidden directories and node_modules.
function stampedFiles(dir: string, filter: StampFilter = {}): Stamped[] {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry): Stamped[] => {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const skipped =
        entry.name.startsWith('.') || entry.name === 'node_modules' || file === filter.skip;
      return skipped ? [] : stampedFiles(file, filter);
    }
    const kept = entry.isFile() && (filter.keep === undefined || filter.keep(file));
    return kept ? [{ file, modified: fs.statSync(file).mtimeMs }] : [];
  });
}

function isBuildRecord(file: string): boolean {
  return file.endsWith('.tsbuildinfo');
}
