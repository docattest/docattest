// What makes a bracketed text a link: `(destination "title")`, either part optional, as
// CommonMark has it. Its backticks are the link's, not a code span's.
const SPACE = '[ \\t\\n]';
const ESCAPED = '\\\\[!-/:-@[-`{-~]|\\\\';
// A character of a destination that is not in angle brackets: no space, control or paren.
const PLAIN = '[^\\x00-\\x20()\\\\]';
const DESTINATION =
  `<(?:[^<>\\n\\\\]|${ESCAPED})*>|` + `(?:${PLAIN}|${ESCAPED}|\\((?:${PLAIN}|${ESCAPED})*\\))+`;
const TITLE =
  `"(?:[^"\\\\]|${ESCAPED})*"|'(?:[^'\\\\]|${ESCAPED})*'|` + `\\((?:[^()\\\\]|${ESCAPED})*\\)`;
const LINK_TAIL = new RegExp(
  `^\\(${SPACE}*(?:${DESTINATION})?(?:${SPACE}+(?:${TITLE}))?${SPACE}*\\)`,
);

interface Opener {
  image: boolean;
  active: boolean;
}

/** The `[` and `![` of a cell that a later `]` may close into a link or an image. */
export class LinkOpeners {
  private readonly openers: Opener[] = [];

  /** Record the opener of a link (`[`) or an image (`![`). */
  open(kind: 'link' | 'image'): void {
    this.openers.push({ image: kind === 'image', active: true });
  }

  /**
   * Close the innermost opener with the `]` that `rest` follows. Returns the length of the
   * link's tail when the two make a link, else 0. A link cannot hold a link, so the link
   * openers around one that closes into a link become text.
   */
  close(rest: string): number {
    const innermost = this.openers.pop();
    if (!innermost) return 0;
    const tail = LINK_TAIL.exec(rest)?.[0].length ?? 0;
    if (!innermost.active || tail === 0) return 0;
    if (!innermost.image) {
      for (const opener of this.openers) if (!opener.image) opener.active = false;
    }
    return tail;
  }
}
