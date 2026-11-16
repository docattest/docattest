// An in-memory project on disk, for as long as one call of the API needs it: the core reads
// projects from disk, so `files` are written to a temporary directory, and a base given as
// files is committed there with git first.
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import * as path from 'node:path';
import { UserError } from '../api/errors.js';

/** A temporary project directory. */
export interface Workspace {
  root: string;
  /** A project file's contents now, or undefined. */
  read(file: string): string | undefined;
}

// Git as docattest uses it here: no one's settings, hooks or signing; one fixed identity.
const GIT_ENV = {
  ...process.env,
  GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null',
  GIT_CONFIG_NOSYSTEM: '1',
  GIT_AUTHOR_NAME: 'docattest',
  GIT_AUTHOR_EMAIL: 'docattest@example.invalid',
  GIT_COMMITTER_NAME: 'docattest',
  GIT_COMMITTER_EMAIL: 'docattest@example.invalid',
};

function write(root: string, files: Record<string, string>): void {
  for (const [file, text] of Object.entries(files)) {
    const target = path.resolve(root, file);
    const rel = path.relative(root, target);
    if (rel === '' || rel.startsWith('..') || path.isAbsolute(rel)) {
      throw new UserError(`an in-memory project's file "${file}" is not inside the project`);
    }
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, text);
  }
}

function git(root: string, ...args: string[]): void {
  try {
    execFileSync('git', args, { cwd: root, env: GIT_ENV, stdio: ['ignore', 'ignore', 'pipe'] });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    throw new UserError(`a base given as files needs git, which failed: ${message}`);
  }
}

/**
 * Write `files` to a new temporary directory, run `use` there and remove it again. With
 * `base`, those files are committed first (as `HEAD`), and the working tree is then made
 * exactly `files`.
 */
export async function inWorkspace<T>(
  files: Record<string, string>,
  base: Record<string, string> | undefined,
  use: (workspace: Workspace) => Promise<T>,
): Promise<T> {
  const root = realpathSync(mkdtempSync(path.join(tmpdir(), 'docattest-project-')));
  try {
    if (base !== undefined) {
      write(root, base);
      git(root, 'init', '-q');
      git(root, 'add', '-A');
      git(root, 'commit', '-q', '--allow-empty', '--no-verify', '-m', 'base');
      for (const file of Object.keys(base)) rmSync(path.join(root, file), { force: true });
    }
    write(root, files);
    return await use({
      root,
      read: (file) => {
        try {
          return readFileSync(path.join(root, file), 'utf8');
        } catch {
          return undefined;
        }
      },
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
