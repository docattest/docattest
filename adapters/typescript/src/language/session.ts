import type { AdapterContext, AdapterSession } from '../api/adapter.js';
import { listTests } from './citations/listing.js';
import { Compiler } from './compiler.js';
import { surfaceObservables } from './inventory/observables.js';
import { affectedExports } from './impact/affected.js';
import { FactEvaluator } from './run/facts.js';
import { Runner } from './run/runner.js';
import { resolveSurfaces } from './surfaces/resolve.js';
import { surfaceSpecs } from './surfaces/specs.js';
import { namesInCode } from './syntax/lexical.js';
import { SymbolResolver } from './symbols.js';

/**
 * The TypeScript adapter for one check: a compiler program for `compile`, `symbols` and
 * `exports`, and a worker pool for `evaluate` and `run`. Nothing is built until used.
 */
export function createSession(ctx: AdapterContext): AdapterSession {
  const specs = surfaceSpecs(ctx.surfaces, ctx.files);
  const surfaces = resolveSurfaces(ctx.files, specs);
  const compiler = new Compiler(ctx.files, surfaces, ctx.examples);
  const symbols = new SymbolResolver(ctx.files, surfaces, compiler, ctx.setup);
  const runner = new Runner(ctx, surfaces);
  const facts = new FactEvaluator(ctx, surfaces, runner, (name) => compiler.exports(name));
  return {
    compile: (examples) => Promise.resolve(compiler.compile(examples)),
    evaluate: async (list) => facts.evaluate(list),
    run: async (examples, cases) => {
      const ran = await Promise.all([
        ...examples.map(async (e) => [await runner.example(e)]),
        ...cases.map(async (c) => runner.cases(c)),
      ]);
      return ran.flat();
    },
    symbols: (attestation) => Promise.resolve(symbols.of(attestation)),
    exports: (name) => Promise.resolve(compiler.exports(name)),
    tests: (file) => Promise.resolve(listTests(ctx.files, file)),
    observables: async (name) => surfaceObservables(facts, compiler.exports(name) ?? [], name),
    describeSurfaces: () =>
      Promise.resolve(new Map(surfaces.map((s) => [s.spec.name, s.description]))),
    affected: (change) => Promise.resolve(affectedExports(ctx.files, specs, change)),
    inCode: (names, files) => Promise.resolve(namesInCode(ctx.files, specs, names, files)),
    close: async () => runner.close(),
  };
}
