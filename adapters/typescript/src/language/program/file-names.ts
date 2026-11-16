import * as path from 'node:path';

// TypeScript names every file with `/`, on Windows too, while `path.join` writes the
// platform's separator. A path made here and a name TypeScript hands back only compare
// equal once they are written the same way; these two functions are where that happens.

/** A platform path as TypeScript names the file. */
export function typeScriptName(file: string): string {
  return path.sep === '\\' ? file.replaceAll('\\', '/') : file;
}

/** A file TypeScript names, as the platform writes its path. */
export function platformPath(name: string): string {
  return path.sep === '\\' ? name.replaceAll('/', '\\') : name;
}
