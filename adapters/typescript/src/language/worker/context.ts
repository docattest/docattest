import type { WorkerMessage } from '../run/protocol.js';
import { QUIET_CONSOLE } from './console.js';
import type { HostClient } from './host-client.js';
import { ModuleRegistry } from './registry.js';
import type { Strays } from './strays.js';

/** What every fact expression may call besides the surface, each by its name here. */
export const FACT_FUNCTIONS = ['snapshot', 'manifest'] as const;

/** What a job executor needs from its worker. */
export interface WorkerContext {
  host: HostClient;
  strays: Strays;
  /** Snapshot data by name; throws for an unknown snapshot. */
  snapshot: (name: string) => unknown;
  /** Manifest data by path; throws for a manifest no fact names. */
  manifest: (path: string) => unknown;
  post: (message: WorkerMessage) => void;
}

/** A fresh registry whose modules see a silent console and the given free variables. */
export function freshRegistry(
  ctx: WorkerContext,
  globals: Record<string, unknown> = {},
): ModuleRegistry {
  return new ModuleRegistry(ctx.host, { console: QUIET_CONSOLE, ...globals });
}

/** Own enumerable properties of a module's exports, whatever shape they have. */
export function exportedEntries(exports: unknown): [string, unknown][] {
  if ((typeof exports === 'object' && exports !== null) || typeof exports === 'function') {
    return Object.entries(exports);
  }
  return [];
}
