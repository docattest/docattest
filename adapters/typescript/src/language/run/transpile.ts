import ts from '../ts.cjs';
import { isObject } from '../../json.js';
import { threadCache } from '../cache.js';
import { IMPORT_META } from './protocol.js';
import { decodeLines } from './sourcemap.js';

/** CommonJS for the worker's module registry, with a map from generated to source lines. */
export interface Transpiled {
  code: string;
  /** For each generated line, the 0-based source line it starts, if any. */
  lines: (number | undefined)[];
}

// Transpiling is a pure function of path and text, so every copy of docattest shares it.
const CACHE = threadCache<Transpiled>('transpiled');

/**
 * Transpile TypeScript or JavaScript to CommonJS. Blocks and project modules run in a fresh
 * registry per block, which needs a module format whose cache docattest owns.
 */
export function transpile(file: string, text: string): Transpiled {
  const key = `${file}\0${text}`;
  const cached = CACHE.get(key);
  if (cached) return cached;
  const out = ts.transpileModule(text, {
    fileName: file,
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      esModuleInterop: true,
      sourceMap: true,
      allowJs: true,
      jsx: ts.JsxEmit.React,
      verbatimModuleSyntax: false,
    },
    transformers: { before: [importMetaTransformer] },
  });
  const code = out.outputText.replace(/\n\/\/# sourceMappingURL=.*\s*$/, '\n');
  const transpiled = { code, lines: decodeLines(mappingsOf(out.sourceMapText)) };
  CACHE.set(key, transpiled);
  return transpiled;
}

function mappingsOf(sourceMap: string | undefined): string {
  const parsed: unknown = JSON.parse(sourceMap ?? '{}');
  return isObject(parsed) && typeof parsed.mappings === 'string' ? parsed.mappings : '';
}

// CommonJS has no `import.meta`; ESM code (including docattest's own build) uses it for
// paths relative to the module, so it becomes a variable the registry provides.
function importMetaTransformer(context: ts.TransformationContext): ts.Transformer<ts.SourceFile> {
  const visit = (node: ts.Node): ts.Node => {
    if (ts.isMetaProperty(node) && node.keywordToken === ts.SyntaxKind.ImportKeyword) {
      return context.factory.createIdentifier(IMPORT_META);
    }
    return ts.visitEachChild(node, visit, context);
  };
  return (source) => ts.visitEachChild(source, visit, context);
}
