import * as fs from 'node:fs';
import { createRequire } from 'node:module';
import * as path from 'node:path';
import type * as Yaml from 'yaml';
import { globMatcher } from '../../files/glob.js';
import { relativeTo, type FileSource } from '../../files/source.js';
import { isObject, isStringArray, projectObject } from '../../json.js';

// Only a pnpm workspace needs YAML, so its parser is loaded then, not by every adapter.
const load = createRequire(import.meta.url);
const parseYaml = (text: string): unknown => (load('yaml') as typeof Yaml).parse(text);

/** A package: its name, version and directory (absolute). */
export interface PackageInfo {
  name: string;
  version: string | undefined;
  dir: string;
}

/**
 * The package a file belongs to: the nearest `package.json` with a name. Nameless ones
 * (such as `{ "type": "module" }` markers inside a build) are skipped.
 */
export function nearestPackage(files: FileSource, file: string): PackageInfo | undefined {
  for (let dir = path.dirname(file); ; dir = path.dirname(dir)) {
    const info = readPackage(files, dir);
    if (info) return info;
    if (path.dirname(dir) === dir) return undefined;
  }
}

/** Packages of the project's workspace: npm, Yarn and Bun `workspaces`, and pnpm's file. */
export function workspacePackages(files: FileSource): PackageInfo[] {
  const patterns = workspacePatterns(files);
  const include = globMatcher(
    patterns.filter((p) => !p.startsWith('!')).map((p) => `${p}/package.json`),
  );
  const exclude = globMatcher(
    patterns.filter((p) => p.startsWith('!')).map((p) => `${p.slice(1)}/package.json`),
  );
  return files
    .list()
    .filter((f) => include(f) && !exclude(f))
    .flatMap((f) => readPackage(files, path.join(files.root, path.posix.dirname(f))) ?? []);
}

/**
 * A path for the report: project-relative when inside the project, also when the runtime
 * resolved it through a symlinked root (macOS's /var is /private/var), else absolute.
 */
export function projectPath(files: FileSource, file: string): string {
  const real = files.memory ? undefined : realpath(files.root);
  return (
    relativeTo(files.root, file) ??
    (real === undefined ? undefined : relativeTo(real, file)) ??
    file
  );
}

function realpath(file: string): string | undefined {
  try {
    return fs.realpathSync(file);
  } catch {
    return undefined;
  }
}

/** Whether two package directories are the same directory once symlinks are followed. */
export function sameDirectory(files: FileSource, a: string, b: string): boolean {
  if (files.memory) return a === b;
  return (realpath(a) ?? a) === (realpath(b) ?? b);
}

function readPackage(files: FileSource, dir: string): PackageInfo | undefined {
  const file = path.join(dir, 'package.json');
  const manifest = projectObject(files.readAbsolute(file), projectPath(files, file));
  if (typeof manifest?.name !== 'string' || !manifest.name) return undefined;
  const version = typeof manifest.version === 'string' ? manifest.version : undefined;
  return { name: manifest.name, version, dir };
}

function workspacePatterns(files: FileSource): string[] {
  const workspaces = projectObject(files.read('package.json'), 'package.json')?.workspaces;
  const declared = isObject(workspaces) ? workspaces.packages : workspaces;
  const pnpm = projectObject(
    files.read('pnpm-workspace.yaml'),
    'pnpm-workspace.yaml',
    parseYaml,
  )?.packages;
  return [...(isStringArray(declared) ? declared : []), ...(isStringArray(pnpm) ? pnpm : [])];
}
