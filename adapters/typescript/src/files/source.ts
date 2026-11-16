import * as path from 'node:path';

/**
 * Where a project's files come from: a directory on disk or an in-memory map. Relative
 * paths are project-relative and POSIX-style; absolute paths are native.
 */
export interface FileSource {
  /** Absolute root. For in-memory projects, a virtual directory that does not exist. */
  readonly root: string;
  readonly memory: boolean;
  /** Every project file, sorted; skips hidden directories and what is being ignored. */
  list(): string[];
  read(file: string): string | undefined;
  exists(file: string): boolean;
  /** Read by absolute path; in-memory projects see only their own files. */
  readAbsolute(file: string): string | undefined;
  existsAbsolute(file: string): boolean;
}

/** A project-relative path in canonical form: forward slashes, no `./`, no doubled `/`. */
export function normalise(file: string): string {
  return file.replace(/\\/g, '/').replace(/^\.\//, '').replace(/\/+/g, '/');
}

/** The project-relative form of an absolute path, or undefined when it lies outside. */
export function relativeTo(root: string, file: string): string | undefined {
  const rel = path.relative(root, file);
  if (rel.startsWith('..') || path.isAbsolute(rel)) return undefined;
  return rel.split(path.sep).join('/');
}

/** A path for messages: project-relative when inside the project, else absolute. */
export function displayPath(root: string, file: string): string {
  return relativeTo(root, file) ?? file;
}
