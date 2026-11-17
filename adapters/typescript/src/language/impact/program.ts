import * as path from 'node:path';
import ts from '../ts.cjs';
import type { SurfaceSpec } from '../surfaces/specs.js';
import { ownRelativeTo, type FileSource } from '../../files/source.js';
import { isStringArray, objectOrNothing } from '../../json.js';
import { createHost } from '../program/host.js';
import { compilerOptions, isTypeScriptSource } from '../program/options.js';
import { resolveSurfaces } from '../surfaces/resolve.js';

/** A program over the code behind each surface, as one revision of the project has it. */
interface SurfacePrograms {
  program: ts.Program;
  /** Surface name → the module whose exports are the surface. */
  entries: Map<string, string>;
}

/**
 * Build the program for one revision. Every project file is read through `files`, so the
 * same code builds the base (git's copy of changed files) and the working tree. A surface
 * that points at built output is traced back to its source through the build's source
 * map, because a diff changes the source, not the build.
 */
export function surfacePrograms(files: FileSource, specs: readonly SurfaceSpec[]): SurfacePrograms {
  const options = compilerOptions(files);
  const host = overlayHost(files, options);
  const entries = new Map<string, string>();
  for (const surface of resolveSurfaces(files, specs)) {
    const built =
      surface.types ??
      (surface.spec.path === undefined && !files.memory
        ? resolveBare(surface.spec.import, files.root, options, host)
        : undefined);
    if (built !== undefined) entries.set(surface.spec.name, sourceOf(files, built));
  }
  const program = ts.createProgram({ rootNames: [...new Set(entries.values())], options, host });
  return { program, entries };
}

// The shared host reads disk projects from disk; here every file inside the project comes
// from the FileSource, so a revision that is not the working tree can stand in for it.
function overlayHost(files: FileSource, options: ts.CompilerOptions): ts.CompilerHost {
  const host = createHost({
    files,
    options,
    virtual: new Map(),
    surfaces: new Map(),
    parsed: new Map(),
  });
  const getSourceFile = host.getSourceFile.bind(host);
  const readFile = host.readFile.bind(host);
  const directoryExists = host.directoryExists?.bind(host);
  const own = (abs: string) => ownRelativeTo(files.root, abs) !== undefined;
  const read = (abs: string) => (own(abs) ? files.readAbsolute(abs) : readFile(abs));
  host.readFile = read;
  host.fileExists = (abs) => read(abs) !== undefined;
  // A directory that only the base has must still be searched; a missing one costs a lookup.
  host.directoryExists = (dir) => own(dir) || directoryExists?.(dir) === true;
  host.getSourceFile = (name, version, ...rest) => {
    if (!own(name)) return getSourceFile(name, version, ...rest);
    const text = files.readAbsolute(name);
    return text === undefined ? undefined : ts.createSourceFile(name, text, version, true);
  };
  return host;
}

function resolveBare(
  specifier: string,
  root: string,
  options: ts.CompilerOptions,
  host: ts.CompilerHost,
): string | undefined {
  const from = path.join(root, 'package.json');
  return ts.resolveModuleName(specifier, from, options, host).resolvedModule?.resolvedFileName;
}

/** The source a built module was compiled from, when its source map names one. */
export function sourceOf(files: FileSource, built: string): string {
  if (isTypeScriptSource(built)) return built;
  const runtime = built.replace(/\.d\.([mc]?)ts$/, '.$1js');
  for (const map of [`${built}.map`, `${runtime}.map`]) {
    const source = mappedSource(files, map);
    if (source !== undefined) return source;
  }
  return built;
}

function mappedSource(files: FileSource, map: string): string | undefined {
  // A built file's source map: one that cannot be read maps nothing.
  const parsed = objectOrNothing(files.readAbsolute(map));
  if (!parsed || !isStringArray(parsed.sources)) return undefined;
  const [first] = parsed.sources;
  const sourceRoot = typeof parsed.sourceRoot === 'string' ? parsed.sourceRoot : '';
  if (first === undefined) return undefined;
  const source = path.resolve(path.dirname(map), sourceRoot, first);
  return files.existsAbsolute(source) ? source : undefined;
}
