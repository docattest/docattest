import ts from '../ts.cjs';

/** Parse a module for syntax-only analysis. */
export function parseSource(fileName: string, text: string): ts.SourceFile {
  const kind = fileName.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
  return ts.createSourceFile(fileName, text, ts.ScriptTarget.ES2022, true, kind);
}

/** Unique strings in first-seen order. */
export function unique(items: readonly string[]): string[] {
  return [...new Set(items)];
}
