/**
 * Prose for claims, as the core writes a claim: HTML comments replaced by a space, every run
 * of whitespace collapsed to one space, trimmed, markup kept (PROTOCOL.md §3, `docProse`).
 */
export function prose(source: string): string {
  return source
    .replace(/<!--[\s\S]*?-->/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}
