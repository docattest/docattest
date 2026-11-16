// What the adapter says of TypeScript and JavaScript text from its spelling alone. Nothing
// here loads the compiler, so the adapter can answer these before it has one.
import type { ExpressionShape } from '../../api/adapter.js';
import type { FileSource } from '../../files/source.js';
import type { SurfaceSpec } from '../surfaces/specs.js';

const ASSERTION = /\bassert\s*[.(]/;

/** Whether example code asserts anything; without an assertion it is weak coverage. */
export function asserts(code: string): boolean {
  return ASSERTION.test(code);
}

// A code span names a symbol when it starts with it: `retry`, `retry(fn)`, `new Client()`,
// `RETRY_DEFAULTS.maxAttempts`. `retry-after` and `retry.ts` are other things.
const LEADING =
  /^(?:new\s+|await\s+)?([A-Za-z_$][\w$]*)(?![\w$\-/@'"]|\.(?:[mc]?[jt]sx?|json|md|ya?ml)$)/;

/** The one of `symbols` an inline code span names, or null. */
export function mentioned(span: string, symbols: ReadonlySet<string>): string | null {
  const symbol = LEADING.exec(span.trim())?.[1];
  return symbol !== undefined && symbols.has(symbol) ? symbol : null;
}

const TOKEN = /[A-Za-z_$][\w$]*|\d+(?:\.\d+)?|'[^']*'|"[^"]*"|`[^`]*`|\S/g;
const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;
const IDENTIFIERS = /[A-Za-z_$][\w$]*/g;

/** A fact expression's tokens, each identifier replaced by a placeholder, and its identifiers. */
export function expressionShape(expression: string): ExpressionShape {
  const tokens: string[] = expression.match(TOKEN) ?? [];
  return {
    shape: tokens.map((t) => (IDENTIFIER.test(t) ? '·' : t)).join(' '),
    names: tokens.filter((t) => IDENTIFIER.test(t)),
  };
}

/**
 * Which of `names` are still written somewhere in the project's code: in `sources` and in
 * the files of the surfaces that name one. A word in a comment or a string counts.
 */
export function namesInCode(
  files: FileSource,
  specs: readonly SurfaceSpec[],
  names: readonly string[],
  sources: readonly string[],
): string[] {
  const wanted = new Set(names);
  const found = new Set<string>();
  const surfaceFiles = specs.flatMap((s) => (s.path === undefined ? [] : [s.path]));
  for (const file of new Set([...sources, ...surfaceFiles])) {
    for (const word of files.read(file)?.match(IDENTIFIERS) ?? []) {
      if (wanted.has(word)) found.add(word);
    }
  }
  return names.filter((name) => found.has(name));
}
