import { createRequire } from 'node:module';
import { resolveThroughExports } from './exports-map.js';

/**
 * Load a third-party dependency natively. Node's CommonJS resolution is used unchanged;
 * only when a package's exports map has no `require` path (ESM-only packages) is it
 * resolved through import conditions and loaded with require(esm), which Node 22.12+ has.
 */
export function requireDependency(specifier: string, base: string): unknown {
  const load = createRequire(base);
  try {
    return load(specifier) as unknown;
  } catch (err) {
    if (!isNotExported(err)) throw err;
    const file = resolveThroughExports(specifier, base);
    if (!file) throw err;
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

function isNotExported(err: unknown): boolean {
  return err instanceof Error && 'code' in err && err.code === 'ERR_PACKAGE_PATH_NOT_EXPORTED';
}
