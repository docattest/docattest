import { mkdirSync, mkdtempSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { diskFiles } from '../../../../src/files/disk.js';
import { resolveSurfaces } from '../../../../src/language/surfaces/resolve.js';
import { workspaceBuildWarnings } from '../../../../src/language/surfaces/workspace-builds.js';

const LONG_AGO = 1_700_000_000;

/** A workspace on disk: `files`, plus `links` as node_modules path → package directory. */
function workspace(files: Record<string, string>, links: Record<string, string>) {
  const root = mkdtempSync(join(tmpdir(), 'docattest-builds-'));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(root, file, '..'), { recursive: true });
    writeFileSync(join(root, file), text);
  }
  for (const [link, target] of Object.entries(links)) {
    mkdirSync(join(root, link, '..'), { recursive: true });
    symlinkSync(join(root, target), join(root, link), 'junction');
  }
  return root;
}

function warningsFor(root: string, entry: string): string[] {
  const files = diskFiles(root);
  const surfaces = resolveSurfaces(files, [{ name: 'default', import: '@acme/kit', path: entry }]);
  return surfaces.flatMap(workspaceBuildWarnings(files));
}

const manifest = (name: string, main: string) => JSON.stringify({ name, type: 'module', main });

describe('workspaceBuildWarnings', () => {
  it('follows a package linked in its importer’s own node_modules, and on through its build', () => {
    const root = workspace(
      {
        'packages/kit/src/index.ts': "export * from './levels.js';",
        'packages/kit/src/levels.ts': "export { LEVELS } from '@acme/types';",
        'packages/types/package.json': manifest('@acme/types', 'dist/index.js'),
        'packages/types/src/index.ts': "export { LEVELS } from '@acme/base';",
        'packages/types/dist/index.js': "export { LEVELS } from '@acme/base';",
        'packages/base/package.json': manifest('@acme/base', 'dist/index.js'),
        'packages/base/src/index.ts': 'export const LEVELS = [1, 2];',
        'packages/base/dist/index.js': 'export const LEVELS = [1];',
      },
      {
        'packages/kit/node_modules/@acme/types': 'packages/types',
        'packages/types/node_modules/@acme/base': 'packages/base',
      },
    );
    expect(warningsFor(root, 'packages/kit/src/index.ts')).toEqual([]);

    utimesSync(join(root, 'packages/base/dist/index.js'), LONG_AGO, LONG_AGO);
    expect(warningsFor(root, 'packages/kit/src/index.ts')).toEqual([
      'the workspace package @acme/base (packages/base) is loaded from its build ' +
        'packages/base/dist, which is older than packages/base/src/index.ts: rebuild @acme/base',
    ]);
  });

  it('takes a build record newer than the sources as a current build', () => {
    // An incremental `tsc --build` over touched but unchanged sources rewrites only its
    // build record, so the outputs keep their older times.
    const root = workspace(
      {
        'packages/kit/src/index.ts': "export { LEVELS } from '@acme/types';",
        'packages/types/package.json': manifest('@acme/types', 'dist/index.js'),
        'packages/types/src/index.ts': 'export const LEVELS = [1];',
        'packages/types/dist/index.js': 'export const LEVELS = [1];',
        'packages/types/tsconfig.tsbuildinfo': '{}',
      },
      { 'packages/kit/node_modules/@acme/types': 'packages/types' },
    );
    utimesSync(join(root, 'packages/types/dist/index.js'), LONG_AGO, LONG_AGO);
    expect(warningsFor(root, 'packages/kit/src/index.ts')).toEqual([]);
  });

  it('says nothing of a package the runtime loads from its sources, or of one not linked', () => {
    const root = workspace(
      {
        'packages/kit/src/index.ts': "export * from '@acme/types';\nexport * from '@acme/old';",
        'packages/types/package.json': manifest('@acme/types', 'src/index.ts'),
        'packages/types/src/index.ts': 'export const LEVELS = [1];',
        'node_modules/@acme/old/package.json': manifest('@acme/old', 'missing.js'),
      },
      { 'node_modules/@acme/types': 'packages/types' },
    );
    expect(warningsFor(root, 'packages/kit/src/index.ts')).toEqual([]);
  });
});
