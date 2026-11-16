// The fixture project the `check --base` cases (tests/parity/changes/) edit, the helpers
// that make a case of it, and the shape of a case.

export type { ProjectFiles } from '../../lib/project.ts';
import type { ProjectFiles } from '../../lib/project.ts';

/** The argument lists a case runs, one step each. */
export type Runs = readonly (readonly string[])[];

/**
 * A case: a git repository where `base` is committed (then each of `commits`), and `head`
 * written over it (`null` deletes a file); the candidate runs each of `runs` there.
 */
export interface ChangeCase {
  name: string;
  base: ProjectFiles;
  head: ProjectFiles;
  runs: Runs;
  /** Further commits after the base, each written over the one before. */
  commits?: readonly ProjectFiles[];
  /** Where the project is in the repository (default: its root). */
  inside?: string;
  /** Files of the repository outside the project. */
  outside?: ProjectFiles;
  /** Run from the repository's root instead of the project's. */
  cwd?: 'repository';
  /** `false`: the project is not in a git repository at all. */
  git?: false;
  /** The candidate's environment, over the harness's. */
  env?: Record<string, string>;
}

export const CONFIG = {
  pages: ['docs/**/*.md'],
  frozen: ['docs/old/**'],
  sources: ['src/**/*.ts'],
  surfaces: { default: './src/index.ts', extra: './src/extra.ts' },
  setup: 'docs/docattest.setup.ts',
  adapter: 'typescript',
};
export const config = (edit: Record<string, unknown> = {}): string =>
  JSON.stringify({ ...CONFIG, ...edit });
export const SRC = [
  'export const LIMIT = 3;',
  'export const KEYS = ["A", "B"];',
  'export const TEXT = "a\\nb";',
  'export function add(a: number, b: number) { return a + b; }',
  '',
].join('\n');
export const SETUP = [
  'import assert from "node:assert";',
  'import { add } from "fixture";',
  'export const cases = { adds(row: Record<string, string>) { assert.equal(add(Number(row.a), Number(row.b)), Number(row.sum)); } };',
  '',
].join('\n');
export const PROJECT: ProjectFiles = {
  'package.json': JSON.stringify({ name: 'fixture', version: '1.0.0', type: 'module' }),
  'docattest.config.json': config(),
  'src/index.ts': SRC,
  'src/extra.ts': 'export const LIMIT = 3;\nexport const OTHER = 3;\n',
  'docs/docattest.setup.ts': SETUP,
  'test/add.test.ts': 'import { it } from "vitest";\nit("adds", () => {});\n',
};

export const page = (...lines: string[]): string => ['# Retry', '', ...lines].join('\n') + '\n';
export const FACT = 'A call is tried at most `3` times. <!-- fact: LIMIT -->';
export const BARE = 'A call is tried at most `3` times.';
export const CLAIM = 'A successful call returns its value unchanged.';
export const EXAMPLE = [
  '```ts check',
  "import { LIMIT } from 'fixture';",
  'assert.equal(LIMIT, 3);',
  '```',
] as const;
export const PAGE = page(FACT, '', CLAIM, '', ...EXAMPLE);
export const LIST = ['The keys are: <!-- fact: KEYS -->', '- `A`', '- `B`'] as const;
export const ORDERED = [
  'The keys, in order: <!-- fact ordered: KEYS -->',
  '- `A`',
  '- `B`',
] as const;
export const COLUMN = [
  'The keys by name. <!-- fact column=key: KEYS -->',
  '',
  '| key | meaning |',
  '|---|---|',
  '| `A` | first |',
  '| `B` | second |',
] as const;
export const BLOCK = [
  'The text is exactly this.',
  '',
  '<!-- fact: TEXT -->',
  '```',
  'a',
  'b',
  '```',
] as const;
export const CELL = (cell: string): string[] => [
  '| name | value |',
  '|---|---|',
  `| the limit | ${cell} |`,
];
export const CASES = [
  '| case | a | b | sum |',
  '|---|---|---|---|',
  '| small | 1 | 2 | 3 |',
  '| large | 10 | 20 | 30 |',
] as const;
export const CITED = 'Adding two numbers gives their sum. <!-- test: test/add.test.ts > "adds" -->';
export const doc = (
  example: string,
  description = 'Doubles a number, whatever its sign.',
): string => `/**\n * ${description}\n${example} */\nexport const double = (n: number) => n * 2;\n`;
export const DOC_EXAMPLE =
  ' * @example\n * // check\n * import { double } from "./double";\n * assert.equal(double(2), 4);\n';
export const LONG =
  'This sentence about the limit of `3` tries is deliberately much longer than sixty characters.';
export const ASTRAL = (lead: number): string =>
  `${'x'.repeat(lead)} limit 😀 of \`3\` tries goes on and on past the cut.`;
export const HEADERS = [
  '| id | line | 1 | a | b | sum |',
  '|---|---|---|---|---|---|',
  '| small | x | y | 1 | 2 | 3 |',
] as const;

export const JSON_AND_TEXT: Runs = [
  ['check', '--base', 'HEAD', '--json'],
  ['check', '--base', 'HEAD'],
] as const;

/** A case: files committed at the base, edits for the head, and the command lines to run. */
export const change = (
  name: string,
  base: ProjectFiles,
  head: ProjectFiles,
  runs: Runs = JSON_AND_TEXT,
): ChangeCase => ({
  name,
  base: { ...PROJECT, ...base },
  head,
  runs,
});
export const retry = (name: string, head: string, runs?: Runs): ChangeCase =>
  change(name, { 'docs/retry.md': PAGE }, { 'docs/retry.md': head }, runs);
