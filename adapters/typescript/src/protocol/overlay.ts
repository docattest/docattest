import { normalise, relativeTo, type FileSource } from '../files/source.js';

/**
 * The project as it was at the base: changed files come from `read` (undefined when they
 * did not exist), every other file from the working tree. Nothing can be written to it.
 */
export function overlay(
  files: FileSource,
  changed: readonly string[],
  read: (file: string) => string | undefined,
): FileSource {
  const isChanged = new Set(changed);
  const readRel = (file: string) => {
    const rel = normalise(file);
    return isChanged.has(rel) ? read(rel) : files.read(rel);
  };
  const readAbsolute = (abs: string) => {
    const rel = relativeTo(files.root, abs);
    return rel === undefined ? files.readAbsolute(abs) : readRel(rel);
  };
  const listed = () => {
    const kept = files.list().filter((f) => !isChanged.has(f));
    return [...kept, ...changed.filter((f) => read(f) !== undefined)].sort();
  };
  const source: FileSource = {
    root: files.root,
    memory: files.memory,
    list: listed,
    read: readRel,
    exists: (file) => readRel(file) !== undefined,
    readAbsolute,
    existsAbsolute: (abs) => readAbsolute(abs) !== undefined,
  };
  return source;
}
