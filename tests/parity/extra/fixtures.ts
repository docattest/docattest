// The project the extra cases (tests/parity/extra/) are built from, and the shape of a case.
import type { ProjectFiles } from '../../lib/project.ts';

/** A case: its name, the project it runs in, and the arguments the candidate is given. */
export type ExtraCase = readonly [
  name: string,
  files: ProjectFiles<string>,
  args: readonly string[],
];

/** What the candidate did: its exit code (`null` when a signal ended it) and its output. */
export interface Run {
  code: number | null;
  stdout: string;
  stderr: string;
}

export const pkg = JSON.stringify({ name: 'fixture', version: '1.0.0', type: 'module' });
export const config = (extra: Record<string, unknown> = {}): string =>
  JSON.stringify({
    pages: ['docs/**/*.md'],
    surfaces: { default: './src/index.ts' },
    adapter: 'typescript',
    ...extra,
  });
export const src =
  'export const N = 3;\nexport const KEYS = ["a", "b"];\nexport function add(a: number, b: number) { return a + b; }\n';
export const project = (
  files: ProjectFiles<string>,
  cfg: string = config(),
): ProjectFiles<string> => ({
  'package.json': pkg,
  'docattest.config.json': cfg,
  'src/index.ts': src,
  ...files,
});
