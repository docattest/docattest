#!/usr/bin/env node
// Check every relative link in the repository's Markdown: the file or directory it names must
// exist, and a `#fragment` into a Markdown file must name one of its headings (as GitHub
// writes their anchors). Links inside code are not links; links with a scheme are not checked.
//
//   node scripts/links.ts         (npm run links)
//
// Prints each broken link with its file and line, and exits 1 when there is one.
import process from 'node:process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { ROOT } from './lib/repository.ts';

/** Not the repository's own Markdown: installed, built, vendored, or recorded output. */
const SKIPPED = new Set<string>([
  '.git',
  'node_modules',
  'core/target',
  'core/vendor',
  'adapters/typescript/dist',
  'tests/parity/recordings',
]);

/** Every Markdown file below `dir`, relative to the root. */
function markdownFiles(dir = ROOT): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    const rel = relative(ROOT, path);
    if (SKIPPED.has(rel) || SKIPPED.has(entry.name)) return [];
    if (entry.isDirectory()) return markdownFiles(path);
    return entry.name.endsWith('.md') ? [rel] : [];
  });
}

/** The lines of a page that are prose: fenced code is blanked, and so are code spans. */
function proseLines(text: string): string[] {
  let fence: string | undefined;
  return text.split('\n').map((line) => {
    const marker = /^\s*(`{3,}|~{3,})/.exec(line)?.[1];
    if (fence !== undefined) {
      if (marker?.startsWith(fence.charAt(0)) && marker.length >= fence.length) {
        fence = undefined;
      }
      return '';
    }
    if (marker !== undefined) {
      fence = marker;
      return '';
    }
    return line.replace(/(`+)[^`]*?\1/g, '');
  });
}

/** GitHub's anchor for a heading's text. */
function slug(heading: string): string {
  const text = heading
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/<[^>]+>/g, '')
    .trim()
    .toLowerCase();
  return text.replace(/[^\p{L}\p{N}\s_-]/gu, '').replace(/\s/g, '-');
}

const anchorCache = new Map<string, Set<string>>();

/** The anchors of a Markdown file's headings, with GitHub's suffixes for repeats. */
function anchors(file: string): Set<string> {
  const cached = anchorCache.get(file);
  if (cached) return cached;
  const seen = new Map<string, number>();
  const found = new Set<string>();
  const text = readFileSync(file, 'utf8');
  const raw = text.split('\n');
  proseLines(text).forEach((line, k) => {
    const heading = /^ {0,3}#{1,6}\s+(.*?)(\s+#+)?\s*$/.exec(line && (raw[k] ?? ''));
    if (!heading) return;
    const base = slug(heading[1] ?? '');
    const count = seen.get(base) ?? 0;
    seen.set(base, count + 1);
    found.add(count === 0 ? base : `${base}-${count}`);
  });
  anchorCache.set(file, found);
  return found;
}

/** The link targets on one line: inline links and images, and reference definitions. */
function targets(line: string): string[] {
  const inline = [...line.matchAll(/\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g)].map(
    (m) => m[1] ?? '',
  );
  const definition = /^\s{0,3}\[[^\]]+\]:\s*<?(\S+?)>?(\s|$)/.exec(line)?.[1];
  return definition ? [...inline, definition] : inline;
}

/** Why a link from `file` to `target` is broken, or undefined when it is not. */
function problem(file: string, target: string): string | undefined {
  if (/^[a-z][a-z0-9+.-]*:/i.test(target)) return undefined;
  const [path = '', fragment] = target.split('#');
  const absolute = path ? resolve(dirname(join(ROOT, file)), decodeURI(path)) : join(ROOT, file);
  if (!existsSync(absolute)) return 'no such file';
  if (fragment === undefined || !absolute.endsWith('.md') || statSync(absolute).isDirectory()) {
    return undefined;
  }
  return anchors(absolute).has(decodeURI(fragment).toLowerCase()) ? undefined : 'no such heading';
}

const broken: string[] = [];
let checked = 0;
const files = markdownFiles();
for (const file of files) {
  proseLines(readFileSync(join(ROOT, file), 'utf8')).forEach((line, k) => {
    for (const target of targets(line)) {
      if (/^[a-z][a-z0-9+.-]*:/i.test(target)) continue;
      checked++;
      const why = problem(file, target);
      if (why) broken.push(`${file}:${k + 1}: ${target} (${why})`);
    }
  });
}
for (const line of broken) process.stdout.write(`${line}\n`);
process.stdout.write(
  `links: ${checked} relative links in ${files.length} Markdown files, ${broken.length} broken\n`,
);
process.exit(broken.length > 0 ? 1 : 0);
