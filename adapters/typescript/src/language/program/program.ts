import * as path from 'node:path';
import ts from '../ts.cjs';
import type { FileSource } from '../../files/source.js';
import { GLOBALS, GLOBALS_FILE, NODE_FALLBACK } from './globals.js';
import { createHost } from './host.js';
import { compilerOptions } from './options.js';

/** What one program is built from. */
interface ProgramInput {
  files: FileSource;
  /** Absolute path → code of each example block. */
  examples: ReadonlyMap<string, string>;
  /** Import specifier → absolute path of a surface's types entry (path surfaces only). */
  surfaces: ReadonlyMap<string, string>;
  /** Project files already parsed for the session's other program, and filled for it. */
  parsed: Map<string, ts.SourceFile>;
}

/**
 * What a program answers. Type-checking examples needs the lib files (megabytes of
 * declarations, most of a program's cost); which names a module exports does not, since
 * the binder decides them, so that program leaves the lib files out.
 */
export type ProgramPurpose = 'check' | 'exports';

/** A program over the project: surfaces, their imports, and one module per example. */
export function createProgram(input: ProgramInput, purpose: ProgramPurpose): ts.Program {
  const options = { ...compilerOptions(input.files), noLib: purpose === 'exports' };
  const globals = path.join(input.files.root, GLOBALS_FILE);
  const virtual = new Map(input.examples);
  virtual.set(globals, GLOBALS + (options.types?.length ? '' : NODE_FALLBACK));
  const { files, surfaces, parsed } = input;
  const host = createHost({ files, options, virtual, surfaces, parsed });
  // As tsc does: a comment that cannot change a type or an error is not parsed.
  host.jsDocParsingMode = ts.JSDocParsingMode.ParseForTypeErrors;
  const rootNames = [...new Set([globals, ...input.surfaces.values(), ...input.examples.keys()])];
  return ts.createProgram({ rootNames, options, host });
}

/** Diagnostics for one module as `TS1234: message` with a 1-based line. */
export function diagnosticsFor(
  program: ts.Program,
  file: string,
): { message: string; line: number }[] {
  const source = program.getSourceFile(file);
  if (!source) {
    return [{ message: 'docattest: example module was not part of the program', line: 1 }];
  }
  const diagnostics = [
    ...program.getSyntacticDiagnostics(source),
    ...program.getSemanticDiagnostics(source),
  ];
  return diagnostics.map((d) => ({
    message: `TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, ' ')}`,
    line: d.start === undefined ? 1 : source.getLineAndCharacterOfPosition(d.start).line + 1,
  }));
}

/** Resolve a bare specifier to a types file, the way the checker would from the root. */
export function resolveTypes(
  program: ts.Program,
  specifier: string,
  root: string,
): string | undefined {
  const options = program.getCompilerOptions();
  const from = path.join(root, 'package.json');
  return ts.resolveModuleName(specifier, from, options, ts.createCompilerHost(options))
    .resolvedModule?.resolvedFileName;
}
