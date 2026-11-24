import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { requireDependency } from '../../../../src/language/node/dependency.js';
import { resolveThroughExports } from '../../../../src/language/node/exports-map.js';
import { nearestPackage, workspacePackages } from '../../../../src/language/surfaces/packages.js';
import { staleCopyWarning } from '../../../../src/language/surfaces/stale.js';
import { diskFiles } from '../../../../src/files/disk.js';

function project(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), 'docattest-node-'));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(root, file, '..'), { recursive: true });
    writeFileSync(join(root, file), text);
  }
  return root;
}

describe('resolveThroughExports', () => {
  it('follows import conditions and subpath patterns', () => {
    const exports = {
      '.': { types: './i.d.ts', import: './i.js' },
      './features/*': { default: './f/*.js' },
    };
    const root = project({
      'node_modules/esm/package.json': JSON.stringify({ name: 'esm', exports }),
    });
    const from = join(root, 'index.js');
    expect(resolveThroughExports('esm', from)).toBe(join(root, 'node_modules/esm/i.js'));
    expect(resolveThroughExports('esm/features/x', from)).toBe(
      join(root, 'node_modules/esm/f/x.js'),
    );
    expect(resolveThroughExports('esm/missing', from)).toBeUndefined();
    expect(resolveThroughExports('absent', from)).toBeUndefined();
  });

  it('prefers the import condition over a require stub and loads it via requireDependency', () => {
    const root = project({
      'node_modules/esm-guarded/package.json': JSON.stringify({
        name: 'esm-guarded',
        type: 'module',
        exports: {
          '.': {
            require: './stub.cjs',
            import: './index.js',
          },
        },
      }),
      'node_modules/esm-guarded/stub.cjs':
        'throw new Error("cannot be imported in a CommonJS module using require()");\n',
      'node_modules/esm-guarded/index.js': 'export const ANSWER = 42;\n',
    });
    const from = join(root, 'entry.js');
    expect(resolveThroughExports('esm-guarded', from)).toBe(
      join(root, 'node_modules/esm-guarded/index.js'),
    );
    expect(requireDependency('esm-guarded', from)).toMatchObject({ ANSWER: 42 });
  });
});

describe('packages', () => {
  const manifest = (name: string, version: string) => JSON.stringify({ name, version });
  const files = {
    'package.json': JSON.stringify({ name: 'repo', workspaces: ['packages/*', '!packages/skip'] }),
    'packages/client/package.json': manifest('@x/client', '3.2.0'),
    'packages/skip/package.json': manifest('skip', '1.0.0'),
    'packages/client/dist/package.json': JSON.stringify({ type: 'module' }),
    'node_modules/@x/client/package.json': manifest('@x/client', '3.0.1'),
    'node_modules/@x/client/index.js': '',
  };

  it('finds the nearest named package and the workspace packages', () => {
    const root = project(files);
    const disk = diskFiles(root);
    expect(nearestPackage(disk, join(root, 'packages/client/dist/index.js'))).toMatchObject({
      name: '@x/client',
      version: '3.2.0',
    });
    expect(workspacePackages(disk).map((p) => p.name)).toEqual(['@x/client']);
  });

  it('warns about an installed copy whose version differs from the workspace package', () => {
    const root = project(files);
    const spec = { name: 'default', import: '@x/client' };
    const warning = staleCopyWarning(
      diskFiles(root),
      spec,
      join(root, 'node_modules/@x/client/index.js'),
    );
    expect(warning).toMatch(
      /resolves to node_modules\/@x\/client \(3\.0\.1\), not the workspace package packages\/client \(3\.2\.0\)/,
    );
  });

  it('stays quiet for a symlink to the workspace package', () => {
    const linked = Object.entries(files).filter(([file]) => !file.startsWith('node_modules/'));
    const root = project(Object.fromEntries(linked));
    mkdirSync(join(root, 'node_modules/@x'), { recursive: true });
    symlinkSync(join(root, 'packages/client'), join(root, 'node_modules/@x/client'), 'junction');
    const spec = { name: 'default', import: '@x/client' };
    expect(
      staleCopyWarning(diskFiles(root), spec, join(root, 'node_modules/@x/client/dist/index.js')),
    ).toBeUndefined();
  });
});
