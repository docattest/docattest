import ts from '../ts.cjs';
import { jestArrayTitles, jestTemplateTitle } from './jest-titles.js';
import type { TestScope } from './test-scope.js';
import { vitestTitle } from './vitest-titles.js';

/** A runner whose way of titling a table's rows is known. */
export type TestRunner = 'vitest' | 'jest';

/** The table of a `.each(table)`, a `.each` template literal or a `.for(table)`. */
export type EachTable =
  | { kind: 'each' | 'for'; table: ts.Expression | undefined }
  | { kind: 'template'; template: ts.TemplateLiteral };

/** Each row of a table: the title it gives, and the arguments its callback gets. */
export interface TableRow {
  title: string | undefined;
  args: unknown[];
}

/**
 * The runner a test file is written for, by what it imports its test functions from;
 * undefined when it says nothing (globals) or names another runner.
 */
export function runnerOf(source: ts.SourceFile): TestRunner | undefined {
  for (const statement of source.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) {
      continue;
    }
    const from = statement.moduleSpecifier.text;
    if (from === 'vitest') return 'vitest';
    if (from === '@jest/globals') return 'jest';
  }
  return undefined;
}

/**
 * The rows of a table and each one's title, as the runner prints it; undefined when the
 * table is not written in the file. When the runner is not known, a title is one only where
 * Vitest and Jest print the same.
 */
export function tableRows(
  each: EachTable,
  title: string,
  runner: TestRunner | undefined,
  scope: TestScope,
): TableRow[] | undefined {
  if (each.kind === 'template') return templateRows(each.template, title, runner, scope);
  const folded = each.table && scope.fold(each.table);
  const rows = folded?.value;
  if (!Array.isArray(rows)) return undefined;
  const items = rows.map((row: unknown) => vitestItems(row, each.kind));
  const vitest = items.map((row, index) => vitestTitle(title, row, index));
  const titles =
    each.kind === 'for' ? vitest : agreed(runner, vitest, jestArrayTitles(title, rows));
  const spread = each.kind === 'each' && rows.every((row) => Array.isArray(row));
  return rows.map((row: unknown, k) => ({
    title: titles[k],
    args: spread ? (row as unknown[]) : [row],
  }));
}

// The values Vitest titles a row with: an array row's own, else the row; `.for` takes a row
// of `null` or `undefined` as no values.
function vitestItems(row: unknown, kind: 'each' | 'for'): unknown[] {
  if (Array.isArray(row)) return row as unknown[];
  return kind === 'for' && (row === null || row === undefined) ? [] : [row];
}

// A template-literal table: a header line of names split by `|`, then one value per cell.
function templateRows(
  template: ts.TemplateLiteral,
  title: string,
  runner: TestRunner | undefined,
  scope: TestScope,
): TableRow[] | undefined {
  if (!ts.isTemplateExpression(template)) return undefined;
  const strings = [template.head.text, ...template.templateSpans.map((s) => s.literal.text)];
  const header = headerOf(strings);
  const cells = template.templateSpans.map((span) => scope.fold(span.expression));
  if (!header || cells.some((c) => !c) || cells.length % header.length !== 0) return undefined;
  const rows: Record<string, unknown>[] = [];
  for (let at = 0; at < cells.length; at += header.length) {
    const row: Record<string, unknown> = {};
    header.forEach((name, k) => {
      Object.defineProperty(row, name, { value: cells[at + k]?.value, enumerable: true });
    });
    rows.push(row);
  }
  const vitest = rows.map((row, index) => vitestTitle(title, [row], index));
  const jest = rows.map((row, index) => jestTemplateTitle(title, row, index));
  const titles = agreed(runner, vitest, jest);
  return rows.map((row, k) => ({ title: titles[k], args: [row] }));
}

// Jest reads the names from the text before the first value, Vitest from the first line of
// the whole template; a header they read differently is not followed.
function headerOf(strings: readonly string[]): string[] | undefined {
  const jest = (strings[0] ?? '').replace(/\s/g, '').split('|');
  const vitest = (strings.join('').trim().replace(/ /g, '').split('\n')[0] ?? '').split('|');
  const same = jest.length === vitest.length && jest.every((name, k) => name === vitest[k]);
  return same && jest.every((name) => name !== '') ? jest : undefined;
}

function agreed(
  runner: TestRunner | undefined,
  vitest: readonly (string | undefined)[],
  jest: readonly (string | undefined)[],
): (string | undefined)[] {
  if (runner === 'vitest') return [...vitest];
  if (runner === 'jest') return [...jest];
  return vitest.map((title, k) => (title === jest[k] ? title : undefined));
}
