import * as fs from 'node:fs';
import * as path from 'node:path';
import { messageOf, UserError } from '../api/errors.js';
import { NOTHING_IGNORED, type Ignored } from './ignored.js';
import type { FileSource } from './source.js';

/** A project in a directory on disk. */
export function diskFiles(root: string, ignored: Ignored = NOTHING_IGNORED): FileSource {
  const abs = path.resolve(root);
  const at = (file: string) => path.join(abs, file);
  return {
    root: abs,
    memory: false,
    list: () => walk(abs, '', ignored),
    read: (file) => readText(at(file)),
    exists: (file) => isFile(at(file)),
    readAbsolute: readText,
    existsAbsolute: isFile,
  };
}

/** Why a read finds nothing there: no such file, or a directory where a file was expected. */
const ABSENT = new Set(['ENOENT', 'ENOTDIR', 'EISDIR']);

/** The file's text; undefined when it is not there. Any other failure is the user's to fix. */
function readText(file: string): string | undefined {
  try {
    return fs.readFileSync(file, 'utf8');
  } catch (err) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code !== undefined && ABSENT.has(code)) return undefined;
    throw new UserError(`cannot read ${file}: ${messageOf(err)}`);
  }
}

function isFile(file: string): boolean {
  try {
    return fs.statSync(file).isFile();
  } catch {
    return false;
  }
}

function entries(dir: string): fs.Dirent[] {
  try {
    return fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return [];
  }
}

// Tool state (.git, .docattest, …) is never a page or a source, and neither is anything the
// language ignores (its dependencies, its build output), by its name. The root is the project
// whatever it holds.
function walk(root: string, rel: string, ignored: Ignored): string[] {
  const out: string[] = [];
  const found = entries(path.join(root, rel));
  for (const entry of found) {
    const skip = ignored(entry.name) || (entry.isDirectory() && entry.name.startsWith('.'));
    if (skip) continue;
    const child = rel ? `${rel}/${entry.name}` : entry.name;
    if (entry.isDirectory()) out.push(...walk(root, child, ignored));
    else if (entry.isFile()) out.push(child);
  }
  return out.sort();
}
