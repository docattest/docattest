import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { diskFiles } from '../../../src/files/disk.js';
import { ignoreRule } from '../../../src/files/ignored.js';
import { globMatcher, isGlob, sourceFileMatcher } from '../../../src/files/glob.js';
import { MEMORY_ROOT, memoryFiles } from '../../../src/files/memory.js';
import { normalise } from '../../../src/files/source.js';

describe('globMatcher', () => {
  it('matches dotfiles and strips ./', () => {
    const match = globMatcher(['./docs/**/*.md']);
    expect(match('docs/a/b.md')).toBe(true);
    expect(match('docs/.hidden/c.md')).toBe(true);
    expect(match('src/a.md')).toBe(false);
    expect(globMatcher([])('anything')).toBe(false);
  });

  it("reads a language's source files as include globs minus exclude globs", () => {
    const source = sourceFileMatcher({ include: ['**/*.py', 'bin/*'], exclude: ['**/*_pb2.py'] });
    expect(['a.py', 'pkg/.venv/b.py', '.c.py', 'bin/run'].map(source)).toEqual(Array(4).fill(true));
    // Whole path, case as written; an exclude wins; no include, no source files.
    expect(['a.PY', 'bin/x/run', 'pkg/a_pb2.py', 'a.pyc'].map(source)).toEqual(
      Array(4).fill(false),
    );
    expect(sourceFileMatcher({ include: [], exclude: [] })('a.py')).toBe(false);
  });

  it('tells globs from paths', () => {
    expect(isGlob('docs/*.md')).toBe(true);
    expect(isGlob('docs/[ab].md')).toBe(true);
    expect(isGlob('docs/a.md')).toBe(false);
  });
});

describe('memoryFiles', () => {
  it('normalises paths and sees only its own files', () => {
    const files = memoryFiles({ './docs\\a.md': 'A', 'docs/.snap/x.json': '{}' });
    expect(files.list()).toEqual(['docs/.snap/x.json', 'docs/a.md']);
    expect(files.read('docs//a.md')).toBe('A');
    expect(files.readAbsolute(join(MEMORY_ROOT, 'docs/a.md'))).toBe('A');
    expect(files.existsAbsolute('/etc/hosts')).toBe(false);
  });
});

describe('diskFiles', () => {
  it('walks the project, skipping hidden directories and the names it is told to ignore', () => {
    const root = mkdtempSync(join(tmpdir(), 'docattest-files-'));
    for (const file of [
      'docs/a.md',
      'node_modules/x/b.md',
      '.git/c.md',
      'docs/.snapshots/s.json',
    ]) {
      mkdirSync(join(root, file, '..'), { recursive: true });
      writeFileSync(join(root, file), file);
    }
    // Which names are never the project's own is the language's to say.
    expect(diskFiles(root).list()).toEqual(['docs/a.md', 'node_modules/x/b.md']);
    const ignoring = (ignoredNames: string[]) => diskFiles(root, ignoreRule({ ignoredNames }));
    const files = ignoring(['node_modules']);
    expect(files.list()).toEqual(['docs/a.md']);
    expect(ignoring(['docs', 'x']).list()).toEqual([]);
    // A name is a glob for one segment, never for a path.
    expect(ignoring(['node_*', '*.md']).list()).toEqual([]);
    expect(ignoring(['docs/a.md', 'x/b.md']).list()).toHaveLength(2);
    expect(files.exists('docs')).toBe(false);
    expect(files.read('missing.md')).toBeUndefined();
  });
});

describe('ignoreRule', () => {
  it('ignores an entry by its name, a glob for one segment', () => {
    const ignored = ignoreRule({ ignoredNames: ['deps', '*.egg-info'] });
    for (const name of ['deps', 'x.egg-info', '.egg-info']) expect(ignored(name), name).toBe(true);
    for (const name of ['deps.md', 'egg-info', 'x.egg-infos']) {
      expect(ignored(name), name).toBe(false);
    }
  });
});

describe('normalise', () => {
  it('canonicalises separators', () => {
    expect(normalise('.\\a\\\\b/c')).toBe('a/b/c');
  });
});
