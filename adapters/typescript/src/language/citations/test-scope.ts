import ts from '../ts.cjs';
import { foldConstant, type Folded } from '../syntax/constant-value.js';

/**
 * What the names of a test file stand for where a title is written: the values the file's
 * own literals give them. A name is known when it is a `const` written above its use, in
 * the file, whose value the file never changes in place ({@link mutated}), or a loop
 * variable or callback parameter bound to one row of a table the file writes. Any other
 * name (a parameter, an import, a `let`) is not known, and nor is anything computed from it.
 */
export class TestScope {
  constructor(
    /** Names the file changes in place, anywhere: none of them is known. */
    private readonly mutated: ReadonlySet<string>,
    /** The values of loop variables and parameters bound so far; undefined where unknown. */
    private readonly bound: ReadonlyMap<string, Folded | undefined> = new Map(),
  ) {}

  /** The value of an expression where it is written, or undefined when it is not known. */
  fold(expression: ts.Expression): Folded | undefined {
    return foldConstant(expression, (name) => this.resolve(name));
  }

  /** A scope in which the names of a binding (`c`, `{ name, input }`) hold a value. */
  binding(pattern: ts.BindingName, value: Folded | undefined): TestScope {
    const bound = new Map(this.bound);
    bind(pattern, value, bound, this);
    return new TestScope(this.mutated, bound);
  }

  private resolve(name: ts.Identifier): Folded | undefined {
    for (let node: ts.Node = name; !ts.isSourceFile(node); node = node.parent) {
      const scope = node.parent;
      if (binds(scope, name.text)) return this.bound.get(name.text);
      const declaration = declarationIn(scope, name.text);
      if (declaration) return this.declared(declaration, name);
    }
    return undefined;
  }

  private declared(declaration: ts.Node, use: ts.Identifier): Folded | undefined {
    if (!ts.isVariableDeclaration(declaration) || !declaration.initializer) return undefined;
    const isConst = (declaration.parent.flags & ts.NodeFlags.Const) !== 0;
    const isAbove = declaration.end <= use.getStart();
    const usable = isConst && isAbove && !this.mutated.has(use.text);
    return usable ? this.fold(declaration.initializer) : undefined;
  }
}

// Whether a function's parameters, a loop's variable or a catch clause bind the name.
function binds(node: ts.Node, name: string): boolean {
  if (ts.isFunctionLike(node)) return node.parameters.some((p) => declares(p.name, name));
  if (ts.isForOfStatement(node) || ts.isForInStatement(node) || ts.isForStatement(node)) {
    const list = node.initializer;
    return (
      list !== undefined &&
      ts.isVariableDeclarationList(list) &&
      list.declarations.some((d) => declares(d.name, name))
    );
  }
  return ts.isCatchClause(node) && node.variableDeclaration !== undefined
    ? declares(node.variableDeclaration.name, name)
    : false;
}

// The declaration of a name among the statements of a block, a module or a source file.
function declarationIn(node: ts.Node, name: string): ts.Node | undefined {
  if (!ts.isBlock(node) && !ts.isSourceFile(node) && !ts.isModuleBlock(node)) return undefined;
  for (const statement of node.statements) {
    if (ts.isVariableStatement(statement)) {
      const found = statement.declarationList.declarations.find((d) => declares(d.name, name));
      if (found) return found;
    } else if (declaredBy(statement).includes(name)) return statement;
  }
  return undefined;
}

// Names a statement that is not a variable statement declares: functions, classes, imports.
function declaredBy(statement: ts.Statement): string[] {
  if (ts.isImportDeclaration(statement)) {
    const clause = statement.importClause;
    const names = [clause?.name?.text];
    const bindings = clause?.namedBindings;
    if (bindings && ts.isNamespaceImport(bindings)) names.push(bindings.name.text);
    if (bindings && ts.isNamedImports(bindings)) {
      names.push(...bindings.elements.map((e) => e.name.text));
    }
    return names.filter((n) => n !== undefined);
  }
  const named = (statement as ts.DeclarationStatement).name;
  return named && ts.isIdentifier(named) ? [named.text] : [];
}

function declares(pattern: ts.BindingName, name: string): boolean {
  if (ts.isIdentifier(pattern)) return pattern.text === name;
  return pattern.elements.some((e) => !ts.isOmittedExpression(e) && declares(e.name, name));
}

// Destructure a value into the names of a pattern, as `const { a, b: [c] } = value` would.
function bind(
  pattern: ts.BindingName,
  value: Folded | undefined,
  into: Map<string, Folded | undefined>,
  scope: TestScope,
): void {
  if (ts.isIdentifier(pattern)) {
    into.set(pattern.text, value);
    return;
  }
  pattern.elements.forEach((element, index) => {
    if (ts.isOmittedExpression(element)) return;
    const part =
      value && !element.dotDotDotToken ? elementOf(element, index, value.value) : undefined;
    const defaulted =
      part && part.value === undefined && element.initializer
        ? scope.fold(element.initializer)
        : part;
    bind(element.name, defaulted, into, scope);
  });
}

function elementOf(element: ts.BindingElement, index: number, value: unknown): Folded | undefined {
  if (value === null || value === undefined) return undefined;
  const key = ts.isArrayBindingPattern(element.parent) ? String(index) : propertyName(element);
  const holder = Object(value) as Record<string, unknown>;
  if (key === undefined) return undefined;
  if (Object.hasOwn(holder, key)) return { value: holder[key] };
  // A key the value inherits (a method) is no row's value; one it lacks is undefined.
  return key in holder ? undefined : { value: undefined };
}

function propertyName(element: ts.BindingElement): string | undefined {
  const name = element.propertyName ?? element.name;
  return ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)
    ? name.text
    : undefined;
}
