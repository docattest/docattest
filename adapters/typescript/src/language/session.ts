import type { AdapterContext, AdapterSession, SurfaceDescription } from '../api/adapter.js';
import type { FileSource } from '../files/source.js';
import { listTests } from './citations/listing.js';
import { Compiler } from './compiler.js';
import { surfaceObservables } from './inventory/observables.js';
import { affectedExports } from './impact/affected.js';
import { symbolMutants } from './mutants/symbol-mutants.js';
import { FactEvaluator } from './run/facts.js';
import { Runner } from './run/runner.js';
import { SurfaceDeclarations } from './surfaces/declarations.js';
import { resolveSurfaces, type ResolvedSurface } from './surfaces/resolve.js';
import { surfaceSpecs } from './surfaces/specs.js';
import { workspaceBuildWarnings } from './surfaces/workspace-builds.js';
import { namesInCode } from './syntax/lexical.js';
import { SymbolResolver } from './symbols.js';
import { runCitedTests } from './test-runs/run-tests.js';

/**
 * The TypeScript adapter for one check: a compiler program for `compile`, `symbols` and
 * `exports`, and a worker pool for `evaluate` and `run`. Nothing is built until used.
 */
export function createSession(ctx: AdapterContext): AdapterSession {
  const specs = surfaceSpecs(ctx.surfaces, ctx.files);
  const surfaces = resolveSurfaces(ctx.files, specs);
  const compiler = new Compiler(ctx.files, surfaces, ctx.examples);
  const declarations = new SurfaceDeclarations(ctx.files, surfaces);
  const symbols = new SymbolResolver(ctx.files, surfaces, compiler, ctx.setup, declarations);
  const runner = new Runner(ctx, surfaces);
  const exportsOf = (name: string) => compiler.exports(name);
  const facts = new FactEvaluator(ctx, surfaces, runner, exportsOf, declarations);
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
    runTests: (tests, timeoutMs) =>
      runCitedTests(ctx.files, tests, { timeoutMs, workers: ctx.workers }),
    observables: async (name) => surfaceObservables(facts, compiler.exports(name) ?? [], name),
    describeSurfaces: () => Promise.resolve(describeSurfaces(ctx.files, surfaces)),
    affected: (change) => Promise.resolve(affectedExports(ctx.files, specs, change)),
    inCode: (names, files) => Promise.resolve(namesInCode(ctx.files, specs, names, files)),
    mutants: (refs, limit) => Promise.resolve(symbolMutants(ctx.files, specs, refs, limit)),
    close: async () => runner.close(),
  };
}

// Only `check` asks for descriptions, so the imports behind the surfaces are walked then.
function describeSurfaces(
  files: FileSource,
  surfaces: readonly ResolvedSurface[],
): Map<string, SurfaceDescription> {
  const buildWarnings = workspaceBuildWarnings(files);
  return new Map(
    surfaces.map((surface) => {
      const { description } = surface;
      const warnings = [...description.warnings, ...buildWarnings(surface)];
      return [surface.spec.name, { ...description, warnings }];
    }),
  );
}
