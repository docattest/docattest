/** Expected stdout declared by a trailing `// Output:` or `// Unordered output:` comment. */
export interface ExpectedOutput {
  readonly text: string;
  readonly unordered: boolean;
}

const OUTPUT_HEADER = /^(Unordered output|Output):(.*)$/i;

/**
 * Extract the trailing `// Output:` or `// Unordered output:` comment block from code,
 * matching Go's testable example convention. Returns undefined when no such comment sits
 * at the end of the block.
 */
export function expectedOutput(code: string): ExpectedOutput | undefined {
  const comments = trailingComments(code);
  for (let idx = 0; idx < comments.length; idx += 1) {
    const match = OUTPUT_HEADER.exec(comments[idx] ?? '');
    if (match) return buildExpected(match, comments.slice(idx + 1));
  }
  return undefined;
}

function trailingComments(code: string): string[] {
  const lines = code.replace(/\r\n?/g, '\n').split('\n');
  let end = lines.length;
  while (end > 0 && lines[end - 1]?.trim() === '') end -= 1;
  let start = end;
  while (start > 0 && lines[start - 1]?.trimStart().startsWith('//')) start -= 1;
  return lines.slice(start, end).map(stripCommentPrefix);
}

function buildExpected(match: RegExpExecArray, rest: string[]): ExpectedOutput {
  const unordered = (match[1] ?? '').toLowerCase().startsWith('unordered');
  const first = (match[2] ?? '').replace(/^ /, '');
  const body = first.length > 0 ? [first, ...rest] : rest;
  return { text: body.join('\n'), unordered };
}

function stripCommentPrefix(line: string): string {
  const body = line.trimStart().slice(2);
  return body.startsWith(' ') ? body.slice(1) : body;
}

/** Outcome of comparing actual stdout against an {@link ExpectedOutput}. */
export interface OutputComparison {
  readonly status: 'pass' | 'fail';
  readonly message: string;
  readonly expected?: string;
  readonly actual?: string;
}

/** Compare captured stdout with what an `// Output:` comment states. */
export function compareOutput(got: string, want: ExpectedOutput): OutputComparison {
  let actual = got.replace(/\r\n?/g, '\n').trim();
  let expected = want.text.trim();
  if (want.unordered) {
    actual = sortLines(actual);
    expected = sortLines(expected);
  }
  if (actual === expected) {
    return { status: 'pass', message: 'printed the output it states' };
  }
  return {
    status: 'fail',
    message: `expected output ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
    expected,
    actual,
  };
}

function sortLines(text: string): string {
  return text.split('\n').sort().join('\n');
}
