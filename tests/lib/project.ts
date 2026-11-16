// What the suites in tests/ share to set up a project on disk: its files, and a git that
// neither reads the user's configuration nor varies from run to run.
import process from 'node:process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

/**
 * A project's files by path: their text, their bytes (for text that is not UTF-8), or
 * `null` for a file to remove. `ProjectFiles<string>` is a project written as text only.
 */
export type ProjectFiles<Content extends string | Buffer = string | Buffer> = Readonly<
  Record<string, Content | null>
>;

/** Write `files` below `root`, making directories as needed; `null` removes a file. */
export function writeFiles(root: string, files: ProjectFiles): void {
  for (const [path, text] of Object.entries(files)) {
    const target = join(root, path);
    if (text === null) {
      rmSync(target, { force: true });
      continue;
    }
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, text);
  }
}

/**
 * The environment of this process, with git committing as `who` and reading no
 * configuration but the repository's own.
 */
export function gitEnv(who: string): NodeJS.ProcessEnv {
  return {
    ...process.env,
    GIT_CONFIG_GLOBAL: '/dev/null',
    GIT_CONFIG_SYSTEM: '/dev/null',
    GIT_AUTHOR_NAME: who,
    GIT_AUTHOR_EMAIL: `${who}@example.invalid`,
    GIT_COMMITTER_NAME: who,
    GIT_COMMITTER_EMAIL: `${who}@example.invalid`,
  };
}

/** Fixed, so that commits, and anything that names one, are the same in every run. */
export const FIXED_GIT_DATES = {
  GIT_AUTHOR_DATE: '2024-01-01T00:00:00Z',
  GIT_COMMITTER_DATE: '2024-01-01T00:00:00Z',
} as const;
