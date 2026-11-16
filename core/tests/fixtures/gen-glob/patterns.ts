// Random patterns: over an alphabet of glob syntax, over regex syntax, and of realistic shapes.
import { int, pick, rand } from './random.ts';

const alphabet: readonly string[] = [
  'a',
  'b',
  'c',
  'x',
  'é',
  '😀',
  'A',
  '1',
  '_',
  ' ',
  '-',
  '.',
  '..',
  '/',
  '//',
  './',
  '*',
  '**',
  '?',
  '[',
  ']',
  '[!',
  '[^',
  '!',
  '^',
  '(',
  ')',
  '|',
  '{',
  '}',
  ',',
  '@',
  '+',
  '\\',
  '\\\\',
  '$',
  '"',
  ':',
  '=',
  '<',
  '>',
  '#',
  '~',
  ';',
  '\n',
  '\u0000',
  '[:alpha:]',
  '[:digit:]',
  '\\d',
  '\\w',
  '\\b',
  '\\1',
  '\\k<n>',
  '(?',
  '(?:',
  '(?=',
  '(?!',
  '(?<',
  '(?<=',
  '(?<!',
  '(?<n>',
  'k',
  'u',
  '0',
  '{1,2}',
  '{1..3}',
  '\\x41',
  '\\u00e9',
  '\\c',
  '@(',
  '!(',
  '+(',
  '*(',
  '?(',
  '.md',
  '.ts',
  'docs',
  'src',
  '**/',
  '/**',
  '*.md',
  '{a,b}',
  '%',
  '&',
];

// Regex syntax that reaches the RegExp through picomatch, with paths over a tiny alphabet so
// that matches are frequent.
const regexAlphabet: readonly string[] = [
  'a',
  'b',
  'a',
  'b',
  '/',
  '.',
  '(',
  ')',
  '|',
  '?',
  '*',
  '+',
  '\\1',
  '\\2',
  '\\k<n>',
  '(?:',
  '(?=',
  '(?!',
  '(?<=',
  '(?<!',
  '(?<n>',
  '{1,2}',
  '{2}',
  '{0}',
  '{,1}',
  '[ab]',
  '[^a]',
  '[a-b]',
  '\\b',
  '\\B',
  '\\w',
  '\\W',
  '\\s',
  '\\d',
  '$',
  '^',
  '@(',
  '+(',
  '*(',
  '!(',
  '?(',
  '"',
  '\\x61',
  '\\u0062',
  '\\c',
  '\\0',
  '\\8',
  ',',
  '{',
  '}',
  '[',
  ']',
  '-',
  '\\',
];
export const regexPattern = (): string => {
  const n = 2 + int(8);
  let s = '';
  for (let i = 0; i < n; i++) s += pick(regexAlphabet);
  return s;
};
export const regexPaths = (): string[] => {
  const out: string[] = [];
  for (let i = 0; i < 10; i++) {
    let p = '';
    const n = 1 + int(5);
    for (let k = 0; k < n; k++) {
      p += pick(['a', 'b', 'a', 'b', 'ab', '/', '.', ' ', '\n', '1', '_']);
    }
    out.push(p);
  }
  return out;
};

export const randomPattern = (): string => {
  const n = 1 + int(8);
  let s = '';
  for (let i = 0; i < n; i++) s += pick(alphabet);
  return s;
};

// Patterns weighted toward realistic shapes.
const segments: readonly string[] = [
  'docs',
  'src',
  'a',
  'b',
  '.github',
  'deep',
  'é',
  '😀',
  'x.md',
  'README.md',
  '*',
  '**',
  '?',
  '*.md',
  '*.ts',
  '{a,b}',
  '[ab]',
  '@(a|b)',
  '!(a)',
  '.*',
  '*.{md,ts}',
  '[[:alpha:]]*',
  '+(a)',
  '\\*',
  '..',
  '.',
];
export const realisticPattern = (): string => {
  const n = 1 + int(4);
  const parts: string[] = [];
  for (let i = 0; i < n; i++) parts.push(pick(segments));
  let s = parts.join('/');
  if (rand() < 0.1) s = `./${s}`;
  if (rand() < 0.1) s = `!${s}`;
  if (rand() < 0.05) s += '/';
  return s;
};
