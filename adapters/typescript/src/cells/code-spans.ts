import { LinkOpeners } from './link-openers.js';
import { rawLength } from './raw-html.js';

/** What a backslash escapes, as CommonMark has it: ASCII punctuation. */
const ESCAPABLE = /[!-/:-@[-`{-~]/;

/**
 * The content of every code span in a cell of a cases table, in order, as a fact over the
 * cell's column compares it. A runner gets each cell as the page writes it, code spans with
 * their backticks; this gives what docattest itself reads. A span opens with a run of
 * backticks and closes at the next run of as many, and the white space around its content is
 * dropped. As CommonMark has it, a backtick escaped with a backslash opens nothing, and the
 * backticks of an HTML tag or comment, of an autolink, or of a link's destination or title
 * belong to those. A cell with no code span gives none.
 *
 * @example
 * // check
 * import { codeSpans } from 'docattest';
 *
 * assert.deepEqual(codeSpans('`FOO=1` `BAR=2`'), ['FOO=1', 'BAR=2']);
 * assert.deepEqual(codeSpans('``a`b`` or `` `c` ``'), ['a`b', '`c`']);
 * assert.deepEqual(codeSpans('no code'), []);
 */
export function codeSpans(cell: string): string[] {
  const spans: string[] = [];
  const openers = new LinkOpeners();
  for (let at = 0; at < cell.length;) {
    const { length, span } = token(cell, at, openers);
    if (span !== undefined) spans.push(span);
    at += length;
  }
  return spans;
}

/** One piece of a cell: its length, and the content of the code span it is, if it is one. */
interface Token {
  length: number;
  span?: string;
}

function token(cell: string, at: number, openers: LinkOpeners): Token {
  const char = cell.charAt(at);
  if (char === '\\') return { length: ESCAPABLE.test(cell.charAt(at + 1)) ? 2 : 1 };
  if (char === '<') return { length: Math.max(1, rawLength(cell.slice(at))) };
  if (cell.startsWith('![', at)) {
    openers.open('image');
    return { length: 2 };
  }
  if (char === '[') {
    openers.open('link');
    return { length: 1 };
  }
  if (char === ']') return { length: 1 + openers.close(cell.slice(at + 1)) };
  return char === '`' ? codeSpan(cell, at) : { length: 1 };
}

// The code span that starts at a backtick. When no run of as many backticks closes it, the
// opening run is only text.
function codeSpan(cell: string, at: number): Token {
  const fence = runLength(cell, at);
  const end = closingRun(cell, at + fence, fence);
  if (end === undefined) return { length: fence };
  return { length: end + fence - at, span: cell.slice(at + fence, end).trim() };
}

function runLength(text: string, at: number): number {
  let end = at;
  while (text.charAt(end) === '`') end++;
  return end - at;
}

// Where the first run of exactly `fence` backticks from `from` starts. A longer or shorter
// run is part of the span's content.
function closingRun(text: string, from: number, fence: number): number | undefined {
  for (let at = from; at < text.length;) {
    if (text.charAt(at) !== '`') {
      at++;
      continue;
    }
    const run = runLength(text, at);
    if (run === fence) return at;
    at += run;
  }
  return undefined;
}
