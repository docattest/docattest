// The package exports values the core owns. Each is held here to what the core does, so a
// change in the core that the copy does not follow fails.
import { mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { coreUnderTest } from '../../core-under-test.ts';
import {
  CHANGE_TYPES,
  CONFIG_KEYS,
  createProject,
  DEFAULT_SNAPSHOT_MAX_AGE_DAYS,
  DEFAULT_TIMEOUT_MS,
  type Change,
} from 'docattest';

const DAY_MS = 86_400_000;
const config = (more: Record<string, unknown> = {}) =>
  JSON.stringify({
    pages: ['docs/**/*.md'],
    surfaces: { default: './src/index.ts' },
    adapter: 'typescript',
    ...more,
  });

describe('DEFAULT_SNAPSHOT_MAX_AGE_DAYS', () => {
  it('is the age past which the core calls a snapshot old', async () => {
    const now = new Date('2026-01-01T00:00:00Z');
    const fetched = (days: number) =>
      new Date(now.getTime() - days * DAY_MS).toISOString().slice(0, 10);
    const snapshot = (days: number) =>
      JSON.stringify({ source: 'https://example.com/x', fetchedAt: fetched(days), data: {} });
    const report = await createProject({
      files: {
        'docattest.config.json': config({ snapshots: { dir: 'docs/.snapshots' } }),
        'src/index.ts': 'export const N = 1;',
        'docs/a.md': 'Text.\n',
        'docs/.snapshots/limit.json': snapshot(DEFAULT_SNAPSHOT_MAX_AGE_DAYS),
        'docs/.snapshots/past.json': snapshot(DEFAULT_SNAPSHOT_MAX_AGE_DAYS + 1),
      },
      env: { now },
    }).check();
    expect(report.snapshots.map((s) => [s.name, s.warning])).toEqual([
      ['limit', false],
      ['past', true],
    ]);
  });
});

describe('CONFIG_KEYS', () => {
  it('are the keys the core names when it refuses one it does not know', async () => {
    const check = createProject({
      files: { 'docattest.config.json': config({ unknownKey: true }), 'src/index.ts': '' },
    }).check();
    await expect(check).rejects.toThrow(`(known: ${CONFIG_KEYS.join(', ')})`);
  });
});

describe('DEFAULT_TIMEOUT_MS', () => {
  it('is the time limit the core gives the adapter when the config sets none', async () => {
    const log = join(mkdtempSync(join(tmpdir(), 'docattest-timeout-')), 'exchange.log');
    const adapter = [
      process.execPath,
      fileURLToPath(new URL('../../../dist/protocol/main.js', import.meta.url)),
    ];
    const core = coreUnderTest();
    const recorded = [core, 'conformance', 'record', log, '--', ...adapter];
    const before = process.env.DOCATTEST_ADAPTER_TYPESCRIPT;
    process.env.DOCATTEST_ADAPTER_TYPESCRIPT = JSON.stringify(recorded);
    try {
      await createProject({
        files: { 'docattest.config.json': config(), 'src/index.ts': '', 'docs/a.md': 'Text.\n' },
      }).check();
    } finally {
      if (before === undefined) delete process.env.DOCATTEST_ADAPTER_TYPESCRIPT;
      else process.env.DOCATTEST_ADAPTER_TYPESCRIPT = before;
    }
    const sent = readFileSync(log, 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((entry) => JSON.parse(entry) as { to: string; line: string })
      .filter((entry) => entry.to === 'adapter')
      .map((entry) => JSON.parse(entry.line) as { method: string; params: { timeoutMs?: number } });
    expect(sent.find((request) => request.method === 'open')?.params.timeoutMs).toBe(
      DEFAULT_TIMEOUT_MS,
    );
  });
});

describe('CHANGE_TYPES', () => {
  it('are the types of change the core reports against a base', async () => {
    const files = {
      'docattest.config.json': config(),
      'src/index.ts':
        'export const ok = (v: unknown) => v; export const LIMIT = 3; export const MAX = 3;',
    };
    const fact = 'A call is tried at most `3` times. <!-- fact: LIMIT -->\n';
    const block = (mode: string) =>
      `# ok\n\n\`\`\`ts ${mode}\nimport { ok } from 'docattest-fixture';\nassert.equal(ok(7), 7);\n\`\`\`\n`;
    const changesOf = async (base: Record<string, string>, now: Record<string, string>) =>
      (
        await createProject({ files: { ...files, ...now } }).check({
          base: { files: { ...files, ...base } },
        })
      ).changes ?? [];
    const found: Change[][] = await Promise.all([
      changesOf({ 'docs/r.md': fact }, { 'docs/r.md': 'A call is tried at most `3` times.\n' }),
      changesOf(
        { 'docs/r.md': fact },
        { 'docattest.config.json': config({ frozen: ['docs/r.md'] }), 'docs/r.md': fact },
      ),
      changesOf({ 'docs/ok.md': block('check') }, { 'docs/ok.md': block('compile') }),
      changesOf({ 'docs/r.md': fact }, { 'docs/r.md': fact.replace('LIMIT', 'MAX') }),
    ]);
    expect(found.map((changes) => changes.map((c) => c.type))).toEqual(
      CHANGE_TYPES.map((type) => [type]),
    );
  });
});
