import type { ModuleName } from '../syntax/module-names.js';
import { ALL_FACT_FUNCTIONS } from '../worker/context.js';
import { ambiguous } from './protocol.js';

/** What a fact's names may be bound to, by name. */
export interface FactScope {
  surface: string;
  /** The surface's exports, as the checker reads them. */
  exported: ReadonlySet<string>;
  /** The setup module's named facts, as its syntax declares them. */
  helpers: ReadonlySet<string>;
  /** What the surface's module declares at its top level. */
  declared: ReadonlyMap<string, ModuleName>;
}

/** How a fact's names are bound, or why they cannot be. */
export type FactBinding =
  | { ok: false; message: string }
  | {
      ok: true;
      /** The constants it reads from source, with their values. */
      constants: [string, unknown][];
      /** Whether it needs the surface's module loaded: it names something else of it. */
      loads: boolean;
    };

const BUILT_IN: ReadonlySet<string> = new Set(ALL_FACT_FUNCTIONS);

/**
 * Bind a fact's names. A constant of the surface's module is read from source, exported or
 * not, so a fact that reads only constants never loads the module; any other name the
 * module declares must be exported. An export or a constant that is also a setup fact is
 * ambiguous.
 */
export function bindFactNames(names: readonly string[], scope: FactScope): FactBinding {
  const constants: [string, unknown][] = [];
  let loads = false;
  for (const name of names) {
    const problem = conflict(name, scope);
    if (problem !== undefined) return { ok: false, message: problem };
    const declared = scope.declared.get(name);
    if (declared?.kind === 'constant') constants.push([name, declared.value]);
    else loads ||= !BUILT_IN.has(name) && !(name in globalThis);
  }
  return { ok: true, constants, loads };
}

function conflict(name: string, scope: FactScope): string | undefined {
  const { surface, exported, helpers } = scope;
  const declared = scope.declared.get(name);
  if (exported.has(name)) return helpers.has(name) ? ambiguous(name, surface) : undefined;
  if (declared?.kind === 'constant') {
    return helpers.has(name)
      ? `"${name}" is both a constant of surface "${surface}" and a setup fact; ` +
          'rename the setup fact'
      : undefined;
  }
  if (!declared || helpers.has(name)) return undefined;
  const why =
    declared.kind === 'computed'
      ? `${name} is a const that surface "${surface}" does not export, and the module ` +
        'computes or changes its value as it runs'
      : `${name} is ${declared.what} that surface "${surface}" does not export`;
  return `${why}; of its unexported names, a fact reads only constants`;
}
