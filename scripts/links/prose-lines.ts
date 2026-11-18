// The prose of a Markdown page, line by line, as the link checker reads it: fenced code and
// code spans are not prose, so a link inside them is not a link.

/** The lines of a page that are prose: fenced code is blanked, and so are code spans. */
export function proseLines(text: string): string[] {
  let open: Fence | undefined;
  return text.split('\n').map((line) => {
    const fence = fenceOn(line);
    if (open !== undefined) {
      if (fence !== undefined && closes(fence, open)) open = undefined;
      return '';
    }
    if (fence !== undefined && opens(fence)) {
      open = fence;
      return '';
    }
    return line.replace(CODE_SPAN, '');
  });
}

// CommonMark: a code span opens at a run of backticks and closes at the next run of as many,
// so a span of four may hold a run of three.
const CODE_SPAN = /(?<!`)(`+)(?!`).*?(?<!`)\1(?!`)/g;

/** A run of three or more backticks or tildes that starts a line, and what follows it. */
interface Fence {
  marker: string;
  info: string;
}

// Any indentation, since a fence in a list item is indented by the item.
const FENCE = /^\s*(`{3,}|~{3,})(.*)$/;

function fenceOn(line: string): Fence | undefined {
  const match = FENCE.exec(line);
  return match ? { marker: match[1] ?? '', info: match[2] ?? '' } : undefined;
}

// CommonMark: a closing fence is of the opening fence's character, at least as long, and
// followed by nothing but spaces.
function closes(fence: Fence, open: Fence): boolean {
  return (
    fence.marker.startsWith(open.marker.charAt(0)) &&
    fence.marker.length >= open.marker.length &&
    fence.info.trim() === ''
  );
}

// CommonMark: the info string of a backtick fence cannot hold a backtick, so a line that
// starts with a code span of three backticks (```` ```python ``` ````) opens no fence.
function opens({ marker, info }: Fence): boolean {
  return !(marker.startsWith('`') && info.includes('`'));
}
