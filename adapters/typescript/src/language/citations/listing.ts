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
 * in the body of every test the citation matches. Setup in hooks or helpers is not followed.
 */
export function citedSymbols(
  fileName: string,
  code: string,
  cited: CitedPath,
  surfaceOf: SurfaceOf,
): SymbolRef[] {
  const tests = findTests(fileName, code).tests.filter((t) => matchesCitation(t, cited));
  if (!tests.length) return [];
  const { named, namespaces } = importsOf(parseSource(fileName, code), surfaceOf);
  const refs: SymbolRef[] = [];
  const visit = (node: ts.Node) => {
    if (ts.isIdentifier(node)) {
      const ref = named.get(node.text);
      if (ref) refs.push(ref);
    }
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression)) {
      const surface = namespaces.get(node.expression.text);
      if (surface) refs.push({ name: node.name.text, surface });
    }
    ts.forEachChild(node, visit);
  };
  for (const { call } of tests) call.arguments.slice(1).forEach(visit);
  return dedupe(refs);
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
