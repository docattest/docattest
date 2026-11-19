import type { CasesAttestation, FactAttestation, TestAttestation } from './attestation.js';
import type { Result } from './report.js';

/**
 * Where a directive takes effect: where it is written; alone above a paragraph or heading, as
 * if written where that one's first sentence ends, on `line`; or nowhere, detached by a blank
 * line from the prose below.
 */
export type Placement =
  { where: 'written' } | { where: 'above'; line: number } | { where: 'detached' };

/**
 * The rule a fact was bound by: `field=`, `column=`, the list or block on the line after it
 * (`block-below`), the code span or number before it on its line (`code-span`, `literal`), the
 * code spans of the sentence it ends (`sentence-spans`) or its one code span on an earlier
 * line (`sentence-span`), or the list or block after the paragraph it ends (`block-after`).
 */
export type FactRule =
  | 'field'
  | 'column'
  | 'block-below'
  | 'code-span'
  | 'literal'
  | 'sentence-spans'
  | 'sentence-span'
  | 'block-after';

/** A directive that attests: what it binds and why, and what checking it gives. */
export interface ExplainedAttestation {
  line: number;
  kind: 'fact' | 'test' | 'cases';
  placed: Placement;
  /** For a fact that bound a target. */
  rule?: FactRule;
  /** For a citation that names its file by the page's alias. */
  alias?: string;
  /** As planned: the claim, a fact's target, a citation's file and titles, a table's rows. */
  attestation: FactAttestation | TestAttestation | CasesAttestation;
  /** As `check` gives them; a fact's carries the value its expression gives as `actual`. */
  results: Result[];
}

/** A test file alias, `<!-- tests: <name> = <file> -->`, and the citations that use it. */
export interface ExplainedAlias {
  line: number;
  kind: 'alias';
  name: string;
  file: string;
  citedAt: number[];
}

/** What `explain` says of the directives on a page, or on one line of it, in line order. */
export interface Explanation {
  page: string;
  directives: (ExplainedAttestation | ExplainedAlias)[];
}
