import type { Evaluation } from '../../api/adapter.js';
import { observe } from './observe.js';
import type { FactItem, FactsJob } from '../run/protocol.js';
import { InternalError } from '../../api/errors.js';
import { compileFunction, type CompiledFunction } from './compile.js';
import { exportedEntries, FACT_FUNCTIONS, freshRegistry, type WorkerContext } from './context.js';
import { describeError } from './describe.js';
import { offline } from './network.js';
import type { ModuleRegistry } from './registry.js';
import { SetupFacts } from './setup-facts.js';

const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;
const RESERVED = new Set(
  [
    'break case catch class const continue debugger default delete do else enum export extends',
    'false finally for function if import in instanceof new null return super switch this throw',
    'true try typeof var void while with yield let static implements interface package private',
    'protected public await arguments eval',
    ...FACT_FUNCTIONS,
  ]
    .join(' ')
    .split(' '),
);

/** Load the surface once, then evaluate each fact from `start` against it. */
export async function runFacts(
  id: string,
  job: FactsJob,
  start: number,
  ctx: WorkerContext,
): Promise<void> {
  const registry = freshRegistry(ctx, { snapshot: ctx.snapshot });
  const exports = await loadSurface(registry, job);
  if (typeof exports === 'string') {
    ctx.post({ type: 'failed', id, message: exports });
    return;
  }
  const surface = new Map([...exports, ...job.constants]);
  ctx.post({ type: 'ready', id });
  const helpers = new SetupFacts(registry, job.setup, surface);
  for (const fact of job.facts.slice(start)) {
    const evaluation = await evaluate(fact, job.surface, helpers, ctx);
    ctx.post({ type: 'item', id, outcome: { kind: 'evaluation', evaluation } });
  }
}

// A job whose facts read only constants from source never runs the surface's module.
async function loadSurface(
  registry: ModuleRegistry,
  job: FactsJob,
): Promise<[string, unknown][] | string> {
  const { file } = job;
  if (file === undefined) return [];
  try {
    return exportedEntries(await offline(() => registry.load(file)));
  } catch (err) {
    return `surface "${job.surface}" failed to load: ${describeError(err).message}`;
  }
}

async function evaluate(
  fact: FactItem,
  surface: string,
  helpers: SetupFacts,
  ctx: WorkerContext,
): Promise<Evaluation> {
  const label = `fact \`${fact.expression}\``;
  const bindings = await helpers.bindingsFor(fact, surface);
  if (typeof bindings === 'string') return { ok: false, message: `${label}: ${bindings}` };
  const names = [...bindings.keys()].filter((n) => IDENTIFIER.test(n) && !RESERVED.has(n));
  try {
    const evaluateFact = factFunction(names, fact.code)(
      ...names.map((n) => bindings.get(n)),
      ...FACT_FUNCTIONS.map((n) => ctx[n]),
    );
    if (typeof evaluateFact !== 'function') {
      throw new InternalError('the fact module exported no function');
    }
    const value: unknown = await offline(
      () => Reflect.apply(evaluateFact, undefined, []) as unknown,
    );
    return { ok: true, value: observe(value) };
  } catch (err) {
    return { ok: false, message: evaluationError(err, label, surface) };
  }
}

// A free identifier the surface does not export is the commonest mistake: say so.
function evaluationError(err: unknown, label: string, surface: string): string {
  const missing = err instanceof ReferenceError ? /^(\S+) is not defined$/.exec(err.message) : null;
  if (!missing || !(err instanceof Error)) return `${label} threw ${describeError(err).message}`;
  const name = missing[1] ?? '';
  const where = `surface "${surface}"`;
  return `${label}: ${name} is not exported by ${where} (ReferenceError: ${err.message})`;
}

// The fact module's code is ours: a transpiled `export default async () => (expression)`.
function factFunction(names: readonly string[], code: string): CompiledFunction {
  const body = `"use strict"; const exports = {}; ${code}; return exports.default;`;
  return compileFunction(
    `(function (${[...names, ...FACT_FUNCTIONS].join(', ')}) {${body}\n})`,
    'fact.js',
  );
}
