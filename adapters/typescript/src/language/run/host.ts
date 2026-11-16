import { builtinModules } from 'node:module';
import * as path from 'node:path';
import { InternalError } from '../../api/errors.js';
import { displayPath, type FileSource } from '../../files/source.js';
import { transpile } from './transpile.js';
import { isHostRequest, type HostReply, type ModuleSource, type Resolution } from './protocol.js';

const SCRIPT_EXTENSIONS = ['.ts', '.tsx', '.mts', '.cts', '.js', '.mjs', '.cjs', '.json'];

/**
 * Answers workers' module questions. The session owns the files (in-memory projects exist
 * only here) and the compiler, so workers never load TypeScript: they ask for a module and
 * get CommonJS back.
 */
export class ModuleHost {
  constructor(
    private readonly files: FileSource,
    /** Import specifier → runtime entry of a surface, absolute. */
    private readonly surfaces: ReadonlyMap<string, string>,
  ) {}

  /** Answer one request from a worker. */
  handle(request: unknown): HostReply {
    if (!isHostRequest(request)) {
      throw new InternalError('a worker sent a malformed module request');
    }
    if (request.op === 'resolve') {
      return { op: 'resolve', resolution: this.resolve(request.specifier, request.from) };
    }
    return { op: 'source', source: this.source(request.file) };
  }

  /** Surfaces and relative imports are project modules; anything else is a dependency. */
  resolve(specifier: string, from: string): Resolution {
    const surface = this.surfaces.get(specifier);
    if (surface !== undefined) return { kind: 'project', file: surface };
    if (specifier.startsWith('.') || specifier.startsWith('/')) {
      const file = this.resolveFile(path.resolve(path.dirname(from), specifier));
      if (file) return { kind: 'project', file };
      return {
        kind: 'missing',
        message: `Cannot find module '${specifier}' from ${displayPath(this.files.root, from)}`,
      };
    }
    // In-memory files are not on disk, so their dependencies resolve from where docattest runs.
    const builtin = specifier.startsWith('node:') || builtinModules.includes(specifier);
    const base = builtin || this.files.memory ? path.join(process.cwd(), 'noop.js') : from;
    return { kind: 'external', base };
  }

  /** A project module's code, transpiled to CommonJS. */
  source(file: string): ModuleSource | undefined {
    const text = this.files.readAbsolute(file);
    if (text === undefined) return undefined;
    if (file.endsWith('.json')) return { format: 'json', text };
    return { format: 'script', code: transpile(file, text).code };
  }

  // Sources import each other by their compiled names (./x.js for ./x.ts), so try those too.
  private resolveFile(file: string): string | undefined {
    const candidates = [file];
    const ext = path.extname(file);
    if (['.js', '.mjs', '.cjs', '.jsx'].includes(ext)) {
      const stem = file.slice(0, -ext.length);
      candidates.push(stem + ext.replace('j', 't'), `${stem}.ts`, `${stem}.tsx`);
    }
    candidates.push(...SCRIPT_EXTENSIONS.map((e) => file + e));
    candidates.push(...['.ts', '.tsx', '.js'].map((e) => path.join(file, `index${e}`)));
    return candidates.find((c) => this.files.existsAbsolute(c));
  }
}
