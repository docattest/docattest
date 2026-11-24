import * as fs from 'node:fs';
import { createRequire } from 'node:module';
import { resolveThroughExports } from './exports-map.js';

/**
 * Load a third-party dependency natively. Node's CommonJS resolution is tried first; when a
 * package has no `require` path (ESM-only) or its `require` entry is a CJS-rejection stub
 * that throws on load, fall back to its `import` entry through Node 22.12+ `require(esm)`.
 */
export function requireDependency(specifier: string, base: string): unknown {
  const load = createRequire(base);
  try {
    return load(specifier) as unknown;
  } catch (err) {
    const file = resolveThroughExports(specifier, base);
    if (!file || sameResolved(load, specifier, file)) throw err;
    return load(file) as unknown;
  }
}

/** Resolve a bare specifier to a file the way {@link requireDependency} would load it. */
export function resolveDependency(specifier: string, base: string): string | undefined {
  try {
    return createRequire(base).resolve(specifier);
  } catch {
    return resolveThroughExports(specifier, base);
  }
}

function sameResolved(load: NodeJS.Require, specifier: string, file: string): boolean {
  try {
    return realPath(load.resolve(specifier)) === realPath(file);
  } catch {
    return false;
  }
}

function realPath(file: string): string {
  try {
    return fs.realpathSync(file);
  } catch {
    return file;
  }
}
