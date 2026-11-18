// What the mirror test's translations share: the shape of one, and the helpers each is
// written with. A translation is by hand: a table of page fragments and a table of source
// files, each for one language (python.ts, go.ts, rust.ts).
import type { ProjectFiles } from '../lib/project.ts';

/** A TypeScript source file's translation: its path and text in the other language. */
export type Translation = (ts: string) => [path: string, text: string];

/** One language a scenario is translated to. */
export interface Language {
  /** The adapter's name, as a config writes it. */
  adapter: string;
  /** A short label for the columns the test prints. */
  label: string;
  /** The project's packaging file, which stands in for package.json. */
  manifest: [path: string, text: string];
  /**
   * The library's surface, as this language's config writes it: every TypeScript entry file
   * that is not a program is this one module.
   */
  surface: string;
  /** The TypeScript entry files that are programs, each with its surface in this language. */
  programs: Readonly<Record<string, string>>;
  /** Page fragments, each replaced wherever it occurs. Pages keep their line count. */
  page: readonly [from: string, to: string][];
  /** Each TypeScript source file → its path and text in this language. */
  code: Readonly<Record<string, Translation>>;
  /**
   * Fragments of what the core prints, put back as the scenario's `expect` names them,
   * for a language that cannot keep a TypeScript name (Go exports begin with a capital).
   */
  restate?: readonly [from: string, to: string][];
}

const replaced = (text: string, table: readonly [from: string, to: string][]): string =>
  table.reduce((t, [from, to]) => t.split(from).join(to), text);

/** The first match of `pattern` in `text` (group `group`), which the fixture must hold. */
export function found(pattern: RegExp, text: string, group = 0): string {
  const match = pattern.exec(text)?.[group];
  if (match === undefined) throw new Error(`no ${String(pattern)} in ${text}`);
  return match;
}

/** The array literal a TypeScript source assigns. */
export const list = (text: string): string => found(/\[.*\]/s, text);

/** The number of attempts a TypeScript source sets. */
export const attempts = (text: string): string => found(/maxAttempts: (\d+)/, text, 1);

/**
 * A TypeScript project's files (or a change to them) as the same project in `language`. Its
 * pages and TypeScript sources are translated; any other file, such as one a fact reads, is
 * the same in every language.
 */
export function translate(
  language: Language,
  files: ProjectFiles<string>,
): Record<string, string | null> {
  const out: Record<string, string | null> = {};
  for (const [path, text] of Object.entries(files)) {
    if (path === 'package.json') {
      out[language.manifest[0]] = language.manifest[1];
    } else if (path === 'docattest.config.json') {
      out[path] = config(language, files, text);
    } else if (path.endsWith('.md')) {
      out[path] = text === null ? null : replaced(text, language.page);
    } else if (path.endsWith('.ts')) {
      const [to, translated] = source(language, path, text);
      out[to] = translated;
    } else {
      out[path] = text;
    }
  }
  return out;
}

/** The scenario's own config, as the bench fixture writes it, with its surfaces in `language`. */
function config(language: Language, files: ProjectFiles<string>, text: string | null): string {
  const written = JSON.parse(text ?? 'null') as Record<string, unknown>;
  const surfaces = Object.fromEntries(
    Object.entries(written.surfaces as Record<string, string>).map(([name, entry]) => [
      name,
      language.programs[entry] ?? language.surface,
    ]),
  );
  const setup = written.setup as string | undefined;
  return JSON.stringify({
    ...written,
    surfaces,
    ...(setup === undefined ? {} : { setup: source(language, setup, files[setup] ?? null)[0] }),
    adapter: language.adapter,
  });
}

/** A TypeScript source file as `language` writes it: its path and its text. */
function source(language: Language, path: string, text: string | null): [string, string] {
  const translation = language.code[path];
  if (!translation || text === null) throw new Error(`no translation for ${path}`);
  return translation(text);
}

/** What the core printed for `language`, in the names the scenario's `expect` uses. */
export const restated = (language: Language, stdout: string): string =>
  replaced(stdout, language.restate ?? []);
