// Where a project's operations run. A project on disk is used where it is; an in-memory one
// is written to a temporary directory for each operation and removed afterwards, and what
// `refresh` writes there is kept in memory for the next.
import * as path from 'node:path';
import { UserError } from '../api/errors.js';
import type { ProjectInput } from '../api/project.js';
import { inWorkspace, type Workspace } from './workspace.js';

export type { Workspace };

/** A project's place. */
export interface Place {
  /** Whether the project is in memory. */
  memory: boolean;
  /** Run `use` with the project on disk; `base`, files to commit first (memory only). */
  run<T>(base: Record<string, string> | undefined, use: (at: Workspace) => Promise<T>): Promise<T>;
  /** Keep a file an operation wrote. */
  keep(file: string, text: string | undefined): void;
}

/** The place of the project `input` describes. */
export function placeOf(input: ProjectInput): Place {
  const given = (input.root === undefined ? 0 : 1) + (input.files === undefined ? 0 : 1);
  if (given !== 1) throw new UserError('createProject needs exactly one of `root` or `files`');
  if (input.files === undefined) return onDisk(path.resolve(input.root ?? '.'));
  const files = { ...input.files };
  return {
    memory: true,
    run: (base, use) => inWorkspace(files, base, use),
    keep: (file, text) => {
      if (text !== undefined) files[file] = text;
    },
  };
}

function onDisk(root: string): Place {
  return {
    memory: false,
    run: (base, use) => {
      if (base !== undefined) {
        throw new UserError(
          'a base given as files needs an in-memory project (`files`); a project on disk ' +
            'compares with a git ref',
        );
      }
      return use({ root, read: () => undefined });
    },
    keep: () => undefined,
  };
}
