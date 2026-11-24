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
import type { SurfaceDeclarations } from './surfaces/declarations.js';
import { nearestPackage, sameDirectory } from './surfaces/packages.js';
import type { ResolvedSurface } from './surfaces/resolve.js';

const stem = (file: string) => file.replace(/\.[mc]?[jt]sx?$/, '');
const sameModule = (file: string, target: string) => {
  const s = stem(file);
  return s === target || s === `${target}${path.sep}index`;
};

/** Which surface symbols each attestation exercises, from syntax and the checker's exports. */
export class SymbolResolver {
  private readonly byImport: Map<string, string>;

  constructor(
    private readonly files: FileSource,
    private readonly surfaces: readonly ResolvedSurface[],
    private readonly compiler: Compiler,
    private readonly setup: string | undefined,
    private readonly declarations: SurfaceDeclarations,
  ) {
    this.byImport = new Map(surfaces.map((s) => [s.spec.import, s.spec.name]));
  }

  /** Symbols of one attestation, each attributed to the surface it was reached through. */
  of(attestation: Attestation): SymbolRef[] {
    if (attestation.kind === 'example') return this.example(attestation);
    if (attestation.kind === 'test') return this.citation(attestation);
    return this.withInitializerNames(this.read(attestation));
  }

  /** What a fact or a cases runner reads of the surfaces. */
  private read(proof: FactAttestation | CasesAttestation): SymbolRef[] {
    if (proof.kind === 'cases') return this.cases(proof);
    return proof.expression ? this.fact(proof) : [];
  }

  // A value holds the values of the names its initializer uses, so a proof that reads it
  // attests those too, at any depth. A function's body is not followed: calling it reads
  // what it returns, not what it is made of.
  private withInitializerNames(refs: readonly SymbolRef[]): SymbolRef[] {
    const used = refs.flatMap((r) =>
      r.surface === undefined ? [] : this.initializerNames(r.name, r.surface),
    );
    return dedupe([...refs, ...used]);
  }

  /** The names of a surface a fact may attest that `name`'s initializer uses, at any depth. */
  private initializerNames(name: string, surface: string): SymbolRef[] {
    const uses = this.declarations.initializerUses(surface);
    const reached = new Set<string>();
    const queue = [name];
    for (let next = queue.pop(); next !== undefined; next = queue.pop()) {
      const unseen = (uses.get(next) ?? []).filter((n) => n !== name && !reached.has(n));
      for (const n of unseen) reached.add(n);
      queue.push(...unseen);
    }
    return [...reached]
      .filter((n) => this.attestable(surface, n))
      .map((n) => ({ name: n, surface }));
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
      .filter((n) => this.attestable(f.surface, n))
      .map((name) => ({ name, surface: f.surface }));
    const helpers = this.setupKeys('facts').filter(
      (k) => identifiers.includes(k) && !exported.has(k),
    );
    return dedupe([...direct, ...helpers.flatMap((h) => this.helper('facts', h))]);
  }

  // A constant a fact reads from source is attested as an export is, exported or not.
  private attestable(surface: string, name: string): boolean {
    const exported = this.compiler.exports(surface)?.includes(name) ?? false;
    return exported || this.declarations.of(surface).get(name)?.kind === 'constant';
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

  /**
   * A setup module or test file may import a surface by package specifier, by a relative path
   * to the surface's entry, or by a relative path to a submodule whose exports that surface
   * re-exports (including source modules behind a built `dist/` surface of the same package).
   */
  private surfaceOf(from: string): SurfaceOf {
    return (specifier, name) => {
      const named = this.byImport.get(specifier);
      if (named || !specifier.startsWith('.')) return named;
      const target = stem(path.resolve(path.dirname(from), specifier));
      const direct = this.surfaces.find(
        (s) => s.entry !== undefined && sameModule(s.entry, target),
      );
      if (direct) return direct.spec.name;
      return this.surfaceByOrigin(target, name) ?? this.surfaceByPackage(target, name);
    };
  }

  private surfaceByOrigin(target: string, name: string | undefined): string | undefined {
    for (const s of this.surfaces) {
      const origins = this.compiler.origins(s.spec.name);
      if (name !== undefined) {
        const files = origins.get(name);
        if (files && [...files].some((f) => sameModule(f, target))) return s.spec.name;
      } else {
        for (const files of origins.values()) {
          if ([...files].some((f) => sameModule(f, target))) return s.spec.name;
        }
      }
    }
    return undefined;
  }

  private surfaceByPackage(target: string, name: string | undefined): string | undefined {
    const pkg = nearestPackage(this.files, `${target}.ts`);
    if (!pkg) return undefined;
    for (const s of this.surfaces) {
      if (s.entry === undefined) continue;
      const surfacePkg = nearestPackage(this.files, s.entry);
      if (!surfacePkg || !sameDirectory(this.files, pkg.dir, surfacePkg.dir)) continue;
      if (name === undefined || this.compiler.exports(s.spec.name)?.includes(name)) {
        return s.spec.name;
      }
    }
    return undefined;
  }

  private known(refs: readonly SymbolRef[]): SymbolRef[] {
    return refs.filter(
      (r) => r.surface !== undefined && this.compiler.exports(r.surface)?.includes(r.name),
    );
  }
}
