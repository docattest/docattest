// The project a benchmark scenario builds on: a package, the standard config, and its files.

/** A docattest.config.json, as a scenario writes it. */
export type Config = Readonly<Record<string, unknown>>;

export const CONFIG = {
  pages: ['docs/**/*.md', 'README.md'],
  surfaces: { default: './src/index.ts' },
  adapter: 'typescript',
};

/** A project on disk: package.json (name `fixture`), the standard config, and `files`. */
export function project(
  files: Readonly<Record<string, string>>,
  config: Config = CONFIG,
): Record<string, string> {
  return {
    'package.json': JSON.stringify({ name: 'fixture', version: '1.0.0', type: 'module' }),
    'docattest.config.json': JSON.stringify(config),
    ...files,
  };
}

/** 1-based line of the first line of `text` that contains `needle`. */
export function lineOf(text: string, needle: string): number {
  const at = text.split('\n').findIndex((line) => line.includes(needle));
  if (at === -1) throw new Error(`fixture bug: "${needle}" not in text`);
  return at + 1;
}
