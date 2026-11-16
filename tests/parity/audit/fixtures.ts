// The projects and helpers the audit cases (tests/parity/audit/) are built from, the
// argument lists they run, and the shape of a case.
import { lines } from '../../lib/lines.ts';
import type { ProjectFiles } from '../../lib/project.ts';

/** The argument lists a case runs, one step each. */
export type Runs = readonly (readonly string[])[];

/** A case: its name, the project it runs in, and its argument lists (default: ALL). */
export type AuditCase = readonly [name: string, files: ProjectFiles<string>, runs?: Runs];

export const pkg = JSON.stringify({ name: 'fixture', version: '1.0.0', type: 'module' });
export const config = (extra: Record<string, unknown> = {}): string =>
  JSON.stringify({
    pages: ['docs/**/*.md'],
    surfaces: { default: './src/index.ts' },
    adapter: 'typescript',
    ...extra,
  });
/** A project with one surface and one page: the smallest a matching case needs. */
export const one = (
  source: string,
  page: string,
  extra: ProjectFiles<string> = {},
): ProjectFiles<string> => ({
  'package.json': pkg,
  'docattest.config.json': config(),
  'src/index.ts': source,
  'docs/p.md': page,
  ...extra,
});
export const list = (...items: string[]): string =>
  items.map((i) => `- \`${i}\``).join('\n') + '\n';
export const table = (header: readonly string[], ...rows: (readonly string[])[]): string =>
  lines(
    `| ${header.join(' | ')} |`,
    `|${header.map(() => '---').join('|')}|`,
    ...rows.map((r) => `| ${r.join(' | ')} |`),
  );
export const fence = (info: string, text: string): string => '```' + info + '\n' + text + '\n```\n';
export const exportList = (name: string, items: readonly string[]): string =>
  `export const ${name} = ${JSON.stringify(items)};`;
export const range = <T>(n: number, f: (k: number) => T): T[] =>
  Array.from({ length: n }, (_, k) => f(k));
export const nest = (depth: number, leaf: string): string =>
  range(depth, (k) => `l${k}`).reduceRight((inner, key) => `{ ${key}: ${inner} }`, leaf);

export const ALL: Runs = [['audit'], ['audit', '--json'], ['inventory'], ['inventory', '--json']];
export const AUDIT: Runs = [['audit'], ['audit', '--json']];

export const TEMPLATE = 'name: demo\nregion: eu\nport: 8080\nlog: info\n';
export const KINDS_SRC = lines(
  exportList('SCOPES', ['read', 'write', 'admin']),
  "export const ROUTES = [{ method: 'GET', path: '/jobs' }, { method: 'POST', path: '/projects' }, { method: 'PUT', path: '/x' }];",
  `export const TEMPLATE = ${JSON.stringify(TEMPLATE)};`,
  `export const BANNER = 'docattest keeps documentation honest';`,
);
export const TWO_SURFACES = (extra: ProjectFiles<string> = {}): ProjectFiles<string> => ({
  'package.json': pkg,
  'docattest.config.json': JSON.stringify({
    pages: ['docs/**/*.md'],
    surfaces: { server: './src/server.ts', default: './src/index.ts' },
    adapter: 'typescript',
  }),
  'src/index.ts': lines(
    exportList('SHARED', ['s1', 's2', 's3']),
    exportList('ONLY_DEFAULT', ['d1', 'd2']),
  ),
  'src/server.ts': lines(
    exportList('SHARED', ['s1', 's2', 's3']),
    exportList('PORTS', ['8080', '9090']),
    `export const MOTD = 'welcome to the server, friend';`,
  ),
  ...extra,
});
