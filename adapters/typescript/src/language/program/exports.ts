import ts from '../ts.cjs';
import { compareStrings } from '../../compare-strings.js';
import { platformPath } from './file-names.js';

/** Exported names of a module, values and types, sorted; undefined when it is not loaded. */
export function exportsOf(program: ts.Program, file: string): string[] | undefined {
  const exported = exportedSymbols(program, file);
  return exported?.map((s) => s.name).sort(compareStrings);
}

/** For each exported name, the files (platform paths) that declare what it ultimately refers to. */
export function exportOrigins(program: ts.Program, file: string): Map<string, Set<string>> {
  const checker = program.getTypeChecker();
  const origins = new Map<string, Set<string>>();
  for (const symbol of exportedSymbols(program, file) ?? []) {
    const target = symbol.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
    const declarations = target.declarations ?? [];
    const declared = declarations.map((d) => platformPath(d.getSourceFile().fileName));
    origins.set(symbol.name, new Set(declared));
  }
  return origins;
}

function exportedSymbols(program: ts.Program, file: string): ts.Symbol[] | undefined {
  const source = program.getSourceFile(file);
  if (!source) return undefined;
  const checker = program.getTypeChecker();
  const module = checker.getSymbolAtLocation(source);
  return module ? checker.getExportsOfModule(module) : [];
}
