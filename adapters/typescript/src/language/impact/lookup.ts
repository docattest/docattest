import ts from '../ts.cjs';
import { relativeTo } from '../../files/source.js';
import { ownerKey, unitsOf, type ModuleUnits, type Unit } from './units.js';

/**
 * The project's units in one program, and the checker's answer to "which units does this
 * name refer to". Only project files hold units: lib and dependency declarations never
 * change under a diff.
 */
export class UnitLookup {
  readonly checker: ts.TypeChecker;
  private readonly modules = new Map<ts.SourceFile, ModuleUnits>();
  private readonly byKey = new Map<string, Unit>();

  constructor(
    readonly program: ts.Program,
    private readonly root: string,
  ) {
    this.checker = program.getTypeChecker();
  }

  /** The project-relative path of a module, or undefined when it is not the project's. */
  relative(file: ts.SourceFile): string | undefined {
    const rel = relativeTo(this.root, file.fileName);
    return rel === undefined || /(^|\/)node_modules\//.test(rel) ? undefined : rel;
  }

  /** The units of a project module (empty for any other). */
  units(file: ts.SourceFile): ModuleUnits {
    let found = this.modules.get(file);
    if (!found) {
      const rel = this.relative(file);
      found = rel === undefined ? { units: new Map(), owner: new Map() } : unitsOf(file, rel);
      for (const unit of found.units.values()) this.byKey.set(unit.key, unit);
      this.modules.set(file, found);
    }
    return found;
  }

  /** A unit by key, once its module has been seen. */
  unit(key: string): Unit | undefined {
    return this.byKey.get(key);
  }

  /** Project modules of the program. */
  projectFiles(): ts.SourceFile[] {
    return this.program.getSourceFiles().filter((f) => this.relative(f) !== undefined);
  }

  /** Units a symbol's declarations live in; a module symbol stands for all its units. */
  keysOfSymbol(symbol: ts.Symbol): string[] {
    const keys = new Set<string>();
    const roots =
      symbol.flags & ts.SymbolFlags.Transient ? this.checker.getRootSymbols(symbol) : [];
    for (const s of [symbol, ...roots]) {
      for (const d of s.declarations ?? []) {
        if (ts.isSourceFile(d)) for (const k of this.units(d).units.keys()) keys.add(k);
        else {
          const key = ownerKey(d, this.units(d.getSourceFile()).owner);
          if (key !== undefined) keys.add(key);
        }
      }
    }
    return [...keys];
  }

  /** Units a name in the code refers to, through imports and re-exports. */
  keysAt(node: ts.Node): string[] {
    const symbol = this.symbolAt(node);
    return symbol ? this.keysOfSymbol(symbol) : [];
  }

  /**
   * Whether a name is a module's own top-level declaration (or a module), so that its
   * text says nothing its unit key does not: renaming or moving it changes only the key.
   */
  isTopLevelName(node: ts.Node): boolean {
    const declarations = this.symbolAt(node)?.declarations ?? [];
    const own = (d: ts.Declaration) =>
      ts.isSourceFile(d) || this.units(d.getSourceFile()).owner.has(d);
    return declarations.length > 0 && declarations.every(own);
  }

  private symbolAt(node: ts.Node): ts.Symbol | undefined {
    const parent = node.parent as ts.Node | undefined;
    const symbol =
      parent && ts.isShorthandPropertyAssignment(parent) && parent.name === node
        ? this.checker.getShorthandAssignmentValueSymbol(parent)
        : this.checker.getSymbolAtLocation(node);
    return symbol && symbol.flags & ts.SymbolFlags.Alias ? this.resolveAlias(symbol) : symbol;
  }

  /** The symbol an alias (an import, a re-export) finally names. */
  resolveAlias(symbol: ts.Symbol): ts.Symbol {
    return symbol.flags & ts.SymbolFlags.Alias ? this.checker.getAliasedSymbol(symbol) : symbol;
  }
}

/** Whether a node is a name the checker can resolve to a declaration. */
export function isReference(node: ts.Node): boolean {
  if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) return true;
  const parent = node.parent as ts.Node | undefined;
  // `obj['key']` names a property as surely as `obj.key`.
  return (
    ts.isStringLiteral(node) &&
    parent !== undefined &&
    ts.isElementAccessExpression(parent) &&
    parent.argumentExpression === node
  );
}
