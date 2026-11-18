// The title Vitest gives each row of `test.each`, `test.for` and `describe.each`, as its
// `formatTitle` (packages/runner/src/suite.ts) writes it. Only values a title shows as
// themselves are followed: a placeholder or a `$name` that would show an array, an object or
// a function, or `-0`, gives no title.

import { placeholderValue, replaceEach } from './title-text.js';

const PLACEHOLDER = /%[sdjifoOc%]/g;
const NAME = /\$([$\w.]+)/g;
// What Vitest's `inspect` escapes in a string, by UTF-16 code unit, and the length past
// which it cuts one short.
const ESCAPED: readonly [number, number][] = [
  [0x00, 0x1f],
  [0x27, 0x27],
  [0x7f, 0x9f],
  [0xad, 0xad],
  [0x600, 0x604],
  [0x70f, 0x70f],
  [0x17b4, 0x17b5],
  [0x200c, 0x200f],
  [0x2028, 0x202f],
  [0x2060, 0x206f],
  [0xfeff, 0xfeff],
  [0xfff0, 0xffff],
];
const LONGEST_SHOWN = 38;

/** The title of row `index`, whose arguments are `items`; undefined when it cannot be told. */
export function vitestTitle(
  template: string,
  items: readonly unknown[],
  index: number,
): string | undefined {
  let title = template;
  if (title.includes('%#') || title.includes('%$')) {
    title = title
      .replace(/%%/g, '\0')
      .replace(/%#/g, `${index}`)
      .replace(/%\$/g, `${index + 1}`)
      .replace(/\0/g, '%%');
  }
  const formatted = format(title, items.slice(0, title.split('%').length - 1));
  return formatted === undefined ? undefined : interpolateNames(formatted, items);
}

function format(template: string, args: readonly unknown[]): string | undefined {
  if (args.some((arg) => !isShown(arg))) return undefined;
  let next = 0;
  const text = replaceEach(template, PLACEHOLDER, ([placeholder]) => {
    if (placeholder === '%%') return '%';
    return next < args.length ? placeholderValue(placeholder, args[next++]) : placeholder;
  });
  const rest = args.slice(next).map((arg) => ` ${String(arg)}`);
  return text === undefined ? undefined : text + rest.join('');
}

// `$name` reads a field of the row's object; `$0` an argument by its position.
function interpolateNames(title: string, items: readonly unknown[]): string | undefined {
  const [first] = items;
  const isObjectRow = typeof first === 'object' && first !== null && !Array.isArray(first);
  return replaceEach(title, NAME, ([whole, key = '']) => {
    const isPosition = /^\d+$/.test(key);
    if (!isObjectRow && !isPosition) return whole;
    const byPosition = isPosition ? attribute(items, key) : undefined;
    return display(isObjectRow ? attribute(first, key, byPosition) : byPosition);
  });
}

function attribute(source: unknown, path: string, fallback?: unknown): unknown {
  let value = source;
  for (const key of path.replace(/\[(\d+)\]/g, '.$1').split('.')) {
    value = (Object(value) as Record<string, unknown>)[key];
    if (value === undefined) return fallback;
  }
  return value;
}

// How Vitest's `objDisplay` shows a value in a title.
function display(value: unknown): string | undefined {
  if (!isShown(value)) return undefined;
  if (typeof value === 'string') {
    return isEscaped(value) || value.length > LONGEST_SHOWN ? undefined : `'${value}'`;
  }
  if (value === 0) return '+0';
  return typeof value === 'bigint' ? `${value}n` : String(value);
}

function isShown(value: unknown): boolean {
  return (
    (typeof value !== 'object' || value === null) &&
    typeof value !== 'function' &&
    !Object.is(value, -0)
  );
}

function isEscaped(text: string): boolean {
  for (let at = 0; at < text.length; at++) {
    const unit = text.charCodeAt(at);
    if (ESCAPED.some(([from, to]) => unit >= from && unit <= to)) return true;
  }
  return false;
}
