// `mutants` (PROTOCOL.md §3): edits to the literals of the symbols `symbols` answers, each
// line of the exchange held to the protocol's schemas by the core's recorder.
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { coreUnderTest } from '../core-under-test.ts';
import { open } from './exchange.ts';
import { SERVER_COMMAND } from './process.ts';

const FILES = {
  'src/index.ts': "export { PORT } from './config.js';\nexport function start(): void {}\n",
  'src/config.ts': '// café\nexport const PORT = 8080;\n',
};

describe('the adapter server, asked for mutants', () => {
  it('answers an entry for each symbol, in order, in lines that fit the schemas', () => {
    const dir = mkdtempSync(join(tmpdir(), 'docattest-mutants-'));
    const log = join(dir, 'exchange.log');
    const core = coreUnderTest();
    const symbols = [
      { name: 'PORT', surface: 'default' },
      { name: 'start', surface: 'default' },
      { name: 'GONE' },
    ];
    // As the core sends them: ids from 1, and `open` with every field it always writes.
    const opened = open(2, FILES);
    const lines = [
      { id: 1, method: 'hello', params: { protocol: 1 } },
      { ...opened, params: { ...opened.params, manifests: {} } },
      { id: 3, method: 'mutants', params: { symbols, limit: 8 } },
    ];
    try {
      const input = `${lines.map((l) => JSON.stringify(l)).join('\n')}\n`;
      const recorded = spawnSync(core, ['conformance', 'record', log, '--', ...SERVER_COMMAND], {
        input,
        encoding: 'utf8',
      });
      expect(recorded.status).toBe(0);
      const answer = recorded.stdout
        .split('\n')
        .filter(Boolean)
        .map((line) => JSON.parse(line) as { id: unknown; result?: unknown })
        .find((response) => response.id === 3);
      expect(answer?.result).toEqual([
        {
          edits: [
            {
              path: 'src/config.ts',
              start: 29,
              end: 33,
              replacement: '8081',
              description: '8080 → 8081',
            },
          ],
        },
        { edits: [], why: 'function' },
        { edits: [], why: 'notFound' },
      ]);
      expect(readFileSync(log, 'utf8')).toContain('"method\\":\\"mutants\\"');
      const validated = spawnSync(core, ['conformance', 'validate', log], { encoding: 'utf8' });
      expect(validated.stdout).toMatch(/schemas: \d+ recorded lines, 0 that do not fit/);
      expect(validated.status).toBe(0);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
