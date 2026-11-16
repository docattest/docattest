import type { Renames, Revision } from './revision.js';
import { TOP_LEVEL } from './units.js';

// Pairing compares vanished declarations with new ones; past this many candidate pairs a
// change is no refactor, and the pairs are not worth the time.
const MAX_PAIRS = 400;

/**
 * Declarations that vanished and reappeared under another name or in another module with
 * nothing else changed: a rename or a move, which no export can observe on its own.
 * `after` and `before` are fingerprints of the changed modules now and at the base.
 *
 * Moves keep their names, and a moved module's declarations use each other, so every
 * same-name pair is proposed at once and kept only while it holds under all the others.
 * A renamed declaration is then matched alone against what is left.
 */
export function renames(
  now: Revision,
  after: ReadonlyMap<string, string>,
  before: ReadonlyMap<string, string>,
): Renames {
  const added = movable(after, before);
  const removed = movable(before, after);
  if (!added.length || !removed.length || added.length * removed.length > MAX_PAIRS) {
    return new Map();
  }
  const holds = (map: Renames, key: string) =>
    now.fingerprintOf(key, map) === before.get(map.get(key) ?? '');
  const pairs = holding(sameNamePairs(added, removed), holds);
  for (const key of added.filter((k) => !pairs.has(k))) {
    const taken = new Set(pairs.values());
    const old = removed.find((o) => !taken.has(o) && holds(new Map([...pairs, [key, o]]), key));
    if (old !== undefined) pairs.set(key, old);
  }
  return pairs;
}

/** Keys of `keys` not in `other`: declarations, not a module's top level, which cannot move. */
function movable(keys: ReadonlyMap<string, string>, other: ReadonlyMap<string, string>): string[] {
  return [...keys.keys()].filter((k) => !other.has(k) && !k.endsWith(`#${TOP_LEVEL}`));
}

/** Each added key with exactly one removed key of the same name: a move, proposed. */
function sameNamePairs(added: readonly string[], removed: readonly string[]): Map<string, string> {
  const pairs = new Map<string, string>();
  for (const key of added) {
    const same = removed.filter((old) => nameOf(old) === nameOf(key));
    if (same.length === 1 && same[0] !== undefined) pairs.set(key, same[0]);
  }
  return pairs;
}

/** The pairs, less those that fail under the others, until every one left holds. */
function holding(
  pairs: Map<string, string>,
  holds: (map: Renames, key: string) => boolean,
): Map<string, string> {
  for (let dropped = true; dropped;) {
    const failing = [...pairs.keys()].filter((key) => !holds(pairs, key));
    for (const key of failing) pairs.delete(key);
    dropped = failing.length > 0;
  }
  return pairs;
}

function nameOf(key: string): string {
  return key.slice(key.lastIndexOf('#') + 1);
}
