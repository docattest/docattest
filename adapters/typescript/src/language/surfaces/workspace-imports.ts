import * as fs from 'node:fs';
import * as path from 'node:path';
import ts from '../ts.cjs';
import { ownRelativeTo, type FileSource } from '../../files/source.js';
import { resolveDependency } from '../node/dependency.js';
import { packageName } from '../node/exports-map.js';
import { compilerOptions } from '../program/options.js';
import type { ResolvedSurface } from './resolve.js';

/** A workspace package that a surface's code imports, as the runtime finds it. */
export interface WorkspaceImport {
  /** The package name, as imported. */
  name: string;
  /** The package's directory, symlinks followed. */
  dir: string;
  /** The file the runtime loads for it, symlinks followed; undefined when it does not resolve. */
  entry: string | undefined;
}

/**
 * The workspace packages a surface's code reaches. Imports are followed from the surface's
 * entry through the project's own files; a bare specifier whose `node_modules` entry is a
 * link to a directory of the project (how npm, Yarn, pnpm and Bun install a workspace
 * package) is a workspace package, and the walk goes on through the file the runtime loads
 * for it, so the packages it imports in turn are found too.
 *
 * The returned function serves every surface of the project: a module or link that several
 * surfaces reach is read and resolved once.
 */
export function workspaceImports(
  files: FileSource,
): (surface: ResolvedSurface) => WorkspaceImport[] {
  if (files.memory) return () => [];
  const graph = new ImportGraph(files);
  return (surface) => graph.importsOf(surface);
}

// What can import: JavaScript and TypeScript modules, not the JSON or assets they import.
const MODULE = /\.[mc]?[jt]sx?$/;

/** One import of a module, resolved with symlinks followed: a file, or a workspace package. */
type ImportStep = { kind: 'module'; file: string } | { kind: 'package'; imported: WorkspaceImport };

class ImportGraph {
  private readonly files: FileSource;
  private readonly root: string;
  private readonly options: ts.CompilerOptions;
  private readonly resolutions: ts.ModuleResolutionCache;
  private readonly steps = new Map<string, ImportStep[]>();
  private readonly links = new Map<string, string | undefined>();

  constructor(files: FileSource) {
    this.files = files;
    this.root = fs.realpathSync(files.root);
    this.options = compilerOptions(files);
    const canonical = ts.sys.useCaseSensitiveFileNames
      ? (name: string) => name
      : (name: string) => name.toLowerCase();
    this.resolutions = ts.createModuleResolutionCache(files.root, canonical, this.options);
  }

  importsOf(surface: ResolvedSurface): WorkspaceImport[] {
    if (surface.spec.path !== undefined) {
      if (surface.entry === undefined) return [];
      return this.walk({ kind: 'module', file: fs.realpathSync(surface.entry) });
    }
    const step = this.packageStep(surface.spec.import, path.join(this.files.root, 'package.json'));
    return step === undefined ? [] : this.walk(step);
  }

  private walk(start: ImportStep): WorkspaceImport[] {
    const found = new Map<string, WorkspaceImport>();
    const seen = new Set<string>();
    const queue: string[] = [];
    const take = (step: ImportStep) => {
      if (step.kind === 'package' && !found.has(step.imported.dir)) {
        found.set(step.imported.dir, step.imported);
      }
      const file = step.kind === 'module' ? step.file : step.imported.entry;
      if (file === undefined || seen.has(file) || !this.isOwnModule(file)) return;
      seen.add(file);
      queue.push(file);
    };
    take(start);
    for (const file of queue) this.stepsOf(file).forEach(take);
    return [...found.values()];
  }

  private isOwnModule(file: string): boolean {
    return ownRelativeTo(this.root, file) !== undefined && MODULE.test(file);
  }

  private stepsOf(file: string): ImportStep[] {
    let steps = this.steps.get(file);
    if (steps === undefined) {
      steps = this.readSteps(file);
      this.steps.set(file, steps);
    }
    return steps;
  }

  private readSteps(file: string): ImportStep[] {
    const code = fs.readFileSync(file, 'utf8');
    return ts.preProcessFile(code, true, true).importedFiles.flatMap(({ fileName: specifier }) => {
      const step = isRelative(specifier)
        ? this.relativeStep(specifier, file)
        : this.packageStep(specifier, file);
      return step === undefined ? [] : [step];
    });
  }

  // The file the runtime loads, which for a TypeScript source (`./x.js` naming `x.ts`) is
  // the one the compiler finds.
  private relativeStep(specifier: string, importer: string): ImportStep | undefined {
    const runtime = path.resolve(path.dirname(importer), specifier);
    const file = fs.statSync(runtime, { throwIfNoEntry: false })?.isFile()
      ? runtime
      : ts.resolveModuleName(specifier, importer, this.options, ts.sys, this.resolutions)
          .resolvedModule?.resolvedFileName;
    return file === undefined ? undefined : { kind: 'module', file: fs.realpathSync(file) };
  }

  /** A bare import, when it names a workspace package. */
  private packageStep(specifier: string, importer: string): ImportStep | undefined {
    if (specifier.startsWith('node:')) return undefined;
    const name = packageName(specifier);
    const dir = this.linkedPackage(name, path.dirname(importer));
    if (dir === undefined) return undefined;
    const resolved = resolveDependency(specifier, importer);
    const entry = resolved !== undefined && fs.existsSync(resolved) ? resolved : undefined;
    return { kind: 'package', imported: { name, dir, entry: entry && fs.realpathSync(entry) } };
  }

  private linkedPackage(name: string, from: string): string | undefined {
    const key = `${from}\0${name}`;
    if (!this.links.has(key)) this.links.set(key, this.findLinkedPackage(name, from));
    return this.links.get(key);
  }

  /** The directory `node_modules/<name>` links to, when that lies in the project. */
  private findLinkedPackage(name: string, from: string): string | undefined {
    for (let dir = from; ; dir = path.dirname(dir)) {
      const candidate = path.join(dir, 'node_modules', name);
      const stat = fs.lstatSync(candidate, { throwIfNoEntry: false });
      if (stat) {
        if (!stat.isSymbolicLink()) return undefined;
        const target = fs.realpathSync(candidate);
        return ownRelativeTo(this.root, target) !== undefined ? target : undefined;
      }
      if (path.dirname(dir) === dir) return undefined;
    }
  }
}

function isRelative(specifier: string): boolean {
  return specifier.startsWith('./') || specifier.startsWith('../');
}
