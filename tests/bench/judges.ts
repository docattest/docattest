// The judges the scenarios are written with: what a command's JSON report must, or must not,
// hold, one group for each command the scenarios run.
import type { Change, Reference, Suggestion } from './answer.ts';
import type { Outcome } from './scenario.ts';

const sameSet = (a: unknown, b: readonly string[]): boolean =>
  Array.isArray(a) &&
  a.length === b.length &&
  [...(a as string[])].sort().join('\n') === [...b].sort().join('\n');

/** The command answered with JSON and exit code `code`. */
export const answered = (out: Outcome, code: number): boolean =>
  out.json !== undefined && out.code === code;

// --- audit -------------------------------------------------------------------------

/** What a scenario expects of a suggestion: `mentions` is part of its expression. */
export interface WantedSuggestion {
  page: string;
  line: number;
  target: string;
  status: string;
  mentions: string;
  missing?: string[];
  unexpected?: string[];
}

/** `audit --json` holds a suggestion matching every given field. */
export function suggests(out: Outcome, want: WantedSuggestion): boolean {
  return (out.json?.suggestions ?? []).some((s) => isSuggestion(s, want));
}

const SUGGESTION_FIELDS = ['page', 'line', 'target', 'status'] as const;

function isSuggestion(s: Suggestion, want: WantedSuggestion): boolean {
  return (
    SUGGESTION_FIELDS.every((field) => s[field] === want[field]) &&
    s.expression.includes(want.mentions) &&
    sameSet(s.missing ?? [], want.missing ?? []) &&
    sameSet(s.unexpected ?? [], want.unexpected ?? [])
  );
}

/** `audit --json` answered and suggested nothing (optionally: nothing on `page`). */
export function suggestsNothing(out: Outcome, page?: string): boolean {
  const suggestions = out.json?.suggestions;
  if (!Array.isArray(suggestions)) return false;
  return suggestions.every((s) => page !== undefined && s.page !== page);
}

// --- check --base ------------------------------------------------------------------

/** What a scenario expects of a change; `line` and `severity` only when given. */
export interface WantedChange {
  type: string;
  page: string;
  line?: number;
  severity?: string;
}

/** `check --base --json` reports a change of `type` on `page` (at `line`, when given). */
export function reportsChange(out: Outcome, { type, page, line, severity }: WantedChange): boolean {
  return (out.json?.changes ?? []).some(
    (c: Change) =>
      c.type === type &&
      c.page === page &&
      (line === undefined || c.line === line) &&
      (severity === undefined || c.severity === severity),
  );
}

/** `check --base --json` answered with an empty `changes` list and exit code `code`. */
export const reportsNoChange = (out: Outcome, code = 0): boolean =>
  Array.isArray(out.json?.changes) && out.json.changes.length === 0 && out.code === code;

// --- impact ------------------------------------------------------------------------

/** `impact --json` lists a reference matching every given field. */
export function refers(out: Outcome, want: Partial<Reference>): boolean {
  return (out.json?.references ?? []).some((r) =>
    Object.entries(want).every(([key, value]) => r[key as keyof Reference] === value),
  );
}

export const affects = (out: Outcome, symbol: string): boolean =>
  (out.json?.symbols ?? []).includes(symbol);

/** `impact --json` answered with no affected symbols and no references. */
export const affectsNothing = (out: Outcome): boolean =>
  Array.isArray(out.json?.references) &&
  out.json.references.length === 0 &&
  Array.isArray(out.json.symbols) &&
  out.json.symbols.length === 0;

// --- test citations ----------------------------------------------------------------

/** `check --json` holds a result of kind `test` with `status` on `page` at `line`. */
export function cites(
  out: Outcome,
  { page, line, status }: { page: string; line: number; status: string },
): boolean {
  return (out.json?.results ?? []).some(
    (r) => r.kind === 'test' && r.page === page && r.line === line && r.status === status,
  );
}
