// One test per automatable event in CORPUS.md. Each reproduces the drift in a
// small project and asserts docattest fails at the right line with the right message,
// then shows the corrected page passing. Titles cite the corpus row and the docs page
// whose behaviour the test relies on.
import { describe, it, expect } from 'vitest';
import { project } from './project.ts';

describe('corpus: copied lists and values', () => {
  it('#1 env var list missing six keys (docs/facts.md: list set compare)', async () => {
    const surface = `export const ENV_KEYS = ['API_TOKEN', 'REGION', 'PORT', 'TTS_ENDPOINT', 'STT_ENDPOINT', 'RATE'];`;
    const stale =
      'Environment variables override the file: <!-- fact: ENV_KEYS -->\n- `API_TOKEN`\n- `REGION`\n- `PORT`\n';
    const report = await project({ 'src/index.ts': surface, 'README.md': stale }).check();
    expect(report.results).toHaveLength(1);
    expect(report.results[0]).toMatchObject({
      page: 'README.md',
      line: 1,
      kind: 'fact',
      status: 'fail',
    });
    expect(report.results[0].message).toMatch(/missing: RATE, STT_ENDPOINT, TTS_ENDPOINT/);
    expect(report.exitCode).toBe(1);

    const fixed = stale + '- `TTS_ENDPOINT`\n- `STT_ENDPOINT`\n- `RATE`\n';
    expect((await project({ 'src/index.ts': surface, 'README.md': fixed }).check()).exitCode).toBe(
      0,
    );
  });

  it('#2 endpoint table missing three routes (docs/facts.md: table column compare)', async () => {
    const surface = `export const ROUTES = ['GET /api/status', 'GET /api/horizon', 'POST /api/say', 'GET /api/distractors', 'POST /api/sync'];`;
    const stale = [
      '<!-- fact column=Endpoint: ROUTES -->',
      '| Endpoint | Purpose |',
      '| --- | --- |',
      '| `GET /api/status` | health |',
      '| `GET /api/horizon` | snapshot |',
    ].join('\n');
    const report = await project({ 'src/index.ts': surface, 'README.md': stale }).check();
    expect(report.results[0]).toMatchObject({ line: 1, status: 'fail' });
    expect(report.results[0].message).toMatch(
      /missing: GET \/api\/distractors, POST \/api\/say, POST \/api\/sync/,
    );
  });

  it('#4 hand-copied error code block missing one code (docs/facts.md: fenced block text compare)', async () => {
    const surface = `export const ErrorCodes = { RPC_ERROR: 'RPC_ERROR', NO_SESSION: 'NO_SESSION', AUTH_ERROR: 'AUTH_ERROR' } as const;\nexport const ErrorCodesSource = "{ RPC_ERROR: 'RPC_ERROR', NO_SESSION: 'NO_SESSION', AUTH_ERROR: 'AUTH_ERROR' }";`;
    const stale = [
      '# Errors',
      '',
      '`error.code` is one of: <!-- fact: Object.keys(ErrorCodes) -->',
      '- `RPC_ERROR`',
      '- `NO_SESSION`',
    ].join('\n');
    const report = await project({ 'src/index.ts': surface, 'docs/errors.md': stale }).check();
    expect(report.results[0]).toMatchObject({
      page: 'docs/errors.md',
      line: 3,
      status: 'fail',
      expected: '[RPC_ERROR, NO_SESSION]',
      actual: '[RPC_ERROR, NO_SESSION, AUTH_ERROR]',
    });
    expect(report.results[0].message).toMatch(/missing: AUTH_ERROR/);
  });

  it('#5 "two networks" when the enum has three (docs/facts.md: list set compare over an enum)', async () => {
    const surface = `export const Network = { PRODUCTION: 'production', STAGING: 'staging', LOCAL: 'local' } as const;`;
    const stale =
      'The SDK supports these networks: <!-- fact: Object.values(Network) -->\n- `production`\n- `staging`\n';
    const report = await project({ 'src/index.ts': surface, 'docs/config.md': stale }).check();
    expect(report.results[0].status).toBe('fail');
    expect(report.results[0].message).toMatch(/missing: local/);
  });
});

describe('corpus: external and typed facts', () => {
  it('#6 scope descriptions differ from the live endpoint (docs/snapshots.md: fact over a snapshot column)', async () => {
    const snapshot = JSON.stringify({
      source: 'https://api.example.com/scopes',
      fetchedAt: '2026-10-01',
      data: [{ scope: 'jobs:read', description: 'Read jobs you have run' }],
    });
    const stale = [
      '<!-- fact column=Allows: snapshot("scopes").map(s => s.description) -->',
      '| Permission | Allows |',
      '|---|---|',
      "| `jobs:read` | Read jobs you've run |",
    ].join('\n');
    const files = {
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        surfaces: { default: './src/index.ts' },
        snapshots: { dir: 'docs/.snapshots' },
        adapter: 'typescript',
      }),
      'src/index.ts': 'export const x = 1;',
      'docs/.snapshots/scopes.json': snapshot,
      'docs/scopes.md': stale,
    };
    const report = await project(files, { now: new Date('2026-10-20') }).check();
    expect(report.results[0].status).toBe('fail');
    expect(report.results[0].message).toMatch(/missing: Read jobs you have run/);
  });

  it('#7 interfaces retyped by hand, base type omitted (docs/examples.md: compile block against real types)', async () => {
    const surface = `export interface BalanceInfo { amount: bigint }\nexport interface TokenBalanceInfo extends BalanceInfo { mint: string }`;
    const stale = [
      '# Types',
      '',
      '```ts compile',
      "import type { TokenBalanceInfo } from 'docattest-fixture';",
      'const t: TokenBalanceInfo = { mint: "m" };',
      '```',
    ].join('\n');
    const report = await project({ 'src/index.ts': surface, 'docs/types.md': stale }).check();
    expect(report.results[0]).toMatchObject({ line: 3, kind: 'example', status: 'fail' });
    expect(report.results[0].message).toMatch(/amount/);
  });
});
