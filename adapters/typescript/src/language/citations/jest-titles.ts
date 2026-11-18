// The title Jest gives each row of `test.each` and `describe.each`, as jest-each writes it
// (src/table/array.ts and template.ts). Only what a title shows as itself is followed: a
// value it would pretty-print (an array, an object), `%p`, `%o`, `%O`, `%c`, `%$` and a
// second `%#` give no title.

import { placeholderValue, replaceEach } from './title-text.js';

const UNFOLLOWED = /%[pOoc$]/;
const PLACEHOLDER = /%[sdifj]/;
// Jest's own stand-in for `%`, so that `%%` and a `%` in a value are not placeholders.
const ESCAPE = '@@__JEST_EACH_PLACEHOLDER_ESCAPE__@@';

/** The title of each row of an array table; undefined where it cannot be told. */
export function jestArrayTitles(
  template: string,
  rows: readonly unknown[],
): (string | undefined)[] {
  if (UNFOLLOWED.test(template) || template.split('%#').length > 2) {
    return rows.map(() => undefined);
  }
  const escaped = template.replace(/%%/g, ESCAPE);
  if (isNamedTable(escaped, rows)) {
    return rows.map((row, index) => interpolateNames(template, row, index)?.replace(/%%/g, '%'));
  }
  const table = rows.every((row) => Array.isArray(row)) ? rows : rows.map((row) => [row]);
  return table.map((row, index) =>
    placeholders(escaped.replace('%#', `${index}`), row as unknown[]),
  );
}

/** The title of row `index` of a template-literal table; undefined when it cannot be told. */
export function jestTemplateTitle(
  template: string,
  row: Record<string, unknown>,
  index: number,
): string | undefined {
  return template.includes('%') ? undefined : interpolateNames(template, row, index);
}

// A table of objects (not all arrays) whose title has no placeholder is read by `$name`;
// any other table, by placeholders, one value of a row for each.
function isNamedTable(title: string, rows: readonly unknown[]): rows is Record<string, unknown>[] {
  return (
    !/%[sdifjoOp#]/.test(title) &&
    !rows.every((row) => Array.isArray(row)) &&
    rows.every((row) => typeof row === 'object' && row !== null)
  );
}

function placeholders(template: string, row: readonly unknown[]): string | undefined {
  let title = template;
  for (const value of row) {
    const placeholder = PLACEHOLDER.exec(title);
    if (!placeholder) break;
    const shown = formatted(
      placeholder[0],
      typeof value === 'string' ? value.replace(/%/g, ESCAPE) : value,
    );
    if (shown === undefined) return undefined;
    title = title.slice(0, placeholder.index) + shown + title.slice(placeholder.index + 2);
  }
  return title.replaceAll(ESCAPE, '%');
}

function formatted(placeholder: string, value: unknown): string | undefined {
  return isPrimitive(value) ? placeholderValue(placeholder, value) : undefined;
}

function interpolateNames(
  title: string,
  row: Record<string, unknown>,
  index: number,
): string | undefined {
  const keys = Object.keys(row);
  if (keys.some((key) => !/^\w+$/.test(key))) return undefined;
  const named = new RegExp(`\\$(${keys.join('|')})[.\\w]*`, 'g');
  const text = replaceEach(title, named, ([match]) => {
    const value = path(row, match.slice(1).split('.'));
    return isPrimitive(value) ? String(value) : undefined;
  });
  return text?.replace('$#', `${index}`);
}

// Jest's `getPath`: a key the value does not own stops the walk at the value.
function path(value: unknown, [head, ...tail]: string[]): unknown {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (!head || !Object.hasOwn(Object(value) as object, head)) return value;
  const inner = (Object(value) as Record<string, unknown>)[head];
  return tail.length ? path(inner, tail) : inner;
}

function isPrimitive(value: unknown): boolean {
  return Object(value) !== value;
}
