import * as path from 'node:path';
import ts from '../ts.cjs';
import type { FileSource } from '../../files/source.js';

/**
 * Compiler options for examples: strict, modern, bundler resolution. These are docattest's,
 * not the project's: a page must compile the way a user's fresh project would.
 */
export function compilerOptions(files: FileSource): ts.CompilerOptions {
  const nodeTypes =
    !files.memory &&
    ts.sys.directoryExists(path.join(files.root, 'node_modules', '@types', 'node'));
  return {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    moduleDetection: ts.ModuleDetectionKind.Force,
    lib: ['lib.esnext.d.ts', 'lib.dom.d.ts', 'lib.dom.iterable.d.ts'],
    types: nodeTypes ? ['node'] : [],
    strict: true,
    noEmit: true,
    allowImportingTsExtensions: true,
    allowJs: true,
    resolveJsonModule: true,
    esModuleInterop: true,
    skipLibCheck: true,
    jsx: ts.JsxEmit.Preserve,
  };
}

const EXTENSIONS: Record<string, ts.Extension> = {
  '.ts': ts.Extension.Ts,
  '.tsx': ts.Extension.Tsx,
  '.mts': ts.Extension.Mts,
  '.cts': ts.Extension.Cts,
  '.js': ts.Extension.Js,
  '.jsx': ts.Extension.Jsx,
  '.mjs': ts.Extension.Mjs,
  '.cjs': ts.Extension.Cjs,
  '.json': ts.Extension.Json,
};

/** Whether a file is TypeScript source: a `.ts`-family module, not a declaration file. */
export function isTypeScriptSource(file: string): boolean {
  return /\.[mc]?tsx?$/.test(file) && !/\.d\.[mc]?ts$/.test(file);
}

/** The TypeScript extension kind of a file name. */
export function extensionOf(file: string): ts.Extension {
  if (file.endsWith('.d.mts')) return ts.Extension.Dmts;
  if (file.endsWith('.d.cts')) return ts.Extension.Dcts;
  if (file.endsWith('.d.ts')) return ts.Extension.Dts;
  return EXTENSIONS[path.extname(file)] ?? ts.Extension.Ts;
}
