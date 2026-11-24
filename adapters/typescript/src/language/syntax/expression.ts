import ts from '../ts.cjs';
import { parseSource, unique } from './parse.js';

const wrap = (expression: string) => `(${expression}\n);`;

/** Why a fact expression is not exactly one expression, or undefined when it is. */
export function expressionProblem(expression: string): string | undefined {
  const out = ts.transpileModule(wrap(expression), {
    reportDiagnostics: true,
    compilerOptions: { target: ts.ScriptTarget.ES2022 },
  });
  const [first] = out.diagnostics ?? [];
  if (first) {
    const reason = ts.flattenDiagnosticMessageText(first.messageText, ' ');
    return `cannot parse fact expression \`${expression}\`: ${reason}`;
  }
  const source = parseSource('fact.ts', wrap(expression));
  const [statement] = source.statements;
  if (source.statements.length !== 1 || !statement || !ts.isExpressionStatement(statement)) {
    return `fact expression \`${expression}\` must be a single expression`;
  }
  return undefined;
}

/** Free identifiers of an expression: not property names, not its own parameters. */
export function expressionIdentifiers(expression: string): string[] {
  return referencedNames(parseSource('fact.ts', wrap(expression)), () => true);
}

/** Exported type names passed as string literals to `typeKeys('...')` or `typeValues('...')`. */
export function typeFactTargets(expression: string): string[] {
  const names: string[] = [];
  const visit = (node: ts.Node) => {
    if (
      ts.isCallExpression(node) &&
      ts.isIdentifier(node.expression) &&
      (node.expression.text === 'typeKeys' || node.expression.text === 'typeValues')
    ) {
      const [first] = node.arguments;
      if (first && ts.isStringLiteralLike(first)) names.push(first.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(parseSource('fact.ts', wrap(expression)));
  return unique(names);
}

/**
 * Names the code at `root` refers to, once each: not property names, bindings or the
 * parameters of a function in it. A node, `root` included, is looked into only when
 * `enters` says so.
 */
export function referencedNames(root: ts.Node, enters: (node: ts.Node) => boolean): string[] {
  const names: string[] = [];
  const visit = (node: ts.Node) => {
    if (!enters(node)) return;
    if (ts.isIdentifier(node) && !isBindingOrMember(node) && !isParameter(node)) {
      names.push(node.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(root);
  return unique(names);
}

function isBindingOrMember(node: ts.Identifier): boolean {
  const p = node.parent;
  return (
    (ts.isPropertyAccessExpression(p) && p.name === node) ||
    (ts.isPropertyAssignment(p) && p.name === node) ||
    (ts.isParameter(p) && p.name === node) ||
    (ts.isBindingElement(p) && p.name === node) ||
    (ts.isVariableDeclaration(p) && p.name === node)
  );
}

// A use of a parameter of an enclosing function in the expression, as `r` in `xs.map((r) => r.a)`.
function isParameter(node: ts.Identifier): boolean {
  const owner = ts.findAncestor(
    node.parent,
    (p) => ts.isFunctionLike(p) && p.parameters.some((param) => binds(param.name, node.text)),
  );
  return owner !== undefined;
}

function binds(name: ts.BindingName, text: string): boolean {
  if (ts.isIdentifier(name)) return name.text === text;
  return name.elements.some((el) => !ts.isOmittedExpression(el) && binds(el.name, text));
}
