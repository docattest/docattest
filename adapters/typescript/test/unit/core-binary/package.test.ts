// What the npm package tells a user with no core (src/core-binary/package.ts). The Python
// launcher says the same of its wheel (adapters/python/tests/test_launcher.py).
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { findCore } from '../../../src/core-binary/locate.js';
import { detectCommand, remedy } from '../../../src/core-binary/package.js';

const known = { npmPackage: '@scope/core-{platform}', platforms: { 'linux-x64': {} } };

describe('with no core', () => {
  it('names the package it expected, why it may be missing, and the fixes', () => {
    const missing = findCore(
      { version: '9.8.7', protocol: 1 },
      {
        env: { DOCATTEST_CACHE: '/c' },
        platform: 'linux',
        arch: 'x64',
        home: '/h',
        bundled: undefined,
        ask: () => undefined,
        remedy: remedy('9.8.7', 'linux-x64', known),
      },
    );
    expect('error' in missing && missing.error.split('\n')).toEqual([
      'docattest: no docattest core 9.8.7 for linux-x64. Looked in: DOCATTEST_CORE (not set); ' +
        'this package (it brought no binary for this platform); ' +
        'the shared cache (/c/core/9.8.7/linux-x64/docattest)',
      'The core for linux-x64 is the package @scope/core-linux-x64@9.8.7, an optional ' +
        'dependency of docattest, and it is not installed: npm leaves optional dependencies ' +
        'out with --omit=optional (or --no-optional, or omit=optional in .npmrc), and a ' +
        "lockfile written on another platform can leave out this one's. To fix it, install " +
        'again with them (npm install --include=optional), or add it to the project ' +
        '(npm install --save-optional @scope/core-linux-x64@9.8.7), or set DOCATTEST_CORE to ' +
        'a docattest core 9.8.7.',
    ]);
  });

  it('says on a platform with no core to build one', () => {
    expect(remedy('9.8.7', 'linux-ppc64', known)).toBe(
      'docattest brings a core for linux-x64; there is none for linux-ppc64. Build one from ' +
        "docattest's source (cargo build --release in its core/ directory) and set " +
        'DOCATTEST_CORE to it.',
    );
  });
});

describe('detectCommand', () => {
  const withDir = (files: Record<string, string>, fn: (dir: string) => void) => {
    const dir = mkdtempSync(join(tmpdir(), 'docattest-pm-'));
    try {
      for (const [rel, text] of Object.entries(files)) {
        const full = join(dir, rel);
        mkdirSync(join(full, '..'), { recursive: true });
        writeFileSync(full, text);
      }
      fn(dir);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  };

  it('detects pnpm, yarn, bun, and npm from packageManager or lockfiles', () => {
    withDir({ 'pnpm-lock.yaml': '' }, (dir) => {
      expect(detectCommand(dir)).toBe('pnpm exec docattest');
      expect(detectCommand(tmpdir(), ['init', '--root', dir])).toBe('pnpm exec docattest');
    });
    withDir({ 'package.json': JSON.stringify({ packageManager: 'pnpm@10.28.1' }) }, (dir) => {
      expect(detectCommand(dir)).toBe('pnpm exec docattest');
    });
    withDir({ 'yarn.lock': '' }, (dir) => {
      expect(detectCommand(dir)).toBe('yarn docattest');
    });
    withDir({ 'bun.lock': '' }, (dir) => {
      expect(detectCommand(dir)).toBe('bunx docattest');
    });
    withDir({ 'package-lock.json': '' }, (dir) => {
      expect(detectCommand(dir)).toBe('npx --no docattest');
    });
  });
});
