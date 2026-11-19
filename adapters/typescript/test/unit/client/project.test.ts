// The programmatic API as a client of the core: what it runs, what it returns, how the
// core's errors come back, and where it cannot do what the in-process API did.
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, describe, expect, it } from 'vitest';
import { COMMANDS, createProject, InternalError, UserError } from 'docattest';
import { CONFIG, project } from '../../project.ts';

const SRC = { 'src/index.ts': 'export const N = 3;\nexport const M = 4;' };
const FACT = 'A call is tried at most `3` times. <!-- fact: N -->';
const LAUNCHER = fileURLToPath(new URL('../../../dist/launcher/main.js', import.meta.url));
const INDEX = fileURLToPath(new URL('../../../dist/index.js', import.meta.url));

const made: string[] = [];
afterAll(() => {
  for (const root of made) rmSync(root, { recursive: true, force: true });
});

function onDisk(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), 'docattest-client-'));
  made.push(root);
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(root, path, '..'), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  return root;
}

describe('createProject, running the core', () => {
  it('returns the report the core prints with --json, for a project in memory or on disk', async () => {
    const files = { 'docattest.config.json': CONFIG, ...SRC, 'docs/a.md': FACT };
    const inMemory = await createProject({ files }).check();
    const root = onDisk(files);
    const fromDisk = await createProject({ root }).check();
    const printed = spawnSync(process.execPath, [LAUNCHER, 'check', '--json'], {
      cwd: root,
      encoding: 'utf8',
    });
    expect(fromDisk).toEqual(JSON.parse(printed.stdout));
    expect(inMemory).toEqual(fromDisk);
    expect(inMemory.results[0]).toMatchObject({ page: 'docs/a.md', line: 1, status: 'pass' });
  });

  it('answers the queries as the commands of the same names do', async () => {
    const p = project({ ...SRC, 'docs/a.md': FACT });
    expect(await p.where('N')).toEqual([
      { page: 'docs/a.md', line: 1, kind: 'fact', claim: 'A call is tried at most `3` times.' },
    ]);
    expect(await p.where('--json')).toEqual([]);
    expect(Object.keys(await p.index())).toEqual(['N']);
    expect(await p.uncovered()).toEqual(['M']);
    expect((await p.inventory('4')).map((e) => e.expression)).toEqual(['M']);
    expect(await p.audit()).toEqual({ suggestions: [] });
    const explained = await p.explain('docs/a.md', 1);
    expect(explained.directives).toMatchObject([
      { line: 1, kind: 'fact', placed: { where: 'written' }, rule: 'code-span' },
    ]);
    expect(explained).toEqual(await p.explain('docs/a.md'));
    await expect(p.explain('docs/a.md', 2)).rejects.toThrow(/docs\/a\.md:2 holds no directive/);
    expect(await p.check({ pages: ['docs/none.md'] }).catch((e: unknown) => e)).toBeInstanceOf(
      UserError,
    );
  });

  it("throws the core's user errors as UserError, with its message", async () => {
    const broken = createProject({ files: { 'docattest.config.json': '{ "pages": [' } });
    const error = await broken.check().catch((e: unknown) => e);
    expect(error).toBeInstanceOf(UserError);
    expect((error as Error).message).toMatch(
      /^docattest: docattest\.config\.json is not valid JSON/,
    );
    await expect(project(SRC).uncovered('nope')).rejects.toThrow(/unknown surface "nope"/);
    expect(() => createProject({})).toThrow(UserError);
    expect(InternalError.name).toBe('InternalError');
  });

  it('takes a base as files for a project in memory, and a git ref only for one on disk', async () => {
    const base = { 'docattest.config.json': CONFIG, ...SRC, 'docs/a.md': `# A\n\n${FACT}` };
    const head = createProject({
      files: { ...base, 'docs/a.md': '# A\n\nA call is tried at most `3` times.' },
    });
    const report = await head.check({ base: { files: base } });
    expect(report.changes?.map((c) => [c.type, c.line])).toEqual([['orphaned-claim', 3]]);
    await expect(head.check({ base: 'HEAD' })).rejects.toThrow(
      /a git ref as base needs a project on disk/,
    );
    const root = onDisk(base);
    await expect(createProject({ root }).check({ base: { files: base } })).rejects.toThrow(
      /needs an in-memory project/,
    );
    await expect(createProject({ root }).check({ base: 'HEAD' })).rejects.toThrow(
      /is not in a git repository/,
    );
  });

  it('labels an impact on given files as the caller does', async () => {
    const base = { 'docattest.config.json': CONFIG, ...SRC, 'docs/a.md': 'Use `N`.' };
    const p = createProject({
      files: { ...base, 'src/index.ts': 'export const N = 4;\nexport const M = 4;' },
    });
    expect(await p.impact({ baseFiles: base })).toMatchObject({ base: 'files', symbols: ['N'] });
    expect((await p.impact({ baseFiles: base, base: 'v1' })).base).toBe('v1');
    await expect(p.impact()).rejects.toThrow(/pass `baseFiles` to impact/);
  });

  it('sets the clock and the network the core runs with', async () => {
    const snapshot = JSON.stringify({
      source: 'https://example.invalid/x',
      fetchedAt: '2026-01-01',
      data: 1,
    });
    const files = {
      'docattest.config.json': JSON.stringify({
        ...JSON.parse(CONFIG),
        snapshots: { dir: 'docs/.snapshots' },
      }),
      ...SRC,
      'docs/.snapshots/x.json': snapshot,
      'docs/a.md': '```ts check network\nassert.ok(1);\n```',
    };
    const report = await createProject({
      files,
      env: { now: new Date('2026-01-31'), network: false },
    }).check();
    expect(report.snapshots).toMatchObject([{ name: 'x', ageDays: 30 }]);
    expect(report.results[0].status).toBe('skipped');
    expect(() => createProject({ files, env: { now: new Date('nope') } })).toThrow(/env\.now/);
  });

  it('refreshes over HTTP, keeps what it wrote in memory, and takes no fetcher', async () => {
    const server = createServer((_request, response) => response.end('[1, 2]'));
    await new Promise<void>((listening) => server.listen(0, '127.0.0.1', listening));
    const { port } = server.address() as { port: number };
    const source = `http://127.0.0.1:${port}/x`;
    const files = {
      'docattest.config.json': JSON.stringify({
        ...JSON.parse(CONFIG),
        snapshots: { dir: 'docs/.snapshots' },
      }),
      ...SRC,
      'docs/.snapshots/x.json': JSON.stringify({ source, fetchedAt: '2026-01-01', data: [] }),
      'docs/a.md': 'Two: `2` <!-- fact: snapshot("x").length -->',
    };
    const p = createProject({ files, env: { now: new Date('2026-03-01') } });
    expect((await p.check()).exitCode).toBe(1);
    try {
      expect(await p.refresh()).toEqual(['x']);
    } finally {
      server.close();
    }
    expect(p.writes()).toEqual(['docs/.snapshots/x.json']);
    const after = await p.check();
    expect([after.exitCode, after.snapshots[0]?.fetchedAt]).toEqual([0, '2026-03-01']);
    // A fetcher, as the in-process API took one: refused, never silently ignored.
    const withFetcher = Reflect.apply(p.refresh.bind(p), undefined, ['x', fetch]) as Promise<
      string[]
    >;
    await expect(withFetcher).rejects.toThrow(/refresh takes no fetcher/);
  });

  it('finds the core where the launcher does, and says where it looked', () => {
    const code = `import(${JSON.stringify(INDEX)}).then((m) => m.createProject({ files: {} }).check()).catch((e) => { console.log(e.name, e.message); })`;
    const env = { ...process.env, DOCATTEST_CORE: '/nonexistent/docattest' };
    const out = spawnSync(process.execPath, ['--eval', code], { env, encoding: 'utf8' });
    expect(out.stdout).toMatch(
      /^UserError docattest: DOCATTEST_CORE \(\/nonexistent\/docattest\) is not a docattest core/,
    );
  });

  it("lists the core's commands", () => {
    const usage = spawnSync(process.execPath, [LAUNCHER, '--help'], { encoding: 'utf8' }).stdout;
    const listed = usage.split('\n').flatMap((line) => /^ {2}([a-z]+)/.exec(line)?.[1] ?? []);
    expect(listed.sort()).toEqual([...COMMANDS].sort());
  });
});
