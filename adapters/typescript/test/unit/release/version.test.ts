// One version for everything a release ships (scripts/version.ts): the check fails when any
// place disagrees, and `set` writes every place.
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const ROOT = fileURLToPath(new URL('../../../../..', import.meta.url));
const SCRIPT = join(ROOT, 'scripts', 'version.ts');
const FILES = [
  'core/Cargo.toml',
  'core/Cargo.lock',
  'adapters/rust/Cargo.toml',
  'adapters/rust/Cargo.lock',
  'adapters/python/pyproject.toml',
  'adapters/python/docattest_python/__init__.py',
  'protocol/core.json',
  'core/protocol/core.json',
  'adapters/go/internal/protocoldata/protocol/core.json',
  'protocol/platforms.json',
  'adapters/typescript/package.json',
  'package-lock.json',
];

const versions = (...args: string[]) =>
  spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8' });

function copy(): string {
  const root = mkdtempSync(join(tmpdir(), 'docattest-version-'));
  for (const file of FILES) cpSync(join(ROOT, file), join(root, file));
  return root;
}

describe('the version of a release', () => {
  it('is the same everywhere in this repository', () => {
    const done = versions('check');
    expect([done.status, done.stderr]).toEqual([0, '']);
    expect(done.stdout).toMatch(/^version \d+\.\d+\.\d+\S* in all 28 places\n$/);
  });

  it('fails when one place disagrees, and names it', () => {
    const root = copy();
    const pyproject = join(root, 'adapters/python/pyproject.toml');
    const text = readFileSync(pyproject, 'utf8');
    writeFileSync(pyproject, text.replace(/^version = "[^"]+"/m, 'version = "0.0.1"'));
    const done = versions('check', '--root', root);
    expect(done.status).toBe(1);
    expect(done.stderr).toContain('adapters/python/pyproject.toml version: 0.0.1');
  });

  it('fails when a platform package is missing from the npm package', () => {
    const root = copy();
    const file = join(root, 'adapters/typescript/package.json');
    const manifest = JSON.parse(readFileSync(file, 'utf8')) as Record<string, unknown>;
    writeFileSync(file, JSON.stringify({ ...manifest, optionalDependencies: {} }));
    const done = versions('check', '--root', root);
    expect(done.status).toBe(1);
    expect(done.stderr).toMatch(/package\.json optionalDependencies\["[^"]+core-linux-x64"\]/);
  });

  it('is written everywhere by one command, and held to the tag of a release', () => {
    const root = copy();
    const set = versions('set', '9.8.7', '--root', root);
    expect([set.status, set.stdout]).toEqual([0, 'version 9.8.7 in all 28 places\n']);
    expect(readFileSync(join(root, 'core/Cargo.toml'), 'utf8')).toContain('version = "9.8.7"');
    expect(versions('check', '--root', root, '--tag', 'v9.8.7').status).toBe(0);
    const tagged = versions('check', '--root', root, '--tag', 'v9.8.6');
    expect([tagged.status, tagged.stderr]).toEqual([1, expect.stringContaining('expected 9.8.6')]);
  });
});
