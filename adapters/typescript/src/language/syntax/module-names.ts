import ts from '../ts.cjs';
import { foldConstant, type Folded } from './constant-value.js';
import { mutatedNames } from './mutations.js';

/**
 * A name a module declares at its top level, as a fact may read it: a constant, whose value
 * is read from source; a `const` whose value the module computes or changes as it runs; or
 * something else, with what a message calls it.
 */
export type ModuleName =
  | { kind: 'constant'; value: unknown; declaration: ts.VariableDeclaration }
  | { kind: 'computed' }
  | { kind: 'other'; what: string };

/**
 * Every name a module declares at its top level. A constant is a `const` whose initializer
 * folds ({@link foldConstant}), naming only constants declared above it, and whose value
 * the module never changes in place. Imports and re-exports declare nothing here.
 */
export function moduleNames(source: ts.SourceFile): Map<string, ModuleName> {
  const names = new Map<string, ModuleName>();
  const mutated = mutatedNames(source);
  // A constant may name one declared above it: one below is not initialised yet when the
  // module runs, and a name it cannot fold is no constant.
  const resolve = (name: ts.Identifier): Folded | undefined => {
    const found = names.get(name.text);
    return found?.kind === 'constant' ? { value: found.value } : undefined;
  };
  for (const statement of source.statements) {
    if (!ts.isVariableStatement(statement)) {
      const declared = declaredName(statement);
      if (declared) names.set(declared.name, { kind: 'other', what: declared.what });
      continue;
    }
    const isConst = (statement.declarationList.flags & ts.NodeFlags.Const) !== 0;
    for (const declaration of statement.declarationList.declarations) {
      if (!ts.isIdentifier(declaration.name)) continue;
      const name = declaration.name.text;
      const folded = isConst && !mutated.has(name) ? initialValue(declaration, resolve) : undefined;
      names.set(
        name,
        folded
          ? { kind: 'constant', value: folded.value, declaration }
          : isConst
            ? { kind: 'computed' }
            : { kind: 'other', what: 'a variable' },
      );
    }
  }
  return names;
}

function initialValue(
  declaration: ts.VariableDeclaration,
  resolve: (name: ts.Identifier) => Folded | undefined,
): Folded | undefined {
  return declaration.initializer && foldConstant(declaration.initializer, resolve);
}

// What each kind of declaration is called in a message.
const DECLARATIONS: [(statement: ts.Node) => boolean, string][] = [
  [ts.isFunctionDeclaration, 'a function'],
  [ts.isClassDeclaration, 'a class'],
  [ts.isEnumDeclaration, 'an enum'],
  [ts.isModuleDeclaration, 'a namespace'],
  [ts.isInterfaceDeclaration, 'a type'],
  [ts.isTypeAliasDeclaration, 'a type'],
];

function declaredName(statement: ts.Statement): { name: string; what: string } | undefined {
  const kind = DECLARATIONS.find(([is]) => is(statement));
  const name = kind && (statement as ts.DeclarationStatement).name;
  return kind && name && ts.isIdentifier(name) ? { name: name.text, what: kind[1] } : undefined;
}
