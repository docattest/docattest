import type { LanguageDescription } from '../api/adapter.js';
import { globMatcher, isGlob } from './glob.js';

/**
 * Whether a file or directory of this name is never the project's own, wherever it is (the
 * language's `ignoredNames`). A rule about one directory entry, so a walk turns away at the
 * entry itself and never looks below it.
 */
export type Ignored = (name: string) => boolean;

/** Nothing is ignored. */
export const NOTHING_IGNORED: Ignored = () => false;

/**
 * The rule a language states: each name is a glob for one path segment, read as the config's
 * globs are.
 */
export function ignoreRule(language: Pick<LanguageDescription, 'ignoredNames'>): Ignored {
  const exact = new Set(language.ignoredNames.filter((n) => !isGlob(n)));
  const globs = language.ignoredNames.filter(isGlob);
  const matches = globMatcher(globs);
  return (name) => exact.has(name) || (globs.length > 0 && matches(name));
}
