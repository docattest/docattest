import ts from '../ts.cjs';
import type { SymbolRef } from '../../api/adapter.js';
import { parseSource } from './parse.js';

/** Maps an import specifier (and optional imported export name) to the surface it names, if any. */
export type SurfaceOf = (specifier: string, name?: string) => string | undefined;

/** Surface symbols a module imports: named, default and namespace member accesses. */
export function importedSymbols(fileName: string, code: string, surfaceOf: SurfaceOf): SymbolRef[] {
  const source = parseSource(fileName, code);
  const refs: SymbolRef[] = [];
  const namespaces = new Map<string, string>();
  for (const bound of surfaceImports(source, surfaceOf)) {
    if (bound.kind === 'namespace') namespaces.set(bound.local, bound.surface);
    else refs.push({ name: bound.name, surface: bound.surface });
  }
  if (namespaces.size) refs.push(...namespaceMembers(source, namespaces));
  return dedupe(refs);
}

/** A local name an import binds to a surface: one of its symbols, or the whole of it. */
type SurfaceImport =
  | { kind: 'symbol'; local: string; name: string; surface: string }
  | { kind: 'namespace'; local: string; surface: string };

/** What a module's import declarations bind from surfaces, in source order. */
export function surfaceImports(source: ts.SourceFile, surfaceOf: SurfaceOf): SurfaceImport[] {
  return source.statements.flatMap((statement) => {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) {
      return [];
    }
    return clauseImports(statement.importClause, statement.moduleSpecifier.text, surfaceOf);
  });
}

// The default import first, then the named imports or the namespace, as written.
function clauseImports(
  clause: ts.ImportClause | undefined,
  specifier: string,
  surfaceOf: SurfaceOf,
): SurfaceImport[] {
  if (!clause) return [];
  const found: SurfaceImport[] = [];
  if (clause.name) {
    const surface = surfaceOf(specifier, 'default');
    if (surface) found.push({ kind: 'symbol', local: clause.name.text, name: 'default', surface });
  }
  if (clause.namedBindings) {
    found.push(...bindingImports(clause.namedBindings, specifier, surfaceOf));
  }
  return found;
}

function bindingImports(
  bindings: ts.NamedImportBindings,
  specifier: string,
  surfaceOf: SurfaceOf,
): SurfaceImport[] {
  if (ts.isNamespaceImport(bindings)) {
    const surface = surfaceOf(specifier);
    return surface ? [{ kind: 'namespace', local: bindings.name.text, surface }] : [];
  }
  const found: SurfaceImport[] = [];
  for (const el of bindings.elements) {
    const name = (el.propertyName ?? el.name).text;
    const surface = surfaceOf(specifier, name);
    if (surface) found.push({ kind: 'symbol', local: el.name.text, name, surface });
  }
  return found;
}

/** The surface an import declaration's specifier names. */
export function importSurface(statement: ts.Statement, surfaceOf: SurfaceOf): string | undefined {
  if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) {
    return undefined;
  }
  return surfaceOf(statement.moduleSpecifier.text);
}

function namespaceMembers(
  source: ts.SourceFile,
  namespaces: ReadonlyMap<string, string>,
): SymbolRef[] {
  const refs: SymbolRef[] = [];
  const visit = (node: ts.Node) => {
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression)) {
      const surface = namespaces.get(node.expression.text);
      if (surface) refs.push({ name: node.name.text, surface });
    }
    if (ts.isQualifiedName(node) && ts.isIdentifier(node.left)) {
      const surface = namespaces.get(node.left.text);
      if (surface) refs.push({ name: node.right.text, surface });
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return refs;
}

/** Symbol references without repeats, in first-seen order. */
export function dedupe(refs: readonly SymbolRef[]): SymbolRef[] {
  const seen = new Set<string>();
  return refs.filter((r) => {
    const key = `${r.surface ?? ''}\0${r.name}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
