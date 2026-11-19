import ts from '../ts.cjs';
import type { DeclaredTest } from '../../api/adapter.js';
import { mutatedNames } from '../syntax/mutations.js';
import { parseSource } from '../syntax/parse.js';
import { runnerOf, tableRows, type TestRunner } from './each-titles.js';
import { declarationOf, type TestDeclaration } from './test-calls.js';
import { TestScope } from './test-scope.js';

/** A test or describe call in a test file, with the call itself for symbol lookup. */
export interface FoundTest extends DeclaredTest {
  call: ts.CallExpression;
  /**
   * The full paths (describe titles, then the title) a runner reports what it declares by:
   * one, or one for each row of a table that it declares by its title as written. Undefined
   * when a title on the way cannot be told.
   */
  reported: string[][] | undefined;
}

/**
 * Every test and describe block in a file whose title the file's own literals write, and the
 * runner it imports its test functions from.
 */
interface FoundTests {
  tests: FoundTest[];
  suites: FoundTest[];
  runner: TestRunner | undefined;
}

/** A title a call declares, the titles the runner reports it by, and its callback's scope. */
interface Titled {
  title: string | undefined;
  reported: string[] | undefined;
  scope: TestScope;
}

/** Where a call is: the describe titles around it, and the paths the runner reports them by. */
interface Within {
  describes: (string | null)[];
  reported: string[][] | undefined;
}

/**
 * Calls to `it` and `test` (and their `.only`, `.skip`, `.each`, `.concurrent` forms) and
 * the describe blocks around them, each with a title the file's own literals write: a
 * string, or an expression of constants ({@link TestScope}). A table the file writes gives
 * one title for each row, as the runner prints it: `.each(rows)`, a `.each` template literal
 * and `.for(rows)` by their title's placeholders, and a `for … of` loop or a `.forEach` over
 * one by the titles its body computes. Titles in comments or strings, and computed from
 * anything else, are not declarations.
 */
export function findTests(fileName: string, code: string): FoundTests {
  const source = parseSource(fileName, code);
  const runner = runnerOf(source);
  const finder = new TestFinder(source, runner);
  finder.visit(source, { describes: [], reported: [[]] }, new TestScope(mutatedNames(source)));
  return { tests: distinct(finder.found.tests), suites: distinct(finder.found.suites), runner };
}

class TestFinder {
  readonly found: Omit<FoundTests, 'runner'> = { tests: [], suites: [] };

  constructor(
    private readonly source: ts.SourceFile,
    private readonly runner: TestRunner | undefined,
  ) {}

  visit(node: ts.Node, within: Within, scope: TestScope): void {
    if (ts.isForOfStatement(node)) {
      this.loop(node, within, scope);
      return;
    }
    const declaration = ts.isCallExpression(node) ? declarationOf(node) : undefined;
    if (declaration && ts.isCallExpression(node)) {
      this.declared(node, declaration, within, scope);
      return;
    }
    if (ts.isCallExpression(node) && isForEach(node)) {
      this.forEach(node, within, scope);
      return;
    }
    ts.forEachChild(node, (child) => {
      this.visit(child, within, scope);
    });
  }

  private declared(
    call: ts.CallExpression,
    declaration: TestDeclaration,
    within: Within,
    scope: TestScope,
  ): void {
    const line = this.source.getLineAndCharacterOfPosition(call.getStart(this.source)).line + 1;
    const callback = call.arguments.find(
      (a) => ts.isArrowFunction(a) || ts.isFunctionExpression(a),
    );
    const titles = this.titles(call, declaration, callback, scope);
    for (const { title, reported: own, scope: inner } of titles) {
      const reported = own && within.reported?.flatMap((path) => own.map((t) => [...path, t]));
      const entry = { title: title ?? '', describes: within.describes, line, call, reported };
      if (title !== undefined) {
        this.found[declaration.role === 'suite' ? 'suites' : 'tests'].push(entry);
      }
      const inside =
        declaration.role === 'suite'
          ? { describes: [...within.describes, title ?? null], reported }
          : within;
      for (const arg of call.arguments) this.visit(arg, inside, inner);
    }
  }

  // The title as written, with the callback's parameters unknown; then one for each row of
  // a table the file writes, with the parameters bound to that row. The runner reports the
  // title as written by its rows' titles.
  private titles(
    call: ts.CallExpression,
    declaration: TestDeclaration,
    callback: ts.Expression | undefined,
    scope: TestScope,
  ): Titled[] {
    const first = call.arguments[0];
    const folded = first && scope.fold(first);
    const title = typeof folded?.value === 'string' ? folded.value : undefined;
    const unbound = withParameters(callback, undefined, scope);
    if (!declaration.table || title === undefined) {
      return [{ title, reported: title === undefined ? undefined : [title], scope: unbound }];
    }
    const rows = tableRows(declaration.table, title, this.runner, scope);
    const each = (rows ?? []).map((row) => ({
      title: row.title,
      reported: row.title === undefined ? undefined : [row.title],
      scope: withParameters(callback, row.args, scope),
    }));
    const reported =
      rows && each.every((row) => row.reported)
        ? each.flatMap((row) => row.reported ?? [])
        : undefined;
    return [{ title, reported, scope: unbound }, ...each];
  }

  private loop(loop: ts.ForOfStatement, within: Within, scope: TestScope): void {
    const list = loop.initializer;
    const variable = ts.isVariableDeclarationList(list) ? list.declarations[0] : undefined;
    const items = loop.awaitModifier ? undefined : iterated(loop.expression, scope);
    if (!variable) this.visit(loop.statement, within, scope);
    else if (!items) this.visit(loop.statement, within, scope.binding(variable.name, undefined));
    else {
      for (const item of items) {
        this.visit(loop.statement, within, scope.binding(variable.name, { value: item }));
      }
    }
  }

  private forEach(call: ts.CallExpression, within: Within, scope: TestScope): void {
    const callback = call.arguments[0];
    const receiver = (call.expression as ts.PropertyAccessExpression).expression;
    const items = iterated(receiver, scope);
    if (!callback || (!ts.isArrowFunction(callback) && !ts.isFunctionExpression(callback))) {
      ts.forEachChild(call, (child) => {
        this.visit(child, within, scope);
      });
      return;
    }
    if (!items) this.visit(callback.body, within, withParameters(callback, undefined, scope));
    items?.forEach((item, index) => {
      const bound = withParameters(callback, [item, index, items], scope);
      this.visit(callback.body, within, bound);
    });
  }
}

function isForEach(call: ts.CallExpression): boolean {
  return ts.isPropertyAccessExpression(call.expression) && call.expression.name.text === 'forEach';
}

// What `Object.entries(x)`, `Object.keys(x)` and `Object.values(x)` make of a folded object.
const OBJECT_FUNCTIONS = new Map<string, (value: object) => unknown[]>([
  ['entries', Object.entries],
  ['keys', Object.keys],
  ['values', Object.values],
]);

/** What a loop goes over, when the file writes it: an array, or `Object.entries` of one. */
function iterated(expression: ts.Expression, scope: TestScope): unknown[] | undefined {
  const call = objectFunction(expression);
  const folded = scope.fold(call ? call.argument : expression)?.value;
  if (!call) return Array.isArray(folded) ? folded : undefined;
  return typeof folded === 'object' && folded !== null ? call.apply(folded) : undefined;
}

function objectFunction(
  expression: ts.Expression,
): { apply: (value: object) => unknown[]; argument: ts.Expression } | undefined {
  if (!ts.isCallExpression(expression) || !ts.isPropertyAccessExpression(expression.expression)) {
    return undefined;
  }
  const { expression: object, name } = expression.expression;
  const [argument] = expression.arguments;
  const apply = OBJECT_FUNCTIONS.get(name.text);
  const isObject = ts.isIdentifier(object) && object.text === 'Object';
  return isObject && apply && argument ? { apply, argument } : undefined;
}

// A callback's scope: each parameter bound to its argument, or unknown without arguments.
function withParameters(
  callback: ts.Expression | undefined,
  args: readonly unknown[] | undefined,
  scope: TestScope,
): TestScope {
  if (!callback || (!ts.isArrowFunction(callback) && !ts.isFunctionExpression(callback))) {
    return scope;
  }
  return callback.parameters.reduce(
    (inner, parameter, k) =>
      inner.binding(parameter.name, args && k < args.length ? { value: args[k] } : undefined),
    scope,
  );
}

// A loop whose rows give one title twice declares that test once, at its first row.
function distinct(found: readonly FoundTest[]): FoundTest[] {
  const seen = new Set<string>();
  return found.filter((test) => {
    const key = JSON.stringify([test.describes, test.title]);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
