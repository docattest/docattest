/**
 * What a suggestion's literal is: a list, a table column, a fenced block, an enumeration
 * written inline in a sentence (its code spans), or a single value in prose.
 */
export type AuditTarget = 'list' | 'column' | 'block' | 'inline' | 'scalar';

/** `match` when page and code agree, `drift` when they nearly agree. */
export type AuditStatus = 'match' | 'drift';

/** What every suggestion says, whatever its literal is. */
interface SuggestionFields {
  page: string;
  /**
   * The first line of the list, table or block; the line an inline enumeration's sentence
   * ends on; a scalar's line.
   */
  line: number;
  /** A fact expression over the surface that the literal matches or nearly matches. */
  expression: string;
  status: AuditStatus;
  /** In the code, not on the page. */
  missing: string[];
  /** On the page, not in the code. */
  unexpected: string[];
  /** The surface the expression reads. */
  surface: string;
  /**
   * The directive to paste above the list, table or block, at the end of the sentence, or
   * right after the scalar.
   */
  directive: string;
}

/** A literal on a living page that looks copied from a surface and carries no directive. */
export type Suggestion =
  | (SuggestionFields & { target: Exclude<AuditTarget, 'scalar'> })
  | (SuggestionFields & {
      target: 'scalar';
      /**
       * The value as the page writes it (a code span with its backticks): the directive goes
       * right after it.
       */
      after: string;
    });

/** One value a surface offers, with the fact expression that reads it. */
export interface InventoryEntry {
  surface: string;
  /** A fact expression that evaluates to this value. */
  expression: string;
  /** `String(value)`. */
  value: string;
  /** Each item, when the value is a list. */
  items?: string[];
}

/** Everything `audit` found, ordered by page, then line. */
export interface AuditReport {
  suggestions: Suggestion[];
}
