// What the core prints for each command the API runs, checked at the top level before it is
// typed. The core is this package's own, so a shape that does not fit is a bug in one of
// the two; it is reported where the answer arrives, naming the command.
import type { AuditReport, InventoryEntry } from '../api/audit.js';
import { InternalError } from '../api/errors.js';
import type { Impact } from '../api/impact.js';
import type { Reference, Report } from '../api/report.js';
import { isObject, isStringArray } from '../json.js';

/** `check --json`: a report with its results, coverage and exit code. */
export const asReport = (value: unknown): Report =>
  fits(
    value,
    'check',
    (v) => isObject(v) && Array.isArray(v.results) && isObject(v.coverage),
  ) as Report;

/** `index --json`: references by symbol. */
export const asIndex = (value: unknown): Record<string, Reference[]> =>
  fits(value, 'index', (v) => isObject(v) && Object.values(v).every(Array.isArray)) as Record<
    string,
    Reference[]
  >;

/** `where --json`: the references to one symbol. */
export const asReferences = (value: unknown): Reference[] =>
  fits(value, 'where', Array.isArray) as Reference[];

/** `uncovered --json`: the names no page attests. */
export const asNames = (value: unknown): string[] =>
  fits(value, 'uncovered', isStringArray) as string[];

/** `audit --json`: the suggestions. */
export const asAudit = (value: unknown): AuditReport =>
  fits(value, 'audit', (v) => isObject(v) && Array.isArray(v.suggestions)) as AuditReport;

/** `inventory --json`: the surface values. */
export const asInventory = (value: unknown): InventoryEntry[] =>
  fits(value, 'inventory', Array.isArray) as InventoryEntry[];

/** `impact --json`: the symbols a change affects and the prose that names them. */
export const asImpact = (value: unknown): Impact =>
  fits(
    value,
    'impact',
    (v) => isObject(v) && isStringArray(v.symbols) && Array.isArray(v.references),
  ) as Impact;

function fits(value: unknown, command: string, shape: (value: unknown) => boolean): unknown {
  if (!shape(value)) {
    throw new InternalError(
      `the docattest core's ${command} answer is not of the shape this package reads`,
    );
  }
  return value;
}
