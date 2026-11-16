// What `audit` compares pages with: the values each export offers, nested ones included, each
// with the fact expression that reads it, and on the default surface what the setup module's
// named facts return. One probe per export or helper travels the ordinary fact path; `audit`
// evaluates whatever it suggests again, through its own expression.
import type { Evaluation, Observable } from '../../api/adapter.js';
import type { FactAttestation } from '../../api/attestation.js';
import { UserError } from '../../api/errors.js';
import { planExport } from './plan.js';
import { helperProbeExpression, parseHelperProbe, parseProbe, probeExpression } from './probe.js';
import type { FactEvaluator } from '../run/facts.js';

const PAGE = '__docattest_audit__';
const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;
/** Evaluates only if the surface loads: its failure is the surface's. */
const SENTINEL = '0';
/** The surface a fact without `@` reads (PROTOCOL.md: `observables`). */
const DEFAULT_SURFACE = 'default';

/**
 * Observables of one surface's exports, each with the expression that reads it; on the
 * default surface, also of each named fact of the setup module that takes no arguments,
 * called. A helper that shares its name with an export is left out: a fact cannot use it.
 */
export async function surfaceObservables(
  evaluator: FactEvaluator,
  names: readonly string[],
  surface: string,
): Promise<Observable[]> {
  const exported = names.filter((n) => IDENTIFIER.test(n) && n !== 'default');
  const helpers =
    surface === DEFAULT_SURFACE
      ? evaluator.helperNames.filter((n) => IDENTIFIER.test(n) && !names.includes(n))
      : [];
  const [loaded, ...probes] = await evaluateAll(evaluator, surface, [
    SENTINEL,
    ...exported.map(probeExpression),
    ...helpers.map(helperProbeExpression),
  ]);
  if (!loaded?.ok) {
    throw new UserError(`audit: ${loaded?.message ?? `surface "${surface}" did not evaluate`}`);
  }
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
  return [...ofExports, ...ofHelpers].map((p) => ({ surface, ...p }));
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
