import picomatch from 'picomatch';
import type { SourceFiles } from '../api/adapter.js';

/** A predicate for project-relative paths matching any of the globs. Dotfiles match. */
export function globMatcher(globs: readonly string[]): (file: string) => boolean {
  if (globs.length === 0) return () => false;
  return picomatch(
    globs.map((g) => g.replace(/^\.\//, '')),
    { dot: true },
  );
}

/**
 * A predicate for the project-relative paths a language calls its source files: those that
 * match a glob of `include` and no glob of `exclude`.
 */
export function sourceFileMatcher(files: SourceFiles): (file: string) => boolean {
  const included = globMatcher(files.include);
  const excluded = globMatcher(files.exclude);
  return (file) => included(file) && !excluded(file);
}

/** Whether a pattern contains glob syntax, as opposed to naming one path. */
export function isGlob(pattern: string): boolean {
  return picomatch.scan(pattern).isGlob;
}
