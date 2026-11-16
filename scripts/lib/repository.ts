// The repository the tools work on: where it is, its JSON files, and which files are its own.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, URL } from 'node:url';

/** The root of the repository. */
export const ROOT = fileURLToPath(new URL('../..', import.meta.url));

/** A JSON file of the repository (or of `root`), parsed; its reader says what it holds. */
export function readJson(file: string, root = ROOT): unknown {
  return JSON.parse(readFileSync(join(root, file), 'utf8'));
}

/**
 * The repository's own files, relative to ROOT and sorted: what git tracks, and new files it
 * does not ignore, that are still on disk.
 */
export function repositoryFiles(): string[] {
  const listed = execFileSync(
    'git',
    ['ls-files', '-z', '--cached', '--others', '--exclude-standard'],
    { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
  );
  return [...new Set(listed.split('\0'))]
    .filter((file) => file && existsSync(join(ROOT, file)))
    .sort((a, b) => a.localeCompare(b));
}
