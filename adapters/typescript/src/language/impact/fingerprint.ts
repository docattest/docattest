import ts from '../ts.cjs';
import { isReference, type UnitLookup } from './lookup.js';
import type { Unit } from './units.js';

/**
 * A unit's tokens as the compiler reads them, so that only a change of meaning changes
 * it: comments, whitespace, statement semicolons, trailing commas, redundant parentheses
 * and quote style are not in it, string and number literals count by value, every name
 * carries the units it resolves to (pointing an import elsewhere is a change), and local
 * names are numbered by binding, so renaming a parameter or a local variable is not. A
 * top-level name prints as its unit's key alone, read through `renamed` (a unit's key now
 * → its key at the base), so a helper renamed or moved along with its callers is the same.
 */
export function fingerprint(
  unit: Unit,
  lookup: UnitLookup,
  renamed: ReadonlyMap<string, string> = new Map(),
): string {
  const printer = new Printer(unit.file, lookup, renamed);
  printer.out.push(unit.prefix);
  for (const node of unit.nodes) printer.emit(node);
  return printer.out.join(' ');
}

class Printer {
  readonly out: string[] = [];
  private readonly locals = new Map<ts.Symbol, string>();

  constructor(
    private readonly file: ts.SourceFile,
    private readonly lookup: UnitLookup,
    private readonly renamed: ReadonlyMap<string, string>,
  ) {}

  emit(node: ts.Node): void {
    if (node.kind >= ts.SyntaxKind.FirstJSDocNode && node.kind <= ts.SyntaxKind.LastJSDocNode) {
      return;
    }
    if (ts.isParenthesizedExpression(node) || ts.isParenthesizedTypeNode(node)) {
      this.emit(ts.isParenthesizedExpression(node) ? node.expression : node.type);
      return;
    }
    const children = node.getChildren(this.file);
    if (children.length === 0) {
      this.out.push(this.token(node));
      return;
    }
    this.out.push(`(${ts.SyntaxKind[node.kind]}`);
    children.forEach((child, k) => {
      if (!isTerminator(child, k === children.length - 1)) this.emit(child);
    });
    this.out.push(')');
  }

  private token(node: ts.Node): string {
    if (ts.isIdentifier(node)) return this.local(node) ?? this.name(node);
    if (ts.isPrivateIdentifier(node)) return node.text;
    if (ts.isStringLiteral(node)) return `str:${JSON.stringify(node.text)}${this.targets(node)}`;
    return literal(node) ?? ts.tokenToString(node.kind) ?? ts.SyntaxKind[node.kind];
  }

  // A name bound inside the unit and invisible outside it: the same binding prints the same
  // number whatever it is called. Shadowing still shows, because binding is the checker's.
  private local(node: ts.Identifier): string | undefined {
    const symbol = this.lookup.checker.getSymbolAtLocation(node);
    const declaration = symbol?.declarations?.[0];
    if (!symbol || !declaration || !isLocal(declaration)) return undefined;
    let id = this.locals.get(symbol);
    if (id === undefined) this.locals.set(symbol, (id = `$${this.locals.size}`));
    return id;
  }

  // `{ helper }` is also the key `helper`, so its text stays even for a top-level name.
  private name(node: ts.Identifier): string {
    const targets = this.targets(node);
    const keyed = ts.isShorthandPropertyAssignment(node.parent);
    const bare = targets && !keyed && this.lookup.isTopLevelName(node);
    return bare ? targets : `${node.text}${targets}`;
  }

  private targets(node: ts.Node): string {
    if (!isReference(node)) return '';
    const keys = this.lookup.keysAt(node).map((k) => this.renamed.get(k) ?? k);
    return keys.length ? `@${keys.sort().join('|')}` : '';
  }
}

// Any other literal, by value: numbers whatever their spelling, JSX text whatever its
// whitespace, and templates and regular expressions as written.
function literal(node: ts.Node): string | undefined {
  if (ts.isNumericLiteral(node) || ts.isBigIntLiteral(node)) return `num:${node.text}`;
  if (ts.isJsxText(node)) return `jsx:${JSON.stringify(node.text.replace(/\s+/g, ' '))}`;
  if (ts.isLiteralExpression(node) || ts.isTemplateLiteralToken(node)) {
    return `${ts.SyntaxKind[node.kind]}:${JSON.stringify(node.text)}`;
  }
  return undefined;
}

// A `;` or `,` that ends its parent separates nothing: a statement's optional semicolon,
// a trailing comma, a type member's separator.
function isTerminator(node: ts.Node, last: boolean): boolean {
  return (
    last && (node.kind === ts.SyntaxKind.SemicolonToken || node.kind === ts.SyntaxKind.CommaToken)
  );
}

function isLocal(declaration: ts.Declaration): boolean {
  if (ts.isParameter(declaration) || ts.isTypeParameterDeclaration(declaration)) return true;
  if (ts.isBindingElement(declaration)) {
    // `{ a }` names a property as well as a variable; `{ a: b }` and `[a]` name variables.
    const keyed = !declaration.propertyName && ts.isObjectBindingPattern(declaration.parent);
    return !keyed && inScope(declaration);
  }
  const named = ts.isVariableDeclaration(declaration) || ts.isFunctionDeclaration(declaration);
  return (named || ts.isClassDeclaration(declaration)) && inScope(declaration.parent);
}

// Inside a function, a block or a loop: somewhere a name cannot be seen from outside.
function inScope(node: ts.Node): boolean {
  for (let n = node; !ts.isSourceFile(n) && !ts.isModuleBlock(n); n = n.parent) {
    if (ts.isFunctionLike(n) || ts.isBlock(n) || ts.isCatchClause(n)) return true;
    if (ts.isForStatement(n) || ts.isForInStatement(n) || ts.isForOfStatement(n)) return true;
  }
  return false;
}
