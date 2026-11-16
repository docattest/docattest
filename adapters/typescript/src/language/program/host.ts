import * as path from 'node:path';
import ts from '../ts.cjs';
import { relativeTo, type FileSource } from '../../files/source.js';
import { threadCache } from '../cache.js';
import { typeScriptName } from './file-names.js';
import { extensionOf } from './options.js';

/** What a compiler host serves beyond the project's own files. */
interface HostInput {
  files: FileSource;
  options: ts.CompilerOptions;
  /** Absolute path → text of virtual modules (example blocks, globals). */
  virtual: ReadonlyMap<string, string>;
  /** Import specifier → absolute path of a surface's types entry. */
  surfaces: ReadonlyMap<string, string>;
  /** Parsed project files, for the programs that share them (one parse serves them all). */
  parsed: Map<string, ts.SourceFile>;
}

// Lib and node_modules declarations never change during a process, so every program shares
// their parsed form, including programs of nested docattest copies.
const SHARED = threadCache<ts.SourceFile>('sourceFiles');

/**
 * A compiler host over a FileSource. In-memory projects are hermetic: only their own files
 * and TypeScript's lib files exist.
 */
export function createHost(input: HostInput): ts.CompilerHost {
  const { files } = input;
  // Keyed by the names TypeScript asks for, which it writes with `/` on every platform.
  const virtual = new Map([...input.virtual].map(([file, text]) => [typeScriptName(file), text]));
  const libFile = ts.getDefaultLibFilePath(input.options);
  const libDir = path.dirname(libFile);
  const readFile = (abs: string): string | undefined => {
    const text = virtual.get(typeScriptName(abs));
    if (text !== undefined) return text;
    if (!files.memory) return ts.sys.readFile(abs);
    return relativeTo(files.root, abs) !== undefined
      ? files.readAbsolute(abs)
      : abs.startsWith(libDir)
        ? ts.sys.readFile(abs)
        : undefined;
  };
  const dirs = files.memory ? memoryDirectories(files, virtual) : undefined;
  const host: ts.CompilerHost = {
    getSourceFile: (fileName, version) => {
      const shareable = fileName.startsWith(libDir) || fileName.includes('/node_modules/');
      return sourceFile(fileName, version, readFile(fileName), shareable ? SHARED : input.parsed);
    },
    getDefaultLibFileName: () => libFile,
    getDefaultLibLocation: () => libDir,
    writeFile: () => undefined,
    getCurrentDirectory: () => files.root,
    getCanonicalFileName: (f) => f,
    useCaseSensitiveFileNames: () => true,
    getNewLine: () => '\n',
    fileExists: (f) => readFile(f) !== undefined,
    readFile,
    directoryExists: (d) =>
      dirs ? dirs.has(d) || d.startsWith(libDir) : ts.sys.directoryExists(d),
    getDirectories: (d) => (dirs ? [] : ts.sys.getDirectories(d)),
    realpath: dirs ? (f) => f : (f) => ts.sys.realpath?.(f) ?? f,
    resolveModuleNameLiterals: (literals, containing, redirected, options) =>
      literals.map(({ text }) => {
        const surface = input.surfaces.get(text);
        if (surface !== undefined) return resolvedSurface(surface);
        return ts.resolveModuleName(text, containing, options, host, undefined, redirected);
      }),
  };
  return host;
}

function sourceFile(
  fileName: string,
  version: ts.ScriptTarget | ts.CreateSourceFileOptions,
  text: string | undefined,
  cache: Map<string, ts.SourceFile>,
) {
  if (text === undefined) return undefined;
  const hit = cache.get(fileName);
  if (hit?.text === text) return hit;
  const created = ts.createSourceFile(fileName, text, version, true);
  cache.set(fileName, created);
  return created;
}

// A surface's import specifier resolves straight to its types entry, not through the
// project's module resolution.
function resolvedSurface(surface: string): ts.ResolvedModuleWithFailedLookupLocations {
  const resolvedUsingTsExtension = /\.[mc]?tsx?$/.test(surface) && !/\.d\.[mc]?ts$/.test(surface);
  return {
    resolvedModule: {
      resolvedFileName: surface,
      extension: extensionOf(surface),
      isExternalLibraryImport: false,
      resolvedUsingTsExtension,
    },
  };
}

function memoryDirectories(files: FileSource, virtual: ReadonlyMap<string, string>): Set<string> {
  const dirs = new Set<string>([typeScriptName(files.root)]);
  const projectFiles = files.list().map((f) => typeScriptName(path.join(files.root, f)));
  for (const file of [...projectFiles, ...virtual.keys()]) {
    for (let d = path.dirname(file); d.length > 1 && !dirs.has(d); d = path.dirname(d)) dirs.add(d);
  }
  return dirs;
}
