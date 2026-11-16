import * as path from 'node:path';
import type { AdapterContext, Evaluation } from '../../api/adapter.js';
import type { FactAttestation } from '../../api/attestation.js';
import { expressionIdentifiers, expressionProblem } from '../syntax/expression.js';
import { objectKeys } from '../syntax/setup-module.js';
import type { ResolvedSurface } from '../surfaces/resolve.js';
import { transpile } from './transpile.js';
import type { ItemResult } from './jobs.js';
import { ambiguous, type FactItem, type Job } from './protocol.js';
import type { Runner } from './runner.js';

/** Evaluates fact expressions: one job per page and surface, one fresh registry per job. */
export class FactEvaluator {
  /** The names of the setup module's named facts, as its syntax declares them. */
  readonly helperNames: readonly string[];

  constructor(
    private readonly ctx: AdapterContext,
    private readonly surfaces: readonly ResolvedSurface[],
    private readonly runner: Runner,
    /** Statically known exports of a surface, by name. */
    private readonly exportsOf: (surface: string) => string[] | undefined,
  ) {
    const setup = ctx.setup;
    const code = setup === undefined ? undefined : ctx.files.read(setup);
    this.helperNames =
      setup === undefined || code === undefined ? [] : objectKeys(setup, code, 'facts');
  }

  /** Evaluate every fact; each id maps to its value or to why there is none. */
  async evaluate(facts: readonly FactAttestation[]): Promise<Map<string, Evaluation>> {
    const values = new Map<string, Evaluation>();
    const groups = new Map<string, FactAttestation[]>();
    for (const fact of facts) {
      const problem = this.problem(fact);
      if (problem) values.set(fact.id, { ok: false, message: problem });
      else {
        groups.set(`${fact.page}\0${fact.surface}`, [
          ...(groups.get(`${fact.page}\0${fact.surface}`) ?? []),
          fact,
        ]);
      }
    }
    await Promise.all([...groups.values()].map(async (group) => this.group(group, values)));
    return values;
  }

  private problem(fact: FactAttestation): string | undefined {
    const surface = this.surfaces.find((s) => s.spec.name === fact.surface);
    if (!surface) {
      const configured = this.surfaces.map((s) => s.spec.name).join(', ');
      return `unknown surface "${fact.surface}" (configured: ${configured})`;
    }
    if (!surface.entry) return `surface "${fact.surface}" (${surface.spec.import}) not found`;
    return expressionProblem(fact.expression) ?? this.clash(fact);
  }

  // Caught here from the setup module's syntax, so a clash fails even when the surface
  // alone could evaluate the expression; the worker catches the dynamic cases.
  private clash(fact: FactAttestation): string | undefined {
    const exported = new Set(this.exportsOf(fact.surface) ?? []);
    const helpers = new Set(this.helperNames);
    const name = expressionIdentifiers(fact.expression).find(
      (n) => exported.has(n) && helpers.has(n),
    );
    return name === undefined
      ? undefined
      : `fact \`${fact.expression}\`: ${ambiguous(name, fact.surface)}`;
  }

  private async group(facts: FactAttestation[], values: Map<string, Evaluation>): Promise<void> {
    const [first] = facts;
    const file = this.surfaces.find((s) => s.spec.name === first?.surface)?.entry;
    if (!first || !file) return;
    const job: Job = {
      type: 'facts',
      surface: first.surface,
      file,
      setup: this.setup(),
      facts: facts.map(item),
    };
    const { failure, items } = await this.runner.execute(job, facts.length);
    const failed = failure?.timedOut
      ? `surface "${first.surface}" failed to load: ${failure.message}`
      : failure?.message;
    facts.forEach((fact, k) => {
      values.set(
        fact.id,
        failed === undefined ? this.evaluation(fact, items[k]) : { ok: false, message: failed },
      );
    });
  }

  private evaluation(fact: FactAttestation, item: ItemResult | undefined): Evaluation {
    const expression = `fact \`${fact.expression}\``;
    if (!item) return { ok: false, message: `${expression} was not evaluated` };
    if (item.kind === 'timeout') {
      return { ok: false, message: `${expression} timed out after ${this.ctx.timeoutMs} ms` };
    }
    if (item.kind === 'lost') return { ok: false, message: `${expression}: ${item.message}` };
    if (item.outcome.kind === 'evaluation') return item.outcome.evaluation;
    return { ok: false, message: `${expression}: the worker returned a verdict for a fact` };
  }

  private setup(): { file: string; label: string } | undefined {
    const setup = this.ctx.setup;
    if (setup === undefined || !this.ctx.files.exists(setup)) return undefined;
    return { file: path.join(this.ctx.files.root, setup), label: setup };
  }
}

// Each expression becomes a module whose default export evaluates it, so `await` works.
function item(fact: FactAttestation): FactItem {
  const { code } = transpile('fact.ts', `export default async () => (\n${fact.expression}\n);`);
  return { expression: fact.expression, code, identifiers: expressionIdentifiers(fact.expression) };
}
