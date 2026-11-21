import { Buffer } from 'node:buffer';
import type { SymbolMutants, SymbolRef } from '../../api/adapter.js';
import type { FileSource } from '../../files/source.js';
import { surfacePrograms } from '../impact/program.js';
import type { SurfaceSpec } from '../surfaces/specs.js';
import { unwrapped } from '../syntax/constant-value.js';
import { literalMutants, type KeepsLength, type LiteralMutant } from '../syntax/literal-mutants.js';
import ts from '../ts.cjs';
import { DeclaredValues, type DeclaredValue } from './declared-value.js';

/**
 * For each symbol, in order, at most `limit` edits that each change one literal of the value
 * it is declared with ({@link literalMutants}), the first in source order; or why it has none.
 * When a scalar constant holds no literal of its own, a type-guided expression edit is offered.
 * The declarations are read from the code behind the surfaces as `affected` reads it: a
 * surface that points at built output is traced back to its source, the code a change edits.
 */
export function symbolMutants(
  files: FileSource,
  specs: readonly SurfaceSpec[],
  symbols: readonly SymbolRef[],
  limit: number,
): SymbolMutants[] {
  const { program, entries } = surfacePrograms(files, specs);
  const declared = new DeclaredValues(program, entries, files.root);
  const checker = program.getTypeChecker();
  const keepsLength: KeepsLength = (array) => {
    const type = checker.getContextualType(array);
    return type !== undefined && checker.isTupleType(type);
  };
  return symbols.map((symbol) => mutantsOf(declared.of(symbol), limit, keepsLength, checker));
}

function mutantsOf(
  declared: DeclaredValue,
  limit: number,
  keepsLength: KeepsLength,
  checker: ts.TypeChecker,
): SymbolMutants {
  if (declared.kind === 'none') return { edits: [], why: declared.why };
  const { value, source, path } = declared;
  const literals = literalMutants(value, source, keepsLength);
  const fallback = literals.length === 0 ? expressionMutant(value, source, checker) : undefined;
  const mutants = (fallback ? [fallback] : literals).slice(0, limit);
  const [first, ...rest] = mutants.map(({ start, end, replacement, description }) => ({
    path,
    start: utf8Offset(source.text, start),
    end: utf8Offset(source.text, end),
    replacement,
    description,
  }));
  return first ? { edits: [first, ...rest] } : { edits: [], why: 'noLiteral' };
}

/** Fallback edit for a scalar constant expression that contains no literal of its own. */
function expressionMutant(
  value: ts.Expression,
  source: ts.SourceFile,
  checker: ts.TypeChecker,
): LiteralMutant | undefined {
  const inner = unwrapped(value);
  if (!isPureNonLiteral(inner)) return undefined;
  const start = value.getStart(source);
  const written = source.text.slice(start, value.end);
  const wrapped = isAtom(value) ? written : `(${written})`;
  const replacement = scalarReplacement(checker.getTypeAtLocation(value), wrapped);
  if (replacement === undefined) return undefined;
  return { start, end: value.end, replacement, description: `${written} → ${replacement}` };
}

function scalarReplacement(type: ts.Type, wrapped: string): string | undefined {
  if (type.flags & ts.TypeFlags.NumberLike) {
    return type.isNumberLiteral() && type.value === 0 ? `${wrapped} + 1` : `2 * ${wrapped}`;
  }
  if (type.flags & ts.TypeFlags.BigIntLike) {
    return `${wrapped} + 1n`;
  }
  if (type.flags & ts.TypeFlags.StringLike) return `${wrapped} + 'x'`;
  if (type.flags & ts.TypeFlags.BooleanLike) return `!${wrapped}`;
  return undefined;
}

function isAtom(node: ts.Expression): boolean {
  return (
    ts.isIdentifier(node) ||
    ts.isPropertyAccessExpression(node) ||
    ts.isParenthesizedExpression(node)
  );
}

/** True when `node` is composed only of names, property accesses and operators (no calls). */
function isPureNonLiteral(node: ts.Node): boolean {
  if (ts.isIdentifier(node)) return node.text !== 'undefined';
  if (ts.isPropertyAccessExpression(node)) return isPureNonLiteral(node.expression);
  if (ts.isParenthesizedExpression(node)) return isPureNonLiteral(node.expression);
  if (ts.isPrefixUnaryExpression(node)) return isPureNonLiteral(node.operand);
  if (ts.isBinaryExpression(node)) {
    return isPureNonLiteral(node.left) && isPureNonLiteral(node.right);
  }
  return false;
}

// The protocol counts bytes of UTF-8 text; the compiler counts UTF-16 code units.
function utf8Offset(text: string, offset: number): number {
  return Buffer.byteLength(text.slice(0, offset), 'utf8');
}
