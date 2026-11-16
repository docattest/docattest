import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { UserError } from '../../../../src/api/errors.js';
import { diskFiles } from '../../../../src/files/disk.js';
import { workspacePackages } from '../../../../src/language/surfaces/packages.js';

/** A project on disk holding `files`. */
function project(files: Record<string, string>) {
  const root = mkdtempSync(join(tmpdir(), 'docattest-packages-'));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(root, file, '..'), { recursive: true });
    writeFileSync(join(root, file), text);
  }
  return diskFiles(root);
}

describe('workspacePackages', () => {
  it('lists the packages the workspaces declare', () => {
    const files = project({
      'package.json': JSON.stringify({ workspaces: ['packages/*'] }),
      'packages/a/package.json': JSON.stringify({ name: 'a', version: '1.0.0' }),
    });
    expect(workspacePackages(files).map((p) => [p.name, p.version])).toEqual([['a', '1.0.0']]);
  });

  it('refuses a project file that does not parse, rather than reading it as no workspaces', () => {
    const broken = (files: Record<string, string>) => () => workspacePackages(project(files));
    expect(broken({ 'package.json': '{ nope' })).toThrow(UserError);
    expect(broken({ 'package.json': '{ nope' })).toThrow(/package\.json cannot be read/);
    expect(broken({ 'package.json': '[]' })).toThrow(/package\.json does not hold an object/);
    expect(broken({ 'pnpm-workspace.yaml': 'packages: [' })).toThrow(
      /pnpm-workspace\.yaml cannot be read/,
    );
  });
});
