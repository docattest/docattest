// What `open` is told of the project and what the session says of its code, and where the
// server's responses may go.
import { spawnSync } from 'node:child_process';
import { closeSync, mkdtempSync, openSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { byId, exchange, HELLO, open } from './exchange.ts';
import { SERVER_COMMAND } from './process.ts';

describe('the adapter server, given a project', () => {
  it('reads the surfaces as the config states them, and finds names in the code', () => {
    const files = {
      'src/index.ts': 'export const LIMIT = 3; // was MAX',
      'src/other.ts': 'export const OTHER = 1;',
      'lib/unlisted.ts': 'export const HIDDEN = 1;',
    };
    const params = { ...open(1, files).params };
    const both = { name: 'other', entry: { import: 'x', path: './src/other.ts' } };
    const surfaces = [
      { name: 'default', entry: './src/index.ts' },
      both,
      { name: 'pkg', entry: 'some-package' },
    ];
    const names = ['LIMIT', 'MAX', 'OTHER', 'HIDDEN', 'GONE'];
    const x = exchange([
      HELLO,
      { id: 1, method: 'open', params: { ...params, surfaces } },
      { id: 2, method: 'exports', params: { surface: 'default' } },
      { id: 3, method: 'exports', params: { surface: 'other' } },
      { id: 4, method: 'inCode', params: { names, files: [] } },
      { id: 5, method: 'inCode', params: { names, files: ['lib/unlisted.ts', 'missing.ts'] } },
      { id: 6, method: 'describeSurfaces', params: {} },
    ]);
    expect(byId(x, 2)?.result).toEqual(['LIMIT']);
    expect(byId(x, 3)?.result).toEqual(['OTHER']);
    expect(byId(x, 4)?.result).toEqual(['LIMIT', 'MAX', 'OTHER']);
    expect(byId(x, 5)?.result).toEqual(['LIMIT', 'MAX', 'OTHER', 'HIDDEN']);
    expect(Object.keys(byId(x, 6)?.result as object)).toEqual(['default', 'other', 'pkg']);
    const old = [{ name: 'default', import: 'docattest-fixture', path: 'src/index.ts' }];
    const refused = exchange([
      HELLO,
      { id: 1, method: 'open', params: { ...params, surfaces: old } },
    ]);
    expect(refused.responses[1]?.error).toEqual({
      kind: 'internal',
      message: 'open: each of "surfaces" must have a "name" and an "entry"',
    });
  });

  it('writes its responses to a file or a device as well as to a pipe', () => {
    const [command = '', ...args] = SERVER_COMMAND;
    const input = `${JSON.stringify(HELLO)}\n`;
    const sink = openSync('/dev/null', 'w');
    const dir = mkdtempSync(join(tmpdir(), 'docattest-out-'));
    const file = openSync(join(dir, 'out'), 'w');
    try {
      const quiet = spawnSync(command, args, { input, stdio: ['pipe', sink, 'pipe'] });
      expect([quiet.status, quiet.stderr.toString()]).toEqual([0, '']);
      const kept = spawnSync(command, args, { input, stdio: ['pipe', file, 'pipe'] });
      expect([kept.status, kept.stderr.toString()]).toEqual([0, '']);
      expect(JSON.parse(readFileSync(join(dir, 'out'), 'utf8'))).toMatchObject({ id: 0 });
    } finally {
      closeSync(sink);
      closeSync(file);
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
