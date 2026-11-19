import ts from '../ts.cjs';

/** One change to one literal: the source text from `start` to `end` (UTF-16 offsets) replaced. */
export interface LiteralMutant {
  start: number;
  end: number;
  replacement: string;
  /** What changes, as the source writes it: `8080 → 8081`, `'LOG_LEVEL' removed`. */
  description: string;
}

/** Whether an array literal must keep its length to compile: its type is a tuple. */
export type KeepsLength = (array: ts.ArrayLiteralExpression) => boolean;

/**
 * The changes to the literals of a value, each meant to be applied alone, in source order and
 * at any depth: a number (a negative one whole) plus one, a string or a template without
 * substitutions with an `x` before its closing quote, a boolean flipped, and an array's last
 * item removed, offered before the changes to its items. A string key of an object literal is
 * a string. A function or class the value defines runs only when called, and a type is no
 * value, so nothing inside them is changed. `null` and `undefined` are left as they are.
 */
export function literalMutants(
  value: ts.Expression,
  source: ts.SourceFile,
  keepsLength: KeepsLength,
): LiteralMutant[] {
  const mutants: LiteralMutant[] = [];
  const visit = (node: ts.Node): void => {
    if (ts.isFunctionLike(node) || ts.isClassLike(node) || ts.isTypeNode(node)) return;
    const changed = leafMutants(node, source);
    if (changed) {
      mutants.push(...changed);
      return;
    }
    const droppable = ts.isArrayLiteralExpression(node) && !keepsLength(node);
    const drop = droppable ? lastItemDropped(node, source) : undefined;
    if (drop) mutants.push(drop);
    ts.forEachChild(node, visit);
  };
  visit(value);
  return mutants;
}

/** The changes to a literal that holds no other; undefined for any other node. */
function leafMutants(node: ts.Node, source: ts.SourceFile): LiteralMutant[] | undefined {
  if (isNumberLiteral(node) || isNegatedNumber(node)) return numberMutants(node, source);
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    const quoted = textOf(node, source);
    return [replaced(node, source, `${quoted.slice(0, -1)}x${quoted.slice(-1)}`)];
  }
  if (node.kind === ts.SyntaxKind.TrueKeyword) return [replaced(node, source, 'false')];
  if (node.kind === ts.SyntaxKind.FalseKeyword) return [replaced(node, source, 'true')];
  return undefined;
}

type NumberLiteral = ts.NumericLiteral | ts.BigIntLiteral;
type NegatedNumber = ts.PrefixUnaryExpression & { operand: NumberLiteral };

function isNumberLiteral(node: ts.Node): node is NumberLiteral {
  return ts.isNumericLiteral(node) || ts.isBigIntLiteral(node);
}

function isNegatedNumber(node: ts.Node): node is NegatedNumber {
  return (
    ts.isPrefixUnaryExpression(node) &&
    node.operator === ts.SyntaxKind.MinusToken &&
    isNumberLiteral(node.operand)
  );
}

// A number far enough from zero has no next number, and that is no change.
function numberMutants(
  node: NumberLiteral | NegatedNumber,
  source: ts.SourceFile,
): LiteralMutant[] {
  const literal = ts.isPrefixUnaryExpression(node) ? node.operand : node;
  const sign = literal === node ? 1 : -1;
  if (ts.isBigIntLiteral(literal)) {
    const value = BigInt(literal.text.slice(0, -1)) * BigInt(sign);
    return [replaced(node, source, `${value + 1n}n`)];
  }
  const value = Number(literal.text) * sign;
  return value + 1 === value ? [] : [replaced(node, source, String(value + 1))];
}

// A single item leaves the brackets empty; otherwise the item goes with the comma before it,
// so a trailing comma stays where it was.
function lastItemDropped(
  array: ts.ArrayLiteralExpression,
  source: ts.SourceFile,
): LiteralMutant | undefined {
  const last = array.elements.at(-1);
  if (!last || ts.isOmittedExpression(last)) return undefined;
  const previous = array.elements.at(-2);
  const start = previous ? previous.end : array.getStart(source) + 1;
  const end = previous ? last.end : array.end - 1;
  const item = textOf(last, source).replace(/\s+/g, ' ');
  return { start, end, replacement: '', description: `${item} removed` };
}

function replaced(node: ts.Node, source: ts.SourceFile, replacement: string): LiteralMutant {
  const start = node.getStart(source);
  const description = `${textOf(node, source)} → ${replacement}`;
  return { start, end: node.end, replacement, description };
}

function textOf(node: ts.Node, source: ts.SourceFile): string {
  return source.text.slice(node.getStart(source), node.end);
}
