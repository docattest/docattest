import ts from '../ts.cjs';

/**
 * A top-level declaration of a module: what a change and a dependency are measured in.
 * Declarations of one name merge (overloads, interface merging); a class is one unit with
 * its members. The statements that are not declarations form one unit per module, named
 * {@link TOP_LEVEL}: the module's side effects at import.
 */
export interface Unit {
  key: string;
  /** Text that belongs to the unit but not to its nodes: `export const` of a variable. */
  prefix: string;
  nodes: ts.Node[];
  file: ts.SourceFile;
}

/** The name of a module's side-effect unit. */
export const TOP_LEVEL = '<top-level>';

/** The units of one module, and the node that owns each. */
export interface ModuleUnits {
  units: Map<string, Unit>;
  owner: Map<ts.Node, string>;
}

/** The units of a module whose project-relative path is `rel`. */
export function unitsOf(file: ts.SourceFile, rel: string): ModuleUnits {
  const out: ModuleUnits = { units: new Map(), owner: new Map() };
  const add = (name: string, node: ts.Node, prefix = '') => {
    const key = `${rel}#${name}`;
    const unit = out.units.get(key) ?? { key, prefix, nodes: [], file };
    unit.nodes.push(node);
    out.units.set(key, unit);
    out.owner.set(node, key);
  };
  for (const statement of file.statements) {
    if (ts.isVariableStatement(statement)) {
      const prefix = variablePrefix(statement);
      for (const d of statement.declarationList.declarations) add(bindingName(d.name), d, prefix);
      continue;
    }
    const name = declarationName(statement);
    if (name === null) continue;
    add(name ?? TOP_LEVEL, statement);
  }
  return out;
}

/** The key of the unit holding a node, walking up to the top level. */
export function ownerKey(node: ts.Node, owner: ReadonlyMap<ts.Node, string>): string | undefined {
  for (let n = node; !ts.isSourceFile(n); n = n.parent) {
    const key = owner.get(n);
    if (key !== undefined) return key;
  }
  return undefined;
}

// `null` for statements that only wire names (imports, re-exports): the checker resolves
// through them, so the declarations they name carry the dependency. `undefined` for a
// statement that runs at import time.
function declarationName(statement: ts.Statement): string | undefined | null {
  if (wiresNames(statement)) return null;
  if (ts.isExportAssignment(statement)) return statement.isExportEquals ? 'export=' : 'default';
  if (ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement)) {
    return statement.name?.text ?? 'default';
  }
  return alwaysNamed(statement) ? statement.name.text : undefined;
}

function wiresNames(statement: ts.Statement): boolean {
  return (
    ts.isImportDeclaration(statement) ||
    ts.isImportEqualsDeclaration(statement) ||
    ts.isExportDeclaration(statement)
  );
}

function alwaysNamed(
  statement: ts.Statement,
): statement is
  ts.InterfaceDeclaration | ts.TypeAliasDeclaration | ts.EnumDeclaration | ts.ModuleDeclaration {
  return (
    ts.isInterfaceDeclaration(statement) ||
    ts.isTypeAliasDeclaration(statement) ||
    ts.isEnumDeclaration(statement) ||
    ts.isModuleDeclaration(statement)
  );
}

function variablePrefix(statement: ts.VariableStatement): string {
  const modifiers = (statement.modifiers ?? []).map((m) => ts.tokenToString(m.kind) ?? '');
  const flags = statement.declarationList.flags;
  const kind = flags & ts.NodeFlags.Const ? 'const' : flags & ts.NodeFlags.Let ? 'let' : 'var';
  return [...modifiers, kind].join(' ');
}

function bindingName(name: ts.BindingName): string {
  if (ts.isIdentifier(name)) return name.text;
  const names: string[] = [];
  const visit = (n: ts.Node): void => {
    if (ts.isBindingElement(n) && ts.isIdentifier(n.name)) names.push(n.name.text);
    else ts.forEachChild(n, visit);
  };
  visit(name);
  return names.join(',');
}
