import ts from '../ts.cjs';
import { exportedSymbols } from './exports.js';

/** Type-level facts extracted from one surface's exported TypeScript declarations. */
export interface SurfaceTypeFacts {
  /** Property names of each exported interface, object type alias, or class. */
  keys: ReadonlyMap<string, readonly string[]>;
  /** Literal members of each exported string/number literal union type. */
  values: ReadonlyMap<string, readonly string[]>;
  /** Exported names that exist only at the type level (no runtime value binding). */
  typeOnly: ReadonlySet<string>;
}

export const EMPTY_TYPE_FACTS: SurfaceTypeFacts = {
  keys: new Map(),
  values: new Map(),
  typeOnly: new Set(),
};

/** Inspect exported types of `file` via the TypeScript type checker. */
export function surfaceTypeFacts(program: ts.Program, file: string): SurfaceTypeFacts {
  const symbols = exportedSymbols(program, file);
  if (!symbols || symbols.length === 0) return EMPTY_TYPE_FACTS;
  const checker = program.getTypeChecker();
  const keys = new Map<string, readonly string[]>();
  const values = new Map<string, readonly string[]>();
  const typeOnly = new Set<string>();
  for (const symbol of symbols) {
    if (symbol.name === 'default') continue;
    const target = resolveAlias(checker, symbol);
    if (!(target.flags & ts.SymbolFlags.Value)) typeOnly.add(symbol.name);
    const type = declaredType(checker, target);
    if (!type) continue;
    const literals = literalUnionValues(type);
    if (literals) {
      values.set(symbol.name, literals);
      continue;
    }
    const props = objectPropertyKeys(checker, type);
    if (props) keys.set(symbol.name, props);
  }
  return { keys, values, typeOnly };
}

function resolveAlias(checker: ts.TypeChecker, symbol: ts.Symbol): ts.Symbol {
  return symbol.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
}

function declaredType(checker: ts.TypeChecker, target: ts.Symbol): ts.Type | undefined {
  if (!(target.flags & ts.SymbolFlags.Type)) return undefined;
  return checker.getDeclaredTypeOfSymbol(target);
}

/** Literal values of a string/number literal union (excluding `boolean`), or undefined. */
function literalUnionValues(type: ts.Type): readonly string[] | undefined {
  if (!type.isUnion() || type.flags & ts.TypeFlags.Boolean) return undefined;
  const seen = new Set<string>();
  for (const member of type.types) {
    if (member.isStringLiteral()) seen.add(member.value);
    else if (member.isNumberLiteral()) seen.add(String(member.value));
    else return undefined;
  }
  return seen.size >= 2 ? [...seen] : undefined;
}

/** Own named properties of an interface, object type literal, or object intersection. */
function objectPropertyKeys(checker: ts.TypeChecker, type: ts.Type): readonly string[] | undefined {
  if (!isPlainObjectType(checker, type)) return undefined;
  const props = checker
    .getPropertiesOfType(type)
    .map((p) => p.name)
    .filter((name) => !name.startsWith('__@'));
  return props.length > 0 ? props : undefined;
}

function isPlainObjectType(checker: ts.TypeChecker, type: ts.Type): boolean {
  if (type.isIntersection()) {
    return type.types.every((part) => isPlainObjectType(checker, part));
  }
  if (!(type.flags & ts.TypeFlags.Object)) return false;
  if (checker.isArrayType(type) || checker.isTupleType(type)) return false;
  // Pure function/constructor types carry no data properties.
  return type.getCallSignatures().length === 0 && type.getConstructSignatures().length === 0;
}
