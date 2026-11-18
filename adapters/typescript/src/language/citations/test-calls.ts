import ts from '../ts.cjs';
import type { EachTable } from './each-titles.js';

/** What a call declares: a test or a suite, and the table it declares one for each row of. */
export interface TestDeclaration {
  role: 'test' | 'suite';
  table: EachTable | undefined;
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
 * What a call declares: `it(…)`, `describe.only(…)`, `it.each(rows)(…)`, `test.describe(…)`;
 * undefined for any other call.
 */
export function declarationOf(call: ts.CallExpression): TestDeclaration | undefined {
  const callee = call.expression;
  if (ts.isPropertyAccessExpression(callee) && FACTORIES.has(callee.name.text)) return undefined;
  let current: ts.Expression | undefined = callee;
  let viaDescribe = false;
  let table: EachTable | undefined;
  while (current !== undefined && !ts.isIdentifier(current)) {
    viaDescribe ||= ts.isPropertyAccessExpression(current) && current.name.text === 'describe';
    table ??= tableOf(current);
    current = innerCallee(current);
  }
  const role = current && roleOf(current.text, viaDescribe);
  return role && { role, table };
}

function roleOf(name: string, viaDescribe: boolean): TestDeclaration['role'] | undefined {
  if (viaDescribe) return name === 'test' ? 'suite' : undefined;
  if (TESTS.has(name)) return 'test';
  return SUITES.has(name) ? 'suite' : undefined;
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

// The table of `.each(rows)`, `.for(rows)` or `.each` with a template literal.
function tableOf(callee: ts.Expression): EachTable | undefined {
  if (ts.isTaggedTemplateExpression(callee)) {
    const isEach = ts.isPropertyAccessExpression(callee.tag) && callee.tag.name.text === 'each';
    return isEach ? { kind: 'template', template: callee.template } : undefined;
  }
  if (!ts.isCallExpression(callee) || !ts.isPropertyAccessExpression(callee.expression)) {
    return undefined;
  }
  const name = callee.expression.name.text;
  return name === 'each' || name === 'for' ? { kind: name, table: callee.arguments[0] } : undefined;
}
