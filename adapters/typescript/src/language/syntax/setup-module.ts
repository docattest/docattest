import ts from '../ts.cjs';
import type { SymbolRef } from '../../api/adapter.js';
import { dedupe, importSurface, type SurfaceOf } from './imports.js';
import { parseSource } from './parse.js';

/** What the setup module declares, as far as syntax can tell. */
interface SetupModule {
  /** Local name → the surface symbol it was imported as. */
  imported: Map<string, SymbolRef>;
  /** Top-level functions and variable initialisers, by name. */
  locals: Map<string, ts.Node>;
  source: ts.SourceFile;
}

/**
 * Surface symbols a setup-module helper reaches: identifiers imported from a surface in the
 * helper's own code and in the top-level helpers it calls. `group` is the exported object
 * that holds it (`cases` or `facts`), `key` its property name.
 */
export function helperSymbols(
  fileName: string,
  code: string,
  group: string,
  key: string,
  surfaceOf: SurfaceOf,
): SymbolRef[] {
  const setup = readSetup(fileName, code, surfaceOf);
  const helper = propertyOf(setup, group, key);
  if (!helper) return [];
  const refs: SymbolRef[] = [];
  const seen = new Set<ts.Node>([setup.locals.get(group)].filter((n) => n !== undefined));
  const visit = (node: ts.Node) => {
    if (ts.isIdentifier(node)) {
      const ref = setup.imported.get(node.text);
      if (ref) refs.push(ref);
      const local = setup.locals.get(node.text);
      if (local && !seen.has(local)) {
        seen.add(local);
        visit(local);
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(helper);
  return dedupe(refs);
}

/** Property names of an exported object literal, such as the keys of `facts`. */
export function objectKeys(fileName: string, code: string, group: string): string[] {
  const setup = readSetup(fileName, code, () => undefined);
  const object = objectLiteral(setup.locals.get(group));
  return (object?.properties ?? []).flatMap((p) => {
    const name = propertyName(p);
    return name === undefined ? [] : [name];
  });
}

function readSetup(fileName: string, code: string, surfaceOf: SurfaceOf): SetupModule {
  const source = parseSource(fileName, code);
  const setup: SetupModule = { imported: new Map(), locals: new Map(), source };
  for (const statement of source.statements) {
    const surface = importSurface(statement, surfaceOf);
    if (surface && ts.isImportDeclaration(statement)) addImports(setup, statement, surface);
    if (ts.isFunctionDeclaration(statement) && statement.name) {
      setup.locals.set(statement.name.text, statement);
    }
    if (!ts.isVariableStatement(statement)) continue;
    for (const d of statement.declarationList.declarations) {
      if (ts.isIdentifier(d.name) && d.initializer) setup.locals.set(d.name.text, d.initializer);
    }
  }
  return setup;
}

function addImports(setup: SetupModule, statement: ts.ImportDeclaration, surface: string): void {
  const clause = statement.importClause;
  if (clause?.name) setup.imported.set(clause.name.text, { name: 'default', surface });
  const bindings = clause?.namedBindings;
  if (!bindings || !ts.isNamedImports(bindings)) return;
  for (const el of bindings.elements) {
    setup.imported.set(el.name.text, { name: (el.propertyName ?? el.name).text, surface });
  }
}

function propertyOf(setup: SetupModule, group: string, key: string): ts.Node | undefined {
  return objectLiteral(setup.locals.get(group))?.properties.find((p) => propertyName(p) === key);
}

function objectLiteral(node: ts.Node | undefined): ts.ObjectLiteralExpression | undefined {
  let current = node;
  while (
    current &&
    (ts.isAsExpression(current) ||
      ts.isSatisfiesExpression(current) ||
      ts.isParenthesizedExpression(current))
  ) {
    current = current.expression;
  }
  return current && ts.isObjectLiteralExpression(current) ? current : undefined;
}

function propertyName(property: ts.ObjectLiteralElementLike): string | undefined {
  const name = property.name;
  return name && (ts.isIdentifier(name) || ts.isStringLiteral(name)) ? name.text : undefined;
}
