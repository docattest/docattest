import ts from '../ts.cjs';
import { isReference, type UnitLookup } from './lookup.js';
import { TOP_LEVEL, type Unit } from './units.js';

/**
 * Which units depend on which, in one program: a unit depends on every unit its code names
 * (calls, members, constants, types, base classes), resolved by the checker through imports
 * and re-exports. A module's top-level statements are its side effects at import: they
 * depend on what they name, and whatever they name depends on them in turn, because they
 * may change it (`registry.set(…)`).
 */
export class DependencyGraph {
  private readonly dependents = new Map<string, Set<string>>();
  private readonly seen = new Set<string>();

  constructor(private readonly lookup: UnitLookup) {}

  /** Follow every dependency from these units, and from every module's side effects. */
  build(roots: Iterable<string>): void {
    const queue = [...roots];
    for (const file of this.lookup.projectFiles()) {
      for (const unit of this.lookup.units(file).units.values()) {
        if (unit.key.endsWith(`#${TOP_LEVEL}`)) queue.push(unit.key);
      }
    }
    for (let key = queue.pop(); key !== undefined; key = queue.pop()) {
      if (this.seen.has(key)) continue;
      this.seen.add(key);
      const unit = this.lookup.unit(key);
      if (unit) queue.push(...this.follow(unit));
    }
  }

  /** Units that are, or depend at any depth on, one of the changed units. */
  tainted(changed: ReadonlySet<string>): Set<string> {
    const out = new Set<string>();
    const queue = [...changed];
    for (let key = queue.pop(); key !== undefined; key = queue.pop()) {
      if (out.has(key)) continue;
      out.add(key);
      queue.push(...(this.dependents.get(key) ?? []));
    }
    return out;
  }

  private follow(unit: Unit): string[] {
    const targets = new Set<string>();
    const visit = (node: ts.Node): void => {
      if (isReference(node)) for (const k of this.lookup.keysAt(node)) targets.add(k);
      ts.forEachChild(node, visit);
    };
    unit.nodes.forEach(visit);
    targets.delete(unit.key);
    const sideEffects = unit.key.endsWith(`#${TOP_LEVEL}`);
    for (const target of targets) {
      this.depend(unit.key, target);
      if (sideEffects) this.depend(target, unit.key);
    }
    return [...targets];
  }

  private depend(from: string, on: string): void {
    const set = this.dependents.get(on) ?? new Set<string>();
    set.add(from);
    this.dependents.set(on, set);
  }
}
