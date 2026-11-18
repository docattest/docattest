import ts from '../ts.cjs';

/** A value folded from source, wrapped because `undefined` is a value too. */
export interface Folded {
  value: unknown;
}

/** The value of a name in an expression, or undefined when it is not a constant. */
export type NameResolver = (name: ts.Identifier) => Folded | undefined;

type Folder<T extends ts.Node> = (node: T, resolve: NameResolver) => Folded | undefined;

/**
 * The value of an expression written only of literals (strings, numbers, bigints, booleans,
 * `null`, templates, arrays and objects), operators on them and names `resolve` folds, as
 * JavaScript would compute it; undefined for anything else, such as a call or a function.
 * Nothing of the code runs: the operators apply to values already folded.
 */
export function foldConstant(node: ts.Expression, resolve: NameResolver): Folded | undefined {
  const expression = unwrapped(node);
  const folder = FOLDERS.get(expression.kind) as Folder<ts.Expression> | undefined;
  return folder?.(expression, resolve);
}

// Type-only syntax around a value leaves the value as it is.
function unwrapped(node: ts.Expression): ts.Expression {
  let current = node;
  while (
    ts.isParenthesizedExpression(current) ||
    ts.isAsExpression(current) ||
    ts.isSatisfiesExpression(current) ||
    ts.isTypeAssertionExpression(current) ||
    ts.isNonNullExpression(current)
  ) {
    current = current.expression;
  }
  return current;
}

const literal =
  (value: unknown): Folder<ts.Node> =>
  () => ({ value });

/** Names that are constants in every module, unless one declares them. */
const GLOBAL_CONSTANTS = new Map<string, Folded>([
  ['undefined', { value: undefined }],
  ['NaN', { value: NaN }],
  ['Infinity', { value: Infinity }],
]);

const FOLDERS = new Map<ts.SyntaxKind, Folder<never>>([
  [ts.SyntaxKind.StringLiteral, (node: ts.StringLiteral) => ({ value: node.text })],
  [ts.SyntaxKind.NoSubstitutionTemplateLiteral, (node: ts.StringLiteral) => ({ value: node.text })],
  [ts.SyntaxKind.NumericLiteral, (node: ts.NumericLiteral) => ({ value: numberOf(node.text) })],
  [ts.SyntaxKind.BigIntLiteral, (node: ts.BigIntLiteral) => ({ value: bigintOf(node.text) })],
  [ts.SyntaxKind.TrueKeyword, literal(true)],
  [ts.SyntaxKind.FalseKeyword, literal(false)],
  [ts.SyntaxKind.NullKeyword, literal(null)],
  [
    ts.SyntaxKind.Identifier,
    (node: ts.Identifier, resolve: NameResolver) =>
      resolve(node) ?? GLOBAL_CONSTANTS.get(node.text),
  ],
  [ts.SyntaxKind.TemplateExpression, foldTemplate],
  [ts.SyntaxKind.ArrayLiteralExpression, foldArray],
  [ts.SyntaxKind.ObjectLiteralExpression, foldObject],
  [ts.SyntaxKind.PrefixUnaryExpression, foldPrefix],
  [ts.SyntaxKind.BinaryExpression, foldBinary],
  [ts.SyntaxKind.ConditionalExpression, foldConditional],
  [ts.SyntaxKind.PropertyAccessExpression, foldMember],
  [ts.SyntaxKind.ElementAccessExpression, foldMember],
]);

const numberOf = (text: string) => Number(text.replaceAll('_', ''));
const bigintOf = (text: string) => BigInt(text.replaceAll('_', '').slice(0, -1));

function foldTemplate(node: ts.TemplateExpression, resolve: NameResolver): Folded | undefined {
  let text = node.head.text;
  for (const span of node.templateSpans) {
    const folded = foldConstant(span.expression, resolve);
    if (!folded) return undefined;
    text += String(folded.value) + span.literal.text;
  }
  return { value: text };
}

function foldArray(node: ts.ArrayLiteralExpression, resolve: NameResolver): Folded | undefined {
  const items: unknown[] = [];
  for (const element of node.elements) {
    const spread = ts.isSpreadElement(element);
    const folded = ts.isOmittedExpression(element)
      ? undefined
      : foldConstant(spread ? element.expression : element, resolve);
    if (!folded) return undefined;
    if (!spread) items.push(folded.value);
    else if (Array.isArray(folded.value) || typeof folded.value === 'string') {
      items.push(...(folded.value as Iterable<unknown>));
    } else return undefined;
  }
  return { value: items };
}

function foldObject(node: ts.ObjectLiteralExpression, resolve: NameResolver): Folded | undefined {
  const target: Record<string, unknown> = {};
  for (const property of node.properties) {
    const entries = propertyEntries(property, resolve);
    if (!entries) return undefined;
    // Defined, not assigned: a key named `__proto__` from a spread is an own property too.
    for (const [key, value] of entries) {
      Object.defineProperty(target, key, {
        value,
        enumerable: true,
        writable: true,
        configurable: true,
      });
    }
  }
  return { value: target };
}

function propertyEntries(
  property: ts.ObjectLiteralElementLike,
  resolve: NameResolver,
): [string, unknown][] | undefined {
  if (ts.isShorthandPropertyAssignment(property)) {
    const folded = resolve(property.name);
    return folded && [[property.name.text, folded.value]];
  }
  if (ts.isSpreadAssignment(property)) return spreadEntries(property, resolve);
  return ts.isPropertyAssignment(property) ? assignedEntry(property, resolve) : undefined;
}

// `...value` copies an object's (or an array's, or a string's) own entries; of `null` or
// `undefined`, none.
function spreadEntries(
  property: ts.SpreadAssignment,
  resolve: NameResolver,
): [string, unknown][] | undefined {
  const folded = foldConstant(property.expression, resolve);
  if (!folded) return undefined;
  return folded.value === null || folded.value === undefined ? [] : Object.entries(folded.value);
}

function assignedEntry(
  property: ts.PropertyAssignment,
  resolve: NameResolver,
): [string, unknown][] | undefined {
  const key = propertyKey(property.name, resolve);
  const folded = key === undefined ? undefined : foldConstant(property.initializer, resolve);
  return key === undefined || !folded ? undefined : [[key, folded.value]];
}

// `__proto__: x` sets the prototype rather than a property, so it is no constant's key.
function propertyKey(name: ts.PropertyName, resolve: NameResolver): string | undefined {
  if (ts.isComputedPropertyName(name)) {
    const folded = foldConstant(name.expression, resolve);
    const key = folded?.value;
    return typeof key === 'string' || typeof key === 'number' ? String(key) : undefined;
  }
  if (ts.isIdentifier(name) || ts.isStringLiteral(name)) {
    return name.text === '__proto__' ? undefined : name.text;
  }
  return ts.isNumericLiteral(name) ? String(numberOf(name.text)) : undefined;
}

// The operands are any folded values: JavaScript applies each operator to them as it would
// when the module runs, and an operation that would throw makes no constant.
type Operands = [number, number];

const PREFIX = new Map<ts.SyntaxKind, (operand: unknown) => unknown>([
  [ts.SyntaxKind.MinusToken, (a) => -(a as number)],
  [ts.SyntaxKind.PlusToken, unaryPlus],
  [ts.SyntaxKind.ExclamationToken, (a) => !a],
  [ts.SyntaxKind.TildeToken, (a) => ~(a as number)],
]);

// `+x` is `Number(x)`, except that it refuses a bigint.
function unaryPlus(operand: unknown): number {
  if (typeof operand === 'bigint') throw new TypeError('Cannot convert a BigInt value to a number');
  return Number(operand);
}

const BINARY = new Map<ts.SyntaxKind, (...operands: Operands) => unknown>([
  [ts.SyntaxKind.PlusToken, (a, b) => a + b],
  [ts.SyntaxKind.MinusToken, (a, b) => a - b],
  [ts.SyntaxKind.AsteriskToken, (a, b) => a * b],
  [ts.SyntaxKind.SlashToken, (a, b) => a / b],
  [ts.SyntaxKind.PercentToken, (a, b) => a % b],
  [ts.SyntaxKind.AsteriskAsteriskToken, (a, b) => a ** b],
  [ts.SyntaxKind.LessThanLessThanToken, (a, b) => a << b],
  [ts.SyntaxKind.GreaterThanGreaterThanToken, (a, b) => a >> b],
  [ts.SyntaxKind.GreaterThanGreaterThanGreaterThanToken, (a, b) => a >>> b],
  [ts.SyntaxKind.AmpersandToken, (a, b) => a & b],
  [ts.SyntaxKind.BarToken, (a, b) => a | b],
  [ts.SyntaxKind.CaretToken, (a, b) => a ^ b],
  [ts.SyntaxKind.LessThanToken, (a, b) => a < b],
  [ts.SyntaxKind.GreaterThanToken, (a, b) => a > b],
  [ts.SyntaxKind.LessThanEqualsToken, (a, b) => a <= b],
  [ts.SyntaxKind.GreaterThanEqualsToken, (a, b) => a >= b],
  [ts.SyntaxKind.EqualsEqualsEqualsToken, (a, b) => a === b],
  [ts.SyntaxKind.ExclamationEqualsEqualsToken, (a, b) => a !== b],
  [ts.SyntaxKind.AmpersandAmpersandToken, (a, b) => a && b],
  [ts.SyntaxKind.BarBarToken, (a, b) => a || b],
  [ts.SyntaxKind.QuestionQuestionToken, (a, b) => (isNullish(a) ? b : a)],
]);

function foldPrefix(node: ts.PrefixUnaryExpression, resolve: NameResolver): Folded | undefined {
  const operator = PREFIX.get(node.operator);
  const operand = operator && foldConstant(node.operand, resolve);
  return operand && applied(() => operator(operand.value));
}

function foldBinary(node: ts.BinaryExpression, resolve: NameResolver): Folded | undefined {
  const operator = BINARY.get(node.operatorToken.kind);
  const left = operator && foldConstant(node.left, resolve);
  const right = left && foldConstant(node.right, resolve);
  return right && applied(() => operator(left.value as number, right.value as number));
}

function isNullish(value: unknown): boolean {
  return value === null || value === undefined;
}

// Only a TypeError can come of operators on plain values (`1n + 1`, `+1n`).
function applied(operation: () => unknown): Folded | undefined {
  try {
    return { value: operation() };
  } catch (err) {
    if (err instanceof TypeError) return undefined;
    throw err;
  }
}

function foldConditional(
  node: ts.ConditionalExpression,
  resolve: NameResolver,
): Folded | undefined {
  const condition = foldConstant(node.condition, resolve);
  if (!condition) return undefined;
  return foldConstant(condition.value ? node.whenTrue : node.whenFalse, resolve);
}

// An own property of a folded value; one it inherits (a method) is not a constant.
function foldMember(
  node: ts.PropertyAccessExpression | ts.ElementAccessExpression,
  resolve: NameResolver,
): Folded | undefined {
  const base = foldConstant(node.expression, resolve);
  const key = base && memberKey(node, resolve);
  if (!base || key === undefined) return undefined;
  if (base.value === null || base.value === undefined) {
    return node.questionDotToken ? { value: undefined } : undefined;
  }
  const holder = Object(base.value) as Record<string, unknown>;
  if (Object.hasOwn(holder, key)) return { value: holder[key] };
  return key in holder ? undefined : { value: undefined };
}

function memberKey(
  node: ts.PropertyAccessExpression | ts.ElementAccessExpression,
  resolve: NameResolver,
): string | undefined {
  if (ts.isPropertyAccessExpression(node)) {
    return ts.isIdentifier(node.name) ? node.name.text : undefined;
  }
  const key = foldConstant(node.argumentExpression, resolve)?.value;
  return typeof key === 'string' || typeof key === 'number' ? String(key) : undefined;
}
