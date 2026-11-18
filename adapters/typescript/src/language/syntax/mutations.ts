import ts from '../ts.cjs';

// Methods that change the array they are called on.
const MUTATING_METHODS = new Set([
  'push',
  'pop',
  'shift',
  'unshift',
  'splice',
  'sort',
  'reverse',
  'fill',
  'copyWithin',
]);
// `Object` functions that change their first argument.
const MUTATING_FUNCTIONS = new Set(['assign', 'defineProperty', 'defineProperties']);

/**
 * The names whose value a file changes in place, by name: an assignment to a member
 * (`LIMITS.max = 5`, `list[0]++`), a `delete`, a mutating array method (`LIST.push(x)`) or
 * `Object.assign(CONFIG, …)`. A const named here holds no constant: its value is the one
 * the module leaves once it ran. Scopes are not told apart, so a local of the same name
 * that is changed counts too.
 */
export function mutatedNames(source: ts.SourceFile): Set<string> {
  const names = new Set<string>();
  const visit = (node: ts.Node): void => {
    const target = mutationTarget(node);
    const root = target && rootName(target);
    if (root !== undefined) names.add(root);
    ts.forEachChild(node, visit);
  };
  visit(source);
  return names;
}

// The expression a node changes in place, when it is a member of something.
function mutationTarget(node: ts.Node): ts.Expression | undefined {
  if (ts.isBinaryExpression(node) && isAssignment(node.operatorToken.kind)) {
    return member(node.left);
  }
  if (ts.isPrefixUnaryExpression(node) || ts.isPostfixUnaryExpression(node)) {
    const counts = [ts.SyntaxKind.PlusPlusToken, ts.SyntaxKind.MinusMinusToken];
    return counts.includes(node.operator) ? member(node.operand) : undefined;
  }
  if (ts.isDeleteExpression(node)) return node.expression;
  return ts.isCallExpression(node) ? calledOn(node) : undefined;
}

function isAssignment(kind: ts.SyntaxKind): boolean {
  return kind >= ts.SyntaxKind.FirstAssignment && kind <= ts.SyntaxKind.LastAssignment;
}

function member(node: ts.Expression): ts.Expression | undefined {
  return ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)
    ? node
    : undefined;
}

// `list.push(x)` changes `list`; `Object.assign(target, …)` changes `target`.
function calledOn(call: ts.CallExpression): ts.Expression | undefined {
  const callee = call.expression;
  if (!ts.isPropertyAccessExpression(callee)) return undefined;
  const isObject = ts.isIdentifier(callee.expression) && callee.expression.text === 'Object';
  if (isObject && MUTATING_FUNCTIONS.has(callee.name.text)) return call.arguments[0];
  return MUTATING_METHODS.has(callee.name.text) ? callee.expression : undefined;
}

// The name at the root of a member chain: `CONFIG` in `CONFIG.limits[0]`.
function rootName(node: ts.Expression): string | undefined {
  let current = node;
  while (ts.isPropertyAccessExpression(current) || ts.isElementAccessExpression(current)) {
    current = current.expression;
  }
  return ts.isIdentifier(current) ? current.text : undefined;
}
