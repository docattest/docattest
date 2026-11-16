import type { SourceChange, SymbolRef } from '../../api/adapter.js';
import type { SurfaceSpec } from '../surfaces/specs.js';
import type { FileSource } from '../../files/source.js';
import { compareStrings } from '../../compare-strings.js';
import { Revision, type Renames } from './revision.js';
import { renames } from './renames.js';

const CODE = /\.(d\.)?[mc]?[jt]sx?$/;

/**
 * Surface exports a change affects: those whose own declaration changed, those that reach
 * a changed declaration through the code they use (in the working tree or at the base,
 * so a dependency the change removed still counts), and those that appeared, disappeared
 * or now stand for a different declaration. Nothing is built when no code changed.
 */
export function affectedExports(
  files: FileSource,
  specs: readonly SurfaceSpec[],
  change: SourceChange,
): SymbolRef[] {
  const code = change.changed.filter((f) => CODE.test(f) && !/(^|\/)node_modules\//.test(f));
  if (!code.length) return [];
  const now = new Revision(files, specs);
  const then = new Revision(change.base, specs);
  const before = then.fingerprints(code);
  const renamed = renames(now, now.fingerprints(code), before);
  const changed = changedUnits(now.fingerprints(code, renamed), before);
  const [nowExports, thenExports] = [now.exports(), then.exports()];
  const hit = new Set(remapped(nowExports, thenExports, renamed));
  if (changed.size) {
    for (const id of now.affected(nowExports, asNow(changed, renamed))) hit.add(id);
    for (const id of then.affected(thenExports, changed)) hit.add(id);
  }
  return [...hit].sort(compareStrings).map((id) => {
    const [surface = '', name = ''] = id.split('\0');
    return { name, surface };
  });
}

// Changed units are named as at the base; the working tree's graph knows them by now.
function asNow(changed: ReadonlySet<string>, renamed: Renames): Set<string> {
  const out = new Set(changed);
  for (const [key, was] of renamed) if (changed.has(was)) out.add(key);
  return out;
}

function changedUnits(now: Map<string, string>, then: Map<string, string>): Set<string> {
  const keys = new Set([...now.keys(), ...then.keys()]);
  return new Set([...keys].filter((k) => now.get(k) !== then.get(k)));
}

function remapped(
  now: Map<string, Set<string>>,
  then: Map<string, Set<string>>,
  renamed: Renames,
): string[] {
  const ids = new Set([...now.keys(), ...then.keys()]);
  const same = (a = new Set<string>(), b = new Set<string>()) =>
    a.size === b.size && [...a].every((k) => b.has(renamed.get(k) ?? k));
  return [...ids].filter((id) => !same(now.get(id), then.get(id)));
}
