// One exchange with the built server: lines in, the input closed, responses out.
import { spawnSync } from 'node:child_process';
import { SERVER_COMMAND } from './process.ts';

export interface Exchange {
  code: number | null;
  stdout: string;
  stderr: string;
  /** Responses in the order they were written. */
  responses: { id: unknown; result?: unknown; error?: { kind: string; message: string } }[];
}

/** Send every line, close the input, and collect what came back. */
export function exchange(lines: readonly (string | object)[]): Exchange {
  const input = lines.map((l) => (typeof l === 'string' ? l : JSON.stringify(l))).join('\n');
  const [command = '', ...args] = SERVER_COMMAND;
  const out = spawnSync(command, args, { input: `${input}\n`, encoding: 'utf8' });
  const responses = out.stdout
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line) as Exchange['responses'][number]);
  return { code: out.status, stdout: out.stdout, stderr: out.stderr, responses };
}

export const byId = (x: Exchange, id: unknown) => x.responses.find((r) => r.id === id);
export const HELLO = { id: 0, method: 'hello', params: { protocol: 1 } };

/** `open` for an in-memory project with one surface. */
export function open(id: number, files: Record<string, string>, examples: unknown[] = []) {
  const surfaces = [{ name: 'default', entry: './src/index.ts' }];
  const snapshots = { data: {} };
  const params = { root: '/unused', files, surfaces, timeoutMs: 10_000, workers: 2 };
  return { id, method: 'open', params: { ...params, network: false, snapshots, examples } };
}

export const example = (id: string, code: string) => ({
  id,
  kind: 'example',
  page: 'docs/a.md',
  line: 1,
  claim: '',
  lang: 'ts',
  mode: 'check',
  network: false,
  code,
  weak: false,
});

export const fact = (id: string, expression: string) => ({
  id,
  kind: 'fact',
  page: 'docs/a.md',
  line: 1,
  claim: '',
  expression,
  surface: 'default',
  ordered: false,
});
