import * as path from 'node:path';
import type { SymbolRef } from '../api/adapter.js';
import type {
  Attestation,
  CasesAttestation,
  ExampleAttestation,
  FactAttestation,
  TestAttestation,
} from '../api/attestation.js';
import type { FileSource } from '../files/source.js';
import { citedSymbols } from './citations/listing.js';
import type { Compiler } from './compiler.js';
import { blockFile, isSourceFile, pagePath } from './languages.js';
import { expressionIdentifiers } from './syntax/expression.js';
import { helperSymbols, objectKeys } from './syntax/setup-module.js';
import { dedupe, importedSymbols, type SurfaceOf } from './syntax/imports.js';
import type { ResolvedSurface } from './surfaces/resolve.js';

const stem = (file: string) => file.replace(/\.[mc]?[jt]sx?$/, '');

/** Which surface symbols each attestation exercises, from syntax and the checker's exports. */
export class SymbolResolver {
  private readonly byImport: Map<string, string>;

  constructor(
    private readonly files: FileSource,
    private readonly surfaces: readonly ResolvedSurface[],
    private readonly compiler: Compiler,
    private readonly setup: string | undefined,
  ) {
    this.byImport = new Map(surfaces.map((s) => [s.spec.import, s.spec.name]));
  }

  /** Symbols of one attestation, each attributed to the surface it was reached through. */
  of(attestation: Attestation): SymbolRef[] {
    if (attestation.kind === 'example') return this.example(attestation);
    if (attestation.kind === 'fact') return attestation.expression ? this.fact(attestation) : [];
    if (attestation.kind === 'test') return this.citation(attestation);
    return this.cases(attestation);
  }

  private citation(t: TestAttestation): SymbolRef[] {
    const code = t.error === undefined ? this.files.read(t.file) : undefined;
    if (code === undefined || !isSourceFile(t.file)) return [];
    const file = pagePath(this.files, t.file);
    return this.known(citedSymbols(file, code, t, this.surfaceOf(file)));
  }

  private example(e: ExampleAttestation): SymbolRef[] {
    const file = blockFile(this.files.root, e);
    const imported = this.known(importedSymbols(file, e.code, (s) => this.byImport.get(s)));
    return e.owner === undefined ? imported : dedupe([...this.owner(e.page, e.owner), ...imported]);
  }

  // A doc-comment example attests its declaration on the surfaces that export that very
  // declaration; when none does (a surface that points at built output), on every
  // surface that exports the name; otherwise it names the symbol without a surface.
  private owner(page: string, name: string): SymbolRef[] {
    const file = pagePath(this.files, page);
    const names = this.surfaces.map((s) => s.spec.name);
    const declaring = names.filter((s) => this.compiler.origins(s).get(name)?.has(file));
    const exporting = names.filter((s) => this.compiler.exports(s)?.includes(name));
    const surfaces = declaring.length ? declaring : exporting;
    return surfaces.length ? surfaces.map((surface) => ({ name, surface })) : [{ name }];
  }

  private fact(f: FactAttestation): SymbolRef[] {
    const identifiers = expressionIdentifiers(f.expression);
    const exported = new Set(this.compiler.exports(f.surface) ?? []);
    const direct = identifiers
      .filter((n) => exported.has(n))
      .map((name) => ({ name, surface: f.surface }));
    const helpers = this.setupKeys('facts').filter(
      (k) => identifiers.includes(k) && !exported.has(k),
    );
    return dedupe([...direct, ...helpers.flatMap((h) => this.helper('facts', h))]);
  }

  private cases(c: CasesAttestation): SymbolRef[] {
    return this.helper('cases', c.runner);
  }

  private helper(group: string, key: string): SymbolRef[] {
    const code = this.setup === undefined ? undefined : this.files.read(this.setup);
    if (this.setup === undefined || code === undefined) return [];
    const file = pagePath(this.files, this.setup);
    return this.known(helperSymbols(file, code, group, key, this.surfaceOf(file)));
  }

  private setupKeys(group: string): string[] {
    const code = this.setup === undefined ? undefined : this.files.read(this.setup);
    return this.setup === undefined || code === undefined
      ? []
      : objectKeys(this.setup, code, group);
  }

  /** A setup module or test file may import a surface by specifier or by a relative path. */
  private surfaceOf(from: string): SurfaceOf {
    return (specifier) => {
      const named = this.byImport.get(specifier);
      if (named || !specifier.startsWith('.')) return named;
      const target = stem(path.resolve(path.dirname(from), specifier));
      return this.surfaces.find((s) => s.entry !== undefined && stem(s.entry) === target)?.spec
        .name;
    };
  }

  private known(refs: readonly SymbolRef[]): SymbolRef[] {
    return refs.filter(
      (r) => r.surface !== undefined && this.compiler.exports(r.surface)?.includes(r.name),
    );
  }
}
