import ts from '../ts.cjs';
import type { DeclaredTest } from '../../api/adapter.js';
import { parseSource } from '../syntax/parse.js';

/** A test or describe call in a test file, with the call itself for symbol lookup. */
interface FoundTest extends DeclaredTest {
  call: ts.CallExpression;
}

/** Every test and describe block in a file whose title is a literal. */
interface FoundTests {
  tests: FoundTest[];
  suites: FoundTest[];
}

// Vitest, Jest, Mocha, node:test and Playwright spellings. `todo` is not a test: it has no body.
const TESTS = new Set(['it', 'test', 'fit', 'xit', 'xtest', 'specify']);
const SUITES = new Set(['describe', 'suite', 'context', 'fdescribe', 'xdescribe']);
const MODIFIERS = new Set([
  'only',
  'skip',
  'concurrent',
  'sequential',
  'fails',
  'failing',
  'shuffle',
  'serial',
  'parallel',
  'fixme',
]);
/** Modifiers that take arguments and return the function that declares the test. */
const FACTORIES = new Set(['each', 'for', 'skipIf', 'runIf']);

/**
 * Calls to `it` and `test` (and their `.only`, `.skip`, `.each`, `.concurrent` forms) with
 * a string-literal title, each with the titles of the describe blocks around it. Titles in
 * comments, strings or computed expressions are not declarations.
 */
export function findTests(fileName: string, code: string): FoundTests {
  const source = parseSource(fileName, code);
  const found: FoundTests = { tests: [], suites: [] };
  const lineOf = (node: ts.Node) =>
    source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
  const visit = (node: ts.Node, describes: (string | null)[]) => {
    const role = ts.isCallExpression(node) ? roleOf(node.expression) : undefined;
    if (role && ts.isCallExpression(node)) {
      const title = literalTitle(node.arguments[0]);
      const entry = { title: title ?? '', describes, line: lineOf(node), call: node };
      if (title !== undefined) found[role === 'suite' ? 'suites' : 'tests'].push(entry);
      if (role === 'suite') {
        const inner = [...describes, title ?? null];
        for (const arg of node.arguments) visit(arg, inner);
        return;
      }
    }
    ts.forEachChild(node, (child) => {
      visit(child, describes);
    });
  };
  visit(source, []);
  return found;
}

/** Whether a callee declares a test or a suite: `it`, `describe.only`, `it.each(rows)`... */
function roleOf(callee: ts.Expression): 'test' | 'suite' | undefined {
  if (ts.isPropertyAccessExpression(callee) && FACTORIES.has(callee.name.text)) return undefined;
  const declaring = declaringName(callee);
  if (!declaring) return undefined;
  if (declaring.viaDescribe) return declaring.name === 'test' ? 'suite' : undefined;
  if (TESTS.has(declaring.name)) return 'test';
  return SUITES.has(declaring.name) ? 'suite' : undefined;
}

/**
 * The name a callee declares through, past modifiers (`.only`) and factories (`.each(rows)`),
 * and whether a `.describe` was on the way (`test.describe`); undefined when anything else is.
 */
function declaringName(callee: ts.Expression): { name: string; viaDescribe: boolean } | undefined {
  let current: ts.Expression | undefined = callee;
  let viaDescribe = false;
  while (current !== undefined && !ts.isIdentifier(current)) {
    viaDescribe ||= ts.isPropertyAccessExpression(current) && current.name.text === 'describe';
    current = innerCallee(current);
  }
  return current === undefined ? undefined : { name: current.text, viaDescribe };
}

/** One step into a callee: what a factory's call, a tagged template or a modifier applies to. */
function innerCallee(callee: ts.Expression): ts.Expression | undefined {
  if (ts.isCallExpression(callee)) return factoryOf(callee.expression);
  if (ts.isTaggedTemplateExpression(callee)) return factoryOf(callee.tag);
  if (!ts.isPropertyAccessExpression(callee)) return undefined;
  const name = callee.name.text;
  return name === 'describe' || MODIFIERS.has(name) ? callee.expression : undefined;
}

// `it.each(rows)` and `it.skipIf(cond)` return the declaring function; any other call in
// callee position is not a test API.
function factoryOf(expression: ts.Expression): ts.Expression | undefined {
  return ts.isPropertyAccessExpression(expression) && FACTORIES.has(expression.name.text)
    ? expression.expression
    : undefined;
}

function literalTitle(node: ts.Expression | undefined): string | undefined {
  if (!node) return undefined;
  return ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)
    ? node.text
    : undefined;
}
