import ts from '../ts.cjs';
import { referencedNames } from './expression.js';

/**
 * For each variable a module declares at its top level, the names its initializer refers
 * to as the module runs. A function or class the initializer defines runs only when called,
 * so the names inside it are not among them; nor are types.
 */
export function initializerUses(source: ts.SourceFile): Map<string, string[]> {
  const uses = new Map<string, string[]>();
  for (const statement of source.statements) {
    if (!ts.isVariableStatement(statement)) continue;
    for (const { name, initializer } of statement.declarationList.declarations) {
      if (ts.isIdentifier(name) && initializer) {
        uses.set(name.text, referencedNames(initializer, runsWithModule));
      }
    }
  }
  return uses;
}

function runsWithModule(node: ts.Node): boolean {
  return !ts.isFunctionLike(node) && !ts.isClassLike(node) && !ts.isTypeNode(node);
}
