import ts from '../ts.cjs';
import type { SymbolRef, TestListing } from '../../api/adapter.js';
import { matchesCitation, type CitedPath } from './match.js';
import type { FileSource } from '../../files/source.js';
import { isSourceFile, pagePath } from '../languages.js';
import { dedupe, surfaceImports, type SurfaceOf } from '../syntax/imports.js';
import { parseSource } from '../syntax/parse.js';
import { findTests } from './tests-in-file.js';

/** What a suite is called in a TypeScript test file, as a message names it. */
const SUITE = 'a describe block';

/** The tests a project-relative file declares, read from its syntax; nothing is run. */
export function listTests(files: FileSource, file: string): TestListing {
  if (!isSourceFile(file)) {
    const message = `${file} is not a TypeScript or JavaScript file, so its tests cannot be read`;
    return { ok: false, message };
  }
  const code = files.read(file);
  if (code === undefined) return { ok: false, message: `${file} does not exist` };
  const found = findTests(pagePath(files, file), code);
  const plain = ({ title, describes, line }: (typeof found.tests)[number]) => ({
    title,
    describes,
    line,
  });
  const suites = found.suites.map((suite) => ({ ...plain(suite), kind: SUITE }));
  return { ok: true, tests: found.tests.map(plain), suites };
}

/**
 * Surface symbols a cited test exercises: identifiers imported from a surface that appear
 * in the body of every test the citation matches, in any `each(...)` table it runs over,
 * and in local helper functions or variables in the same file that the test references.
 */
export function citedSymbols(
  fileName: string,
  code: string,
  cited: CitedPath,
  surfaceOf: SurfaceOf,
): SymbolRef[] {
  const found = findTests(fileName, code);
  const tests = found.tests.filter((t) => matchesCitation(t, cited));
  if (!tests.length) return [];
  const source = parseSource(fileName, code);
  const { named, namespaces } = importsOf(source, surfaceOf);
  const helpers = localDeclarations(source);
  const refs: SymbolRef[] = [];
  const seen = new Set<ts.Node>();
  const visit = (node: ts.Node) => {
    if (ts.isIdentifier(node)) {
      const ref = named.get(node.text);
      if (ref) refs.push(ref);
      const local = helpers.get(node.text);
      if (local && !seen.has(local)) {
        seen.add(local);
        visit(local);
      }
    }
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression)) {
      const surface = namespaces.get(node.expression.text);
      if (surface) refs.push({ name: node.name.text, surface });
    }
    ts.forEachChild(node, visit);
  };
  for (const { call } of tests) {
    if (ts.isCallExpression(call.expression)) call.expression.arguments.forEach(visit);
    call.arguments.slice(1).forEach(visit);
  }
  return dedupe(refs);
}

/** Local helper functions and variables declared at top level or inside describe blocks. */
function localDeclarations(source: ts.SourceFile): Map<string, ts.Node> {
  const locals = new Map<string, ts.Node>();
  collectStatements(source.statements, locals);
  return locals;
}

function collectStatements(
  statements: ts.NodeArray<ts.Statement>,
  locals: Map<string, ts.Node>,
): void {
  for (const statement of statements) {
    if (ts.isFunctionDeclaration(statement) && statement.name) {
      locals.set(statement.name.text, statement);
    } else if (ts.isVariableStatement(statement)) {
      collectVariables(statement, locals);
    } else if (ts.isExpressionStatement(statement)) {
      for (const body of describeBodies(statement.expression)) collectStatements(body, locals);
    }
  }
}

function collectVariables(statement: ts.VariableStatement, locals: Map<string, ts.Node>): void {
  for (const decl of statement.declarationList.declarations) {
    if (ts.isIdentifier(decl.name) && decl.initializer) {
      locals.set(decl.name.text, decl.initializer);
    }
  }
}

function describeBodies(expression: ts.Expression): ts.NodeArray<ts.Statement>[] {
  if (!ts.isCallExpression(expression) || !isDescribeCall(expression)) return [];
  const bodies: ts.NodeArray<ts.Statement>[] = [];
  for (const arg of expression.arguments) {
    if ((ts.isArrowFunction(arg) || ts.isFunctionExpression(arg)) && ts.isBlock(arg.body)) {
      bodies.push(arg.body.statements);
    }
  }
  return bodies;
}

function isDescribeCall(call: ts.CallExpression): boolean {
  const callee = call.expression;
  const name = ts.isIdentifier(callee)
    ? callee.text
    : ts.isPropertyAccessExpression(callee) && ts.isIdentifier(callee.expression)
      ? callee.expression.text
      : '';
  return name === 'describe' || name === 'suite';
}

interface Imports {
  /** Local name → the surface symbol it was imported as. */
  named: Map<string, SymbolRef>;
  /** Namespace import name → its surface. */
  namespaces: Map<string, string>;
}

function importsOf(source: ts.SourceFile, surfaceOf: SurfaceOf): Imports {
  const imports: Imports = { named: new Map(), namespaces: new Map() };
  for (const bound of surfaceImports(source, surfaceOf)) {
    if (bound.kind === 'namespace') imports.namespaces.set(bound.local, bound.surface);
    else imports.named.set(bound.local, { name: bound.name, surface: bound.surface });
  }
  return imports;
}
