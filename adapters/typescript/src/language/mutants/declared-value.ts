import type { NoMutantReason, SymbolRef } from '../../api/adapter.js';
import { ownRelativeTo } from '../../files/source.js';
import { unwrapped } from '../syntax/constant-value.js';
import ts from '../ts.cjs';

/** The value a symbol is declared with in a file of the project's own, or why there is none. */
export type DeclaredValue =
  | { kind: 'value'; value: ts.Expression; source: ts.SourceFile; path: string }
  | { kind: 'none'; why: NoMutantReason };

/** The declarations of symbols as `symbols` answers them, in one program over the surfaces. */
export class DeclaredValues {
  private readonly checker: ts.TypeChecker;

  constructor(
    private readonly program: ts.Program,
    /** Surface name → the module whose exports are the surface. */
    private readonly entries: ReadonlyMap<string, string>,
    private readonly root: string,
  ) {
    this.checker = program.getTypeChecker();
  }

  /** The value `symbol` is declared with, or why it has none to change. */
  of(symbol: SymbolRef): DeclaredValue {
    const declarations =
      symbol.surface === undefined
        ? this.unattributed(symbol.name)
        : this.onSurface(symbol.surface, symbol.name);
    return this.valueOf(declarations);
  }

  // An export, through any re-export; else a constant of the surface's module that a fact
  // reads from source, exported or not.
  private onSurface(surface: string, name: string): readonly ts.Declaration[] {
    const entry = this.entries.get(surface);
    const source = entry === undefined ? undefined : this.program.getSourceFile(entry);
    if (!source) return [];
    const module = this.checker.getSymbolAtLocation(source);
    const exported = module && this.checker.getExportsOfModule(module).find((s) => s.name === name);
    if (!exported) return topLevelDeclarations(source, name);
    const target =
      exported.flags & ts.SymbolFlags.Alias ? this.checker.getAliasedSymbol(exported) : exported;
    return target.declarations ?? [];
  }

  // A constant a fact reads from source is one a surface's module declares at its top level.
  private unattributed(name: string): readonly ts.Declaration[] {
    for (const entry of this.entries.values()) {
      const source = this.program.getSourceFile(entry);
      const declarations = source ? topLevelDeclarations(source, name) : [];
      if (declarations.length) return declarations;
    }
    return [];
  }

  // A value and a type may share a name (`const Color = {…}; type Color = …`): the value is
  // what a proof reads.
  private valueOf(declarations: readonly ts.Declaration[]): DeclaredValue {
    const own = declarations.filter((d) => this.pathOf(d) !== undefined);
    if (!own.length) return { kind: 'none', why: 'notFound' };
    const values = own.filter(
      (d) => !ts.isInterfaceDeclaration(d) && !ts.isTypeAliasDeclaration(d),
    );
    if (!values.length || values.some(offersNoValue)) return { kind: 'none', why: 'function' };
    for (const declaration of values) {
      const value = valueExpression(declaration);
      const path = this.pathOf(declaration);
      if (value && path !== undefined) {
        return { kind: 'value', value, source: declaration.getSourceFile(), path };
      }
    }
    return { kind: 'none', why: 'noLiteral' };
  }

  /** The project-relative path of the file a declaration is in, when it is the project's own. */
  private pathOf(declaration: ts.Declaration): string | undefined {
    return ownRelativeTo(this.root, declaration.getSourceFile().fileName);
  }
}

/** The declarations a module makes at its top level under `name`; imports declare nothing. */
function topLevelDeclarations(source: ts.SourceFile, name: string): ts.Declaration[] {
  return source.statements.flatMap((statement): ts.Declaration[] => {
    if (ts.isVariableStatement(statement)) {
      return statement.declarationList.declarations.filter(
        (d) => ts.isIdentifier(d.name) && d.name.text === name,
      );
    }
    return isNamedStatement(statement) && statement.name?.text === name ? [statement] : [];
  });
}

type NamedStatement =
  | ts.FunctionDeclaration
  | ts.ClassDeclaration
  | ts.EnumDeclaration
  | ts.InterfaceDeclaration
  | ts.TypeAliasDeclaration;

function isNamedStatement(statement: ts.Statement): statement is NamedStatement {
  return (
    ts.isFunctionDeclaration(statement) ||
    ts.isClassDeclaration(statement) ||
    ts.isEnumDeclaration(statement) ||
    ts.isInterfaceDeclaration(statement) ||
    ts.isTypeAliasDeclaration(statement)
  );
}

// What a proof reads of a function or a class is computed when it runs, and an enum's
// members are no value's literals.
function offersNoValue(declaration: ts.Declaration): boolean {
  if (ts.isFunctionLike(declaration) || ts.isClassLike(declaration)) return true;
  if (ts.isEnumDeclaration(declaration)) return true;
  const value = valueExpression(declaration);
  const inner = value && unwrapped(value);
  return inner !== undefined && (ts.isFunctionLike(inner) || ts.isClassLike(inner));
}

/** What a declaration gives its name: a variable's initializer, a default export's expression. */
function valueExpression(declaration: ts.Declaration): ts.Expression | undefined {
  if (ts.isVariableDeclaration(declaration)) return declaration.initializer;
  return ts.isExportAssignment(declaration) ? declaration.expression : undefined;
}
