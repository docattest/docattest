import * as path from 'node:path';
import type { SurfaceSpec } from '../surfaces/specs.js';
import type { FileSource } from '../../files/source.js';
import { fingerprint } from './fingerprint.js';
import { DependencyGraph } from './graph.js';
import { UnitLookup } from './lookup.js';
import { surfacePrograms } from './program.js';

/** A unit key now → its key at the base, for declarations renamed or moved unchanged. */
export type Renames = ReadonlyMap<string, string>;

/** One revision of the project (the base or the working tree), seen through the checker. */
export class Revision {
  readonly lookup: UnitLookup;
  private readonly entries: Map<string, string>;
  private readonly root: string;

  constructor(files: FileSource, specs: readonly SurfaceSpec[]) {
    const { program, entries } = surfacePrograms(files, specs);
    this.root = files.root;
    this.lookup = new UnitLookup(program, files.root);
    this.entries = entries;
  }

  /**
   * Fingerprints of every unit in the given modules that this revision's program holds,
   * keyed (and naming what they use) as the base would, through `renamed`.
   */
  fingerprints(files: readonly string[], renamed: Renames = new Map()): Map<string, string> {
    const out = new Map<string, string>();
    for (const rel of files) {
      const source = this.lookup.program.getSourceFile(path.join(this.root, rel));
      if (!source) continue;
      for (const unit of this.lookup.units(source).units.values()) {
        out.set(renamed.get(unit.key) ?? unit.key, fingerprint(unit, this.lookup, renamed));
      }
    }
    return out;
  }

  /** One unit's fingerprint, once {@link fingerprints} has seen its module. */
  fingerprintOf(key: string, renamed: Renames): string | undefined {
    const unit = this.lookup.unit(key);
    return unit && fingerprint(unit, this.lookup, renamed);
  }

  /** `surface\0name` → the units each export of each surface stands for. */
  exports(): Map<string, Set<string>> {
    const out = new Map<string, Set<string>>();
    const { program, checker } = this.lookup;
    for (const [surface, entry] of this.entries) {
      const source = program.getSourceFile(entry);
      const module = source && checker.getSymbolAtLocation(source);
      for (const symbol of module ? checker.getExportsOfModule(module) : []) {
        const keys = this.lookup.keysOfSymbol(this.lookup.resolveAlias(symbol));
        out.set(`${surface}\0${symbol.name}`, new Set(keys));
      }
    }
    return out;
  }

  /** Exports that are, or depend on, a changed unit. */
  affected(exported: Map<string, Set<string>>, changed: ReadonlySet<string>): string[] {
    const graph = new DependencyGraph(this.lookup);
    graph.build([...exported.values()].flatMap((keys) => [...keys]));
    const tainted = graph.tainted(changed);
    return [...exported]
      .filter(([, keys]) => [...keys].some((k) => tainted.has(k)))
      .map(([e]) => e);
  }
}
