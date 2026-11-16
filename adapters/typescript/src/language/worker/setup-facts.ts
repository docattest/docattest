import { ambiguous, type FactItem } from '../run/protocol.js';
import { exportedEntries, FACT_FUNCTIONS } from './context.js';
import { describeError } from './describe.js';
import { offline } from './network.js';
import type { ModuleRegistry } from './registry.js';

const FACT_NAMES: ReadonlySet<string> = new Set(FACT_FUNCTIONS);

/**
 * Named facts from the setup module, loaded only when an expression needs a name the
 * surface does not export: loading runs the module's top level, and facts must stay pure.
 */
export class SetupFacts {
  private loaded: Map<string, unknown> | string | undefined;

  constructor(
    private readonly registry: ModuleRegistry,
    private readonly setup: { file: string; label: string } | undefined,
    private readonly surface: ReadonlyMap<string, unknown>,
  ) {}

  /** The names an expression may use, or why it cannot be evaluated. */
  async bindingsFor(fact: FactItem, surfaceName: string): Promise<Map<string, unknown> | string> {
    const needsHelpers = fact.identifiers.some(
      (n) => !this.surface.has(n) && !FACT_NAMES.has(n) && !(n in globalThis),
    );
    if (!needsHelpers || !this.setup) return new Map(this.surface);
    const helpers = await this.load(this.setup);
    if (typeof helpers === 'string') return helpers;
    const clash = fact.identifiers.find((n) => this.surface.has(n) && helpers.has(n));
    if (clash !== undefined) return ambiguous(clash, surfaceName);
    return new Map([...helpers, ...this.surface]);
  }

  private async load(setup: {
    file: string;
    label: string;
  }): Promise<Map<string, unknown> | string> {
    if (this.loaded !== undefined) return this.loaded;
    try {
      const exports = await offline(() => this.registry.load(setup.file));
      const facts = new Map(exportedEntries(exports)).get('facts');
      const valid = facts === undefined || (typeof facts === 'object' && facts !== null);
      this.loaded = valid
        ? new Map(exportedEntries(facts))
        : `${setup.label} exports \`facts\`, but it is not an object`;
    } catch (err) {
      this.loaded = `setup module ${setup.label} failed to load: ${describeError(err).message}`;
    }
    return this.loaded;
  }
}
