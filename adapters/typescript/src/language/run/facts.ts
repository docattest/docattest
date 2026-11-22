import * as path from 'node:path';
import type { AdapterContext, Evaluation } from '../../api/adapter.js';
import type { FactAttestation } from '../../api/attestation.js';
import { expressionIdentifiers, expressionProblem } from '../syntax/expression.js';
import { objectKeys } from '../syntax/setup-module.js';
import type { SurfaceDeclarations } from '../surfaces/declarations.js';
import type { ResolvedSurface } from '../surfaces/resolve.js';
import { bindFactNames, type FactBinding } from './fact-names.js';
import { transpile } from './transpile.js';
import type { ItemResult } from './jobs.js';
import type { FactItem, Job } from './protocol.js';
import type { Runner } from './runner.js';

/** A fact whose names are bound, ready to run. */
type BoundFact = FactAttestation & { binding: Extract<FactBinding, { ok: true }> };

/**
 * Evaluates fact expressions: one job per page and surface (two when some of its facts read
 * only constants, which never load the module), one fresh registry per job.
 */
export class FactEvaluator {
  /** The names of the setup module's named facts, as its syntax declares them. */
  readonly helperNames: readonly string[];

  constructor(
    private readonly ctx: AdapterContext,
    private readonly surfaces: readonly ResolvedSurface[],
    private readonly runner: Runner,
    /** Statically known exports of a surface, by name. */
    private readonly exportsOf: (surface: string) => string[] | undefined,
    private readonly declarations: SurfaceDeclarations,
  ) {
    const setup = ctx.setup;
    const code = setup === undefined ? undefined : ctx.files.read(setup);
    this.helperNames =
      setup === undefined || code === undefined ? [] : objectKeys(setup, code, 'facts');
  }

  /** Evaluate every fact; each id maps to its value or to why there is none. */
  async evaluate(facts: readonly FactAttestation[]): Promise<Map<string, Evaluation>> {
    const values = new Map<string, Evaluation>();
    const groups = new Map<string, BoundFact[]>();
    for (const fact of facts) {
      const bound = this.bind(fact);
      if (typeof bound === 'string') values.set(fact.id, { ok: false, message: bound });
      else {
        // Facts that read only constants run apart, so a module that fails to load fails
        // only the facts that load it.
        const key = `${fact.page}\0${fact.surface}\0${String(bound.binding.loads)}`;
        groups.set(key, [...(groups.get(key) ?? []), bound]);
      }
    }
    await Promise.all([...groups.values()].map(async (group) => this.group(group, values)));
    return values;
  }

  /**
   * Why a surface's module cannot be loaded, or undefined when it loads. A fact that reads
   * only constants never loads it, so this is how `audit` learns that a surface is broken.
   */
  async loadFailure(surface: string): Promise<string | undefined> {
    const file = this.entryOf(surface);
    if (typeof file !== 'object') return file;
    const job: Job = {
      type: 'facts',
      surface,
      file: file.entry,
      constants: [],
      setup: undefined,
      facts: [],
    };
    const { failure } = await this.runner.execute(job, 0);
    return failure && failureMessage(surface, failure);
  }

  /** The module a surface runs from, or why it has none. */
  private entryOf(name: string): { entry: string } | string {
    const surface = this.surfaces.find((s) => s.spec.name === name);
    if (!surface) {
      const configured = this.surfaces.map((s) => s.spec.name).join(', ');
      return `unknown surface "${name}" (configured: ${configured})`;
    }
    return surface.entry
      ? { entry: surface.entry }
      : `surface "${name}" (${surface.spec.import}) not found`;
  }

  /** The fact with its names bound, or why it cannot be evaluated. */
  private bind(fact: FactAttestation): BoundFact | string {
    const noSurfaces = this.surfaces.length === 0 && fact.surface === 'default';
    if (!noSurfaces) {
      const entry = this.entryOf(fact.surface);
      if (typeof entry !== 'object') return entry;
    }
    const problem = expressionProblem(fact.expression);
    if (problem !== undefined) return problem;
    if (noSurfaces) {
      return { ...fact, binding: { ok: true, loads: false, constants: [] } };
    }
    // Checked here from the setup module's syntax, so a clash fails even when the surface
    // alone could evaluate the expression; the worker catches the dynamic cases.
    const binding = bindFactNames(expressionIdentifiers(fact.expression), {
      surface: fact.surface,
      exported: new Set(this.exportsOf(fact.surface) ?? []),
      helpers: new Set(this.helperNames),
      declared: this.declarations.of(fact.surface),
    });
    return binding.ok ? { ...fact, binding } : `fact \`${fact.expression}\`: ${binding.message}`;
  }

  private async group(facts: BoundFact[], values: Map<string, Evaluation>): Promise<void> {
    const [first] = facts;
    if (!first) return;
    const file = first.binding.loads
      ? this.surfaces.find((s) => s.spec.name === first.surface)?.entry
      : undefined;
    if (first.binding.loads && !file) return;
    const job: Job = {
      type: 'facts',
      surface: first.surface,
      file,
      constants: [...new Map(facts.flatMap((f) => f.binding.constants))],
      setup: this.setup(),
      facts: facts.map(item),
    };
    const { failure, items } = await this.runner.execute(job, facts.length);
    const failed = failure && failureMessage(first.surface, failure);
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

// A job's failure before any fact ran: the worker says why the module failed to load, but
// not when it ran out of time doing so.
function failureMessage(surface: string, failure: { message: string; timedOut: boolean }): string {
  return failure.timedOut
    ? `surface "${surface}" failed to load: ${failure.message}`
    : failure.message;
}

// Each expression becomes a module whose default export evaluates it, so `await` works.
function item(fact: FactAttestation): FactItem {
  const { code } = transpile('fact.ts', `export default async () => (\n${fact.expression}\n);`);
  return { expression: fact.expression, code, identifiers: expressionIdentifiers(fact.expression) };
}
