import * as path from 'node:path';
import type { SourceFiles } from '../api/adapter.js';
import type { ExampleAttestation } from '../api/attestation.js';
import { sourceFileMatcher } from '../files/glob.js';
import type { FileSource } from '../files/source.js';

/** Info-string languages whose blocks the TypeScript adapter compiles and runs. */
export const BLOCK_LANGUAGES = [
  'ts',
  'typescript',
  'tsx',
  'mts',
  'cts',
  'js',
  'javascript',
  'jsx',
  'mjs',
  'cjs',
];

const JAVASCRIPT = ['js', 'javascript', 'mjs', 'cjs'];

/**
 * The language's source files, as the core is told: TypeScript and JavaScript modules of
 * every flavour, declaration files excepted (`x.d.tsx` is a module, not a declaration).
 */
export const SOURCE_FILES: SourceFiles = {
  include: ['**/*.{ts,tsx,mts,mtsx,cts,ctsx,js,jsx,mjs,mjsx,cjs,cjsx}'],
  exclude: ['**/*.d.{ts,mts,cts}'],
};

/** Installed dependencies: never the project's own pages or sources, at any depth. */
export const IGNORED_NAMES = ['node_modules'];

/** Whether a project-relative file is one of them: the adapter's own reading of that data. */
export const isSourceFile = sourceFileMatcher(SOURCE_FILES);

/**
 * The virtual module an example becomes: next to its page, so relative imports in the
 * block resolve as a reader would expect, and named after page and line for messages.
 */
export function blockFile(
  root: string,
  example: Pick<ExampleAttestation, 'page' | 'line' | 'lang'>,
): string {
  const lang = example.lang.toLowerCase();
  const ext =
    lang === 'tsx' ? '.tsx' : lang === 'jsx' ? '.jsx' : JAVASCRIPT.includes(lang) ? '.js' : '.ts';
  const name = `__docattest_${path.posix.basename(example.page)}_L${example.line}${ext}`;
  return path.join(root, path.posix.dirname(example.page), name);
}

/** The absolute path of a project-relative page. */
export function pagePath(files: FileSource, page: string): string {
  return path.join(files.root, page);
}
