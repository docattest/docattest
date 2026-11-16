// The JSON report a command prints, as far as the scenarios read it, and reading it from
// what the command printed.

/** One of `audit --json`'s suggestions. */
export interface Suggestion {
  page: string;
  line: number;
  target: string;
  status: string;
  expression: string;
  missing?: string[];
  unexpected?: string[];
}

/** One of `check --base --json`'s changes. */
export interface Change {
  type: string;
  page: string;
  line: number;
  severity: string;
}

/** One of `impact --json`'s references. */
export interface Reference {
  symbol: string;
  page: string;
  line: number;
  kind: string;
  proven?: boolean;
}

/** One of `check --json`'s results. */
export interface Result {
  kind: string;
  page: string;
  line: number;
  status: string;
}

/**
 * The JSON a command printed, as far as the scenarios read it: the fields of `check`,
 * `audit` and `impact` reports. It is the command's output, so every field may be missing.
 */
export interface Answer {
  suggestions?: Suggestion[];
  changes?: Change[];
  references?: Reference[];
  symbols?: string[];
  results?: Result[];
  coverage?: { uncovered?: string[] };
}

/** Parse a command's stdout as the JSON report it may be. */
export function answerOf(stdout: string): Answer | undefined {
  try {
    // A report of the core's: read as far as the scenarios read it, and judged by them.
    return JSON.parse(stdout) as Answer;
  } catch {
    return undefined;
  }
}
