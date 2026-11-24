import type ts from './ts.cjs';
import type { Diagnostic } from '../api/adapter.js';
import type { ExampleAttestation } from '../api/attestation.js';
import type { FileSource } from '../files/source.js';
import { blockFile } from './languages.js';
import { exportOrigins, exportsOf } from './program/exports.js';
import { EMPTY_TYPE_FACTS, surfaceTypeFacts, type SurfaceTypeFacts } from './program/types.js';
import {
  createProgram,
  diagnosticsFor,
  resolveTypes,
  type ProgramPurpose,
} from './program/program.js';
import type { ResolvedSurface } from './surfaces/resolve.js';

/**
 * The checker's view of a project for the session: a program that type-checks examples and
 * a lighter one that lists exports, each built on first use and parsing the project once.
 */
export class Compiler {
  private readonly programs = new Map<ProgramPurpose, ts.Program>();
  private readonly parsed = new Map<string, ts.SourceFile>();
  private readonly exportCache = new Map<string, string[] | undefined>();
  private readonly originCache = new Map<string, Map<string, Set<string>>>();
  private readonly typeFactsCache = new Map<string, SurfaceTypeFacts>();

  constructor(
    private readonly files: FileSource,
    private readonly surfaces: readonly ResolvedSurface[],
    private readonly examples: readonly ExampleAttestation[],
  ) {}

  /** Diagnostics for examples; each must be one the session was created with. */
  compile(examples: readonly ExampleAttestation[]): Diagnostic[] {
    // The core asks even when a check has no examples; the program would cost its lib files.
    if (examples.length === 0) return [];
    const program = this.getProgram('check');
    return examples.flatMap((e) =>
      diagnosticsFor(program, blockFile(this.files.root, e)).map((d) => ({ id: e.id, ...d })),
    );
  }

  /** Exported names of a surface, or undefined when it cannot be resolved. */
  exports(name: string): string[] | undefined {
    if (!this.exportCache.has(name)) {
      const file = this.typesFile(name);
      this.exportCache.set(name, file ? exportsOf(this.getProgram('exports'), file) : undefined);
    }
    return this.exportCache.get(name);
  }

  /** For each export of a surface, the files declaring what it refers to. */
  origins(name: string): Map<string, Set<string>> {
    let origins = this.originCache.get(name);
    if (!origins) {
      const file = this.typesFile(name);
      origins = file
        ? exportOrigins(this.getProgram('exports'), file)
        : new Map<string, Set<string>>();
      this.originCache.set(name, origins);
    }
    return origins;
  }

  /** Type-level keys and literal union values of a surface's exported declarations. */
  typeFacts(name: string): SurfaceTypeFacts {
    let cached = this.typeFactsCache.get(name);
    if (!cached) {
      const file = this.typesFile(name);
      cached = file ? surfaceTypeFacts(this.getProgram('exports'), file) : EMPTY_TYPE_FACTS;
      this.typeFactsCache.set(name, cached);
    }
    return cached;
  }

  private typesFile(name: string): string | undefined {
    const surface = this.surfaces.find((s) => s.spec.name === name);
    if (!surface) return undefined;
    if (surface.spec.path !== undefined) return surface.types;
    return this.files.memory
      ? undefined
      : resolveTypes(this.getProgram('exports'), surface.spec.import, this.files.root);
  }

  private getProgram(purpose: ProgramPurpose): ts.Program {
    const built = this.programs.get(purpose);
    if (built) return built;
    const program = createProgram(
      {
        files: this.files,
        examples: new Map(this.examples.map((e) => [blockFile(this.files.root, e), e.code])),
        surfaces: new Map(
          this.surfaces.flatMap((s) =>
            s.types === undefined ? [] : [[s.spec.import, s.types] as const],
          ),
        ),
        parsed: this.parsed,
      },
      purpose,
    );
    this.programs.set(purpose, program);
    return program;
  }
}
