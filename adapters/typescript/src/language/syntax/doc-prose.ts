import type { DocProse } from '../../api/adapter.js';
import { prose } from './prose.js';

const DOC_COMMENT = /\/\*\*[\s\S]*?\*\//g;

/** The descriptions of the doc comments in a source file: the prose before the first tag. */
export function docCommentProse(text: string): DocProse[] {
  return [...text.matchAll(DOC_COMMENT)].map((match) => {
    const start = lineAt(text, match.index);
    const end = start + match[0].split('\n').length - 1;
    return { text: description(match[0]), start, end };
  });
}

function description(comment: string): string {
  const lines = comment
    .replace(/^\/\*\*/, '')
    .replace(/\*\/$/, '')
    .split('\n')
    .map((l) => l.replace(/^\s*\* ?/, ''));
  const tag = lines.findIndex((l) => /^\s*@\w/.test(l));
  return prose(lines.slice(0, tag < 0 ? lines.length : tag).join(' '));
}

function lineAt(text: string, index: number): number {
  return text.slice(0, index).split('\n').length;
}
