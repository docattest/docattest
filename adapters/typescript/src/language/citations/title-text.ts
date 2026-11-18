// What the runners' ways of titling a row have in common.

/**
 * `text` with each match of a global `pattern` replaced, or undefined as soon as a
 * replacement is: a title with one part that cannot be told cannot be told at all.
 */
export function replaceEach(
  text: string,
  pattern: RegExp,
  replacement: (match: RegExpExecArray) => string | undefined,
): string | undefined {
  let out = '';
  let from = 0;
  for (const match of text.matchAll(pattern)) {
    const replaced = replacement(match);
    if (replaced === undefined) return undefined;
    out += text.slice(from, match.index) + replaced;
    from = match.index + match[0].length;
  }
  return out + text.slice(from);
}

// How each placeholder shows a primitive, as Node's `util.format` does, and Vitest's own
// `format` too for every value it is given here (it is never given `-0`).
const PLACEHOLDERS = new Map<string, (value: unknown) => string | undefined>([
  ['%s', (value) => bigintOf(value) ?? numberOf(value)],
  ['%d', (value) => bigintOf(value) ?? numberOf(Number(value))],
  ['%i', (value) => bigintOf(value) ?? numberOf(Number.parseInt(String(value)))],
  ['%f', (value) => numberOf(Number.parseFloat(String(value)))],
  ['%j', (value) => (typeof value === 'bigint' ? undefined : jsonOf(value))],
]);

/**
 * How a printf placeholder (`%s`, `%d`, `%i`, `%f`, `%j`) shows a primitive in a title;
 * undefined for another placeholder, or a value it cannot show.
 */
export function placeholderValue(placeholder: string, value: unknown): string | undefined {
  return PLACEHOLDERS.get(placeholder)?.(value);
}

function bigintOf(value: unknown): string | undefined {
  return typeof value === 'bigint' ? `${value}n` : undefined;
}

// `util.format` writes `-0` with its sign.
function numberOf(value: unknown): string {
  return Object.is(value, -0) ? '-0' : String(value);
}

// `JSON.stringify` gives nothing for `undefined`; a title shows it as `undefined`.
function jsonOf(value: unknown): string {
  return value === undefined ? 'undefined' : JSON.stringify(value);
}
