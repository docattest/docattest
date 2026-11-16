/**
 * A map shared by every copy of docattest loaded in this thread. Checking docattest's own
 * docs loads fresh copies of docattest per example, and parsing TypeScript's lib files or
 * transpiling the same module again for each copy would dominate the run. Values must be
 * pure functions of their key.
 */
export function threadCache<V>(name: string): Map<string, V> {
  const key = Symbol.for(`docattest.${name}`);
  // The one untyped boundary: every copy stores the same shape under the same key.
  const holder = globalThis as unknown as Record<symbol, Map<string, V> | undefined>;
  const existing = holder[key];
  if (existing) return existing;
  const created = new Map<string, V>();
  holder[key] = created;
  return created;
}
