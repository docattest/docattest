import * as fs from 'node:fs';
import { createRequire } from 'node:module';
import * as path from 'node:path';
import { isObject, objectOrNothing } from '../../json.js';

// Resolving a bare specifier through a package's exports map with the conditions Node uses
// for `import`. CommonJS resolution rejects ESM-only packages (no `require` condition);
// this finds the file they export so it can be loaded with require(esm).

/** Conditions active when resolving for import under Node, matched in exports-map order. */
const CONDITIONS = new Set(['node', 'import', 'module-sync', 'require', 'default']);

/** The file a bare specifier resolves to under import conditions, or undefined. */
export function resolveThroughExports(specifier: string, from: string): string | undefined {
  const { name, subpath } = splitSpecifier(specifier);
  for (const dir of createRequire(from).resolve.paths(specifier) ?? []) {
    const packageDir = path.join(dir, name);
    const manifest = readManifest(path.join(packageDir, 'package.json'));
    if (!manifest) continue;
    // The nearest package with this name decides, exactly as Node would.
    const target = matchSubpath(manifest.exports, subpath);
    const file = target === undefined ? undefined : pickCondition(target);
    return file ? path.join(packageDir, file) : undefined;
  }
  return undefined;
}

function readManifest(file: string): Record<string, unknown> | undefined {
  // A dependency's manifest: one that cannot be read resolves nothing through it.
  return fs.existsSync(file) ? objectOrNothing(fs.readFileSync(file, 'utf8')) : undefined;
}

/** The package a bare specifier names: `@scope/name/sub` → `@scope/name`, `name/sub` → `name`. */
export function packageName(specifier: string): string {
  return splitSpecifier(specifier).name;
}

function splitSpecifier(specifier: string): { name: string; subpath: string } {
  const parts = specifier.split('/');
  const n = specifier.startsWith('@') ? 2 : 1;
  const subpath = parts.length > n ? `./${parts.slice(n).join('/')}` : '.';
  return { name: parts.slice(0, n).join('/'), subpath };
}

// Exports maps are untyped JSON: every step narrows `unknown` as it goes.

function isConditions(value: unknown): value is Record<string, unknown> {
  return isObject(value) && !Object.keys(value).some((k) => k.startsWith('.'));
}

function matchSubpath(exports: unknown, subpath: string): unknown {
  if (typeof exports === 'string' || Array.isArray(exports) || isConditions(exports)) {
    return subpath === '.' ? exports : undefined;
  }
  if (!isObject(exports)) return undefined;
  return exports[subpath] ?? matchPattern(exports, subpath);
}

// Subpath patterns: "./features/*": "./dist/features/*.js". The longest prefix wins.
function matchPattern(exports: Record<string, unknown>, subpath: string): unknown {
  let best: { prefix: number; value: unknown; match: string } | undefined;
  for (const [key, value] of Object.entries(exports)) {
    const star = key.indexOf('*');
    if (star < 0) continue;
    const [pre, post] = [key.slice(0, star), key.slice(star + 1)];
    const fits =
      subpath.length >= pre.length + post.length &&
      subpath.startsWith(pre) &&
      subpath.endsWith(post);
    if (fits && (!best || pre.length > best.prefix)) {
      best = {
        prefix: pre.length,
        value,
        match: subpath.slice(pre.length, subpath.length - post.length),
      };
    }
  }
  return best ? replaceStar(best.value, best.match) : undefined;
}

function replaceStar(value: unknown, match: string): unknown {
  if (typeof value === 'string') return value.replaceAll('*', match);
  if (Array.isArray(value)) return value.map((v: unknown) => replaceStar(v, match));
  if (!isObject(value)) return value;
  return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, replaceStar(v, match)]));
}

function pickCondition(value: unknown): string | undefined {
  if (typeof value === 'string') return value;
  const candidates: unknown[] = Array.isArray(value)
    ? value
    : isObject(value)
      ? Object.entries(value).flatMap(([key, v]) => (CONDITIONS.has(key) ? [v] : []))
      : [];
  for (const candidate of candidates) {
    const file = pickCondition(candidate);
    if (file) return file;
  }
  return undefined;
}
