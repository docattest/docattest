// What `audit` compares pages with: the values each export offers, nested ones included, each
// with the fact expression that reads it, and on the default surface what the setup module's
// named facts return. One probe per export or helper travels the ordinary fact path; `audit`
// evaluates whatever it suggests again, through its own expression.
import type { Evaluation, Observable } from '../../api/adapter.js';
import type { FactAttestation } from '../../api/attestation.js';
import { UserError } from '../../api/errors.js';
import { planExport } from './plan.js';
import { helperProbeExpression, parseHelperProbe, parseProbe, probeExpression } from './probe.js';
import type { SurfaceTypeFacts } from '../program/types.js';
import type { FactEvaluator } from '../run/facts.js';

const PAGE = '__docattest_audit__';
const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;
/** The surface a fact without `@` reads (PROTOCOL.md: `observables`). */
const DEFAULT_SURFACE = 'default';
/** Bound type-level inventory entries so huge generated interfaces do not dominate audit. */
const MAX_TYPE_ITEMS = 64;

/**
 * Observables of one surface's exports, each with the expression that reads it; on the
 * default surface, also of each named fact of the setup module that takes no arguments,
 * called. A helper that shares its name with an export is left out: a fact cannot use it.
 */
export async function surfaceObservables(
  evaluator: FactEvaluator,
  names: readonly string[],
  surface: string,
  typeFacts?: SurfaceTypeFacts,
): Promise<Observable[]> {
  const exported = names.filter((n) => IDENTIFIER.test(n) && n !== 'default');
  const helpers =
    surface === DEFAULT_SURFACE
      ? evaluator.helperNames.filter((n) => IDENTIFIER.test(n) && !names.includes(n))
      : [];
  // A surface whose module does not load offers nothing, even the constants it declares.
  const [failure, probes] = await Promise.all([
    evaluator.loadFailure(surface),
    evaluateAll(evaluator, surface, [
      ...exported.map(probeExpression),
      ...helpers.map(helperProbeExpression),
    ]),
  ]);
  if (failure !== undefined) throw new UserError(failure);
  const ofExports = exported.flatMap((name, k) => {
    const probe = probes[k];
    const nodes = probe?.ok ? parseProbe(probe.value.text) : undefined;
    return planExport(name, nodes ?? []);
  });
  const ofHelpers = helpers.flatMap((name, k) => {
    const probe = probes[exported.length + k];
    const found = probe?.ok ? parseHelperProbe(probe.value.text) : undefined;
    if (!found) return [];
    return planExport(found.awaited ? `(await ${name}())` : `${name}()`, found.nodes);
  });
  const ofTypes = typeFacts ? typeObservables(exported, typeFacts) : [];
  return [...ofExports, ...ofHelpers, ...ofTypes].map((p) => ({ surface, ...p }));
}

function typeObservables(
  exported: readonly string[],
  typeFacts: SurfaceTypeFacts,
): Omit<Observable, 'surface'>[] {
  const out: Omit<Observable, 'surface'>[] = [];
  for (const name of exported) {
    if (!typeFacts.typeOnly.has(name)) continue;
    const unionValues = typeFacts.values.get(name);
    if (unionValues && unionValues.length >= 2 && unionValues.length <= MAX_TYPE_ITEMS) {
      out.push(typeList(`typeValues('${name}')`, name, unionValues));
      continue;
    }
    const propKeys = typeFacts.keys.get(name);
    if (propKeys && propKeys.length >= 2 && propKeys.length <= MAX_TYPE_ITEMS) {
      out.push(typeList(`typeKeys('${name}')`, name, propKeys));
    }
  }
  return out;
}

function typeList(
  expression: string,
  name: string,
  items: readonly string[],
): Omit<Observable, 'surface'> {
  const list = [...items];
  return {
    expression,
    path: [name],
    value: { text: list.join(','), items: list, description: 'an array' },
  };
}

async function evaluateAll(
  evaluator: FactEvaluator,
  surface: string,
  expressions: readonly string[],
): Promise<(Evaluation | undefined)[]> {
  const facts = expressions.map((expression, k): FactAttestation => ({
    id: `${PAGE}:${surface}:${k}`,
    kind: 'fact',
    page: PAGE,
    line: k + 1,
    claim: '',
    expression,
    surface,
    ordered: false,
  }));
  const values = await evaluator.evaluate(facts);
  return facts.map((f) => values.get(f.id));
}
