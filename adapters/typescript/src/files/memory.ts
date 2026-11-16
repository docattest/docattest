import { normalise, relativeTo, type FileSource } from './source.js';

/** The virtual root of in-memory projects. Nothing is ever read from or written to it. */
export const MEMORY_ROOT = '/__docattest_memory__';

/** A project held in memory: path → contents. */
export function memoryFiles(initial: Record<string, string>): FileSource {
  const files = new Map(Object.entries(initial).map(([k, v]) => [normalise(k), v]));
  const read = (file: string) => files.get(normalise(file));
  const toRelative = (abs: string) => relativeTo(MEMORY_ROOT, abs);
  const source: FileSource = {
    root: MEMORY_ROOT,
    memory: true,
    list: () => [...files.keys()].sort(),
    read,
    exists: (file) => files.has(normalise(file)),
    readAbsolute: (abs) => {
      const rel = toRelative(abs);
      return rel === undefined ? undefined : read(rel);
    },
    existsAbsolute: (abs) => {
      const rel = toRelative(abs);
      return rel !== undefined && files.has(rel);
    },
  };
  return source;
}
