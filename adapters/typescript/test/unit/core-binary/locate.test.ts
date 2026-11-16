// Where the npm launcher finds the core, and what it refuses. The Python launcher follows
// the same rules (adapters/python/tests/test_launcher.py): one cache serves both.
import { chmodSync, cpSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  cacheDirectory,
  cachedCore,
  findCore,
  platformName,
  type Surroundings,
} from '../../../src/core-binary/locate.js';

const PIN = { version: '9.8.7', protocol: 1 };

function script(path: string, text = ''): string {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `#!/bin/sh\n${text}`);
  chmodSync(path, 0o755);
  return path;
}

const around = (dir: string, extra: Partial<Surroundings> = {}): Surroundings => ({
  env: {},
  platform: 'linux',
  arch: 'x64',
  home: join(dir, 'home'),
  bundled: join(dir, 'none', 'docattest'),
  ask: () => undefined,
  ...extra,
});

describe('the core lookup', () => {
  it('names platforms the same in every package', () => {
    const names = [
      ['darwin', 'arm64'],
      ['linux', 'x64'],
      ['linux', 'aarch64'],
      ['win32', 'AMD64'],
    ].map(([platform, arch]) => platformName(platform, arch));
    expect(names).toEqual(['darwin-arm64', 'linux-x64', 'linux-arm64', 'windows-x64']);
  });

  it("keeps the cache in the user's cache directory", () => {
    const cache = (env: Record<string, string>, platform: string, home: string) =>
      cacheDirectory({ env, platform, home });
    expect(cache({}, 'darwin', '/Users/u')).toBe('/Users/u/Library/Caches/docattest');
    expect(cache({}, 'linux', '/home/u')).toBe('/home/u/.cache/docattest');
    expect(cache({ XDG_CACHE_HOME: '/x' }, 'linux', '/home/u')).toBe('/x/docattest');
    expect(cache({ XDG_CACHE_HOME: 'relative' }, 'linux', '/home/u')).toBe(
      '/home/u/.cache/docattest',
    );
    const local = 'C:\\Users\\u\\AppData\\Local';
    expect(cache({ LOCALAPPDATA: local }, 'win32', 'C:\\Users\\u')).toBe(`${local}\\docattest`);
    expect(cache({}, 'win32', 'C:\\Users\\u')).toBe(`${local}\\docattest`);
    expect(cache({ DOCATTEST_CACHE: '/mine', XDG_CACHE_HOME: '/x' }, 'linux', '/h')).toBe('/mine');
    const mac = around('/h', {
      env: { DOCATTEST_CACHE: '/mine' },
      platform: 'darwin',
      arch: 'arm64',
    });
    expect(cachedCore(PIN, mac)).toBe('/mine/core/9.8.7/darwin-arm64/docattest');
  });

  it('takes the core it is given, then the bundled one, then the cached one', () => {
    const dir = mkdtempSync(join(tmpdir(), 'docattest-launcher-'));
    const cache = join(dir, 'cache');
    const env = { DOCATTEST_CACHE: cache };
    expect(findCore(PIN, around(dir, { env }))).toEqual({
      error:
        'docattest: no docattest core 9.8.7 for linux-x64. Looked in: DOCATTEST_CORE (not set); ' +
        `this package (${dir}/none/docattest); ` +
        `the shared cache (${cache}/core/9.8.7/linux-x64/docattest)`,
    });
    // Another version in the cache is not this package's core.
    script(join(cache, 'core', '9.8.6', 'linux-x64', 'docattest'));
    expect(findCore(PIN, around(dir, { env }))).toHaveProperty('error');
    const cached = script(join(cache, 'core', '9.8.7', 'linux-x64', 'docattest'));
    expect(findCore(PIN, around(dir, { env }))).toEqual({ core: cached });
    const bundled = script(join(dir, 'pkg', 'bin', 'docattest'));
    expect(findCore(PIN, around(dir, { env, bundled }))).toEqual({ core: bundled });
    // Neither of those is run to find it; a core someone points at is asked.
    const asked: string[] = [];
    const ask = (core: string) => (asked.push(core), '{"version":"1.0.0","protocol":1}');
    const given = { ...env, DOCATTEST_CORE: '/given/core' };
    expect(findCore(PIN, around(dir, { env: given, bundled, ask }))).toEqual({
      core: '/given/core',
    });
    expect(asked).toEqual(['/given/core']);
  });

  it('refuses a core that speaks another protocol, or none', () => {
    const env = { DOCATTEST_CORE: '/given/core' };
    const told = (answer: string | undefined) =>
      findCore(PIN, around('/d', { env, ask: () => answer }));
    expect(told('{"version":"2.0.0","protocol":2}')).toEqual({
      error:
        'docattest: DOCATTEST_CORE (/given/core) is core 2.0.0, which speaks adapter protocol 2; ' +
        'this package speaks protocol 1 (it was built for core 9.8.7)',
    });
    for (const answer of [undefined, '', 'docattest 1.0', '{"version":"1"}', '[1]']) {
      expect(told(answer)).toEqual({
        error:
          'docattest: DOCATTEST_CORE (/given/core) is not a docattest core: ' +
          'it does not answer --version --json',
      });
    }
  });
});

// The built launcher, and what 'the launcher, run' runs it with.
const launcher = fileURLToPath(new URL('../../../dist/launcher/main.js', import.meta.url));
const launchDir = mkdtempSync(join(tmpdir(), 'docattest-launch-'));
// A stand-in core that says what it was given.
const core = script(
  join(launchDir, 'my tools', 'core'),
  'case "$*" in "--version --json") echo \'{"version":"0.0.0","protocol":1}\'; exit 0;; esac\n' +
    'printf "%s\\n" "$DOCATTEST_ADAPTER_TYPESCRIPT" "$DOCATTEST_COMMAND" "$#" "$@"\ncat\nexit 7\n',
);
const launch = (args: string[], env: Record<string, string> = {}, main = launcher) => {
  const clean = Object.fromEntries(
    Object.entries(process.env).filter(([key]) => !key.startsWith('DOCATTEST_')),
  );
  return spawnSync(process.execPath, [main, ...args], {
    env: { ...clean, DOCATTEST_CORE: core, ...env },
    input: 'from stdin',
    encoding: 'utf8',
  });
};

describe('the launcher, run', () => {
  it('hands over the adapter and the command, and passes everything else through', () => {
    const done = launch(['check', 'a b', '--json']);
    expect([done.status, done.stderr]).toEqual([7, '']);
    const [adapter, command, ...rest] = done.stdout.split('\n');
    const main = fileURLToPath(new URL('../../../dist/protocol/main.js', import.meta.url));
    expect(JSON.parse(adapter)).toEqual([process.execPath, main]);
    expect(command).toBe('npx --no docattest');
    expect(rest).toEqual(['3', 'check', 'a b', '--json', 'from stdin']);
  });

  it('leaves alone what is already set', () => {
    const env = { DOCATTEST_ADAPTER_TYPESCRIPT: 'mine', DOCATTEST_COMMAND: 'pnpm docattest' };
    expect(launch(['hooks', 'install'], env).stdout.split('\n').slice(0, 2)).toEqual([
      'mine',
      'pnpm docattest',
    ]);
  });

  it('says which core it runs when a trace is asked for', () => {
    const done = launch(['check'], { DOCATTEST_TRACE: '1' });
    expect([done.status, done.stderr]).toEqual([7, `[trace launcher] core ${core}\n`]);
  });

  it('asks for no trace when the variable is set to nothing', () => {
    const done = launch(['check'], { DOCATTEST_TRACE: '' });
    expect([done.status, done.stderr]).toEqual([7, '']);
  });

  it('names the three places when there is no core, and the package it expected', () => {
    const env = { DOCATTEST_CORE: '', DOCATTEST_CACHE: join(launchDir, 'cache') };
    // The package where no node_modules lies above it, as when npm skipped the platform
    // package: this checkout's own install holds one.
    const alone = join(launchDir, 'alone', 'dist');
    cpSync(fileURLToPath(new URL('../../../dist', import.meta.url)), alone, { recursive: true });
    const done = launch(['check'], env, join(alone, 'launcher', 'main.js'));
    expect(done.status).toBe(1);
    expect(done.stderr.trimEnd().split('\n')).toHaveLength(2);
    expect(done.stderr).toContain('set DOCATTEST_CORE to');
    expect(done.stderr).toMatch(
      /^docattest: no docattest core \d+\.\d+\.\d+ for [a-z0-9]+-[a-z0-9]+\./,
    );
    expect(done.stderr).toContain('DOCATTEST_CORE (not set)');
    expect(done.stderr).toContain('this package (');
    expect(done.stderr).toContain(`the shared cache (${join(launchDir, 'cache', 'core')}`);
  });

  it('pins the core and its platforms the package was built for, from the one source', () => {
    const read = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf8');
    expect(read('../../../dist/core.json')).toBe(read('../../../../../protocol/core.json'));
    expect(read('../../../dist/platforms.json')).toBe(
      read('../../../../../protocol/platforms.json'),
    );
  });
});
