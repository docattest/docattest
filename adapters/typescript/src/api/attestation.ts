/** What a fact compares its value with: the literal, list, column, block or field on the page. */
export type FactTarget =
  | { kind: 'scalar'; value: string }
  | { kind: 'set'; items: string[]; from: 'list' | 'table' | 'sentence' | 'field' }
  | { kind: 'text'; text: string };

interface AttestationBase {
  /** Unique within one check. */
  id: string;
  page: string;
  line: number;
  /** The sentence the proof defends. */
  claim: string;
  /** A problem found while planning; the attestation is an `error` without running. */
  error?: string;
}

/** A fenced block, or a doc-comment `@example`, that compiles and maybe runs. */
export interface ExampleAttestation extends AttestationBase {
  kind: 'example';
  lang: string;
  mode: 'check' | 'compile';
  network: boolean;
  code: string;
  /** A `check` block with no assertion: it runs, but it is an illustration. */
  weak: boolean;
  /** The documented export, for doc-comment examples. */
  owner?: string;
  /**
   * The adapter's own info-string words on the block (its `infoWords`), as written, an
   * attribute as `key=value`. Absent when there are none. Only the adapter reads them.
   */
  tags?: string[];
}

/** A literal on the page checked against an expression over a surface. */
export interface FactAttestation extends AttestationBase {
  kind: 'fact';
  expression: string;
  /** Surface name; `default` unless the directive says `@name`. */
  surface: string;
  ordered: boolean;
  column?: string;
  /** The path `field=` names in the fenced block after the fact, as written. */
  field?: string;
  /** Absent only when the attestation carries an `error`. */
  target?: FactTarget;
}

/** One row of a cases table: its line, its first cell and every cell by header. */
export interface CasesRow {
  line: number;
  name: string;
  values: Record<string, string>;
}

/** A decision table run row by row by a setup-module runner. */
export interface CasesAttestation extends AttestationBase {
  kind: 'cases';
  runner: string;
  header: string[];
  rows: CasesRow[];
}

/** A sentence tied by name to a test that already exists; docattest does not run it. */
export interface TestAttestation extends AttestationBase {
  kind: 'test';
  /** The test file, relative to the project root. */
  file: string;
  /** The test's own title. */
  title: string;
  /** Titles of the enclosing describe blocks, outermost first; empty matches any. */
  describes: string[];
}

/** One unit of proof found on a page. */
export type Attestation = ExampleAttestation | FactAttestation | CasesAttestation | TestAttestation;
