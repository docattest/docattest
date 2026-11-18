import type { Kind } from './constants.js';

/** What `impact` compares the working tree with. */
export interface ImpactOptions {
  /** A git revision. Default: `HEAD`. */
  base?: string | undefined;
  /**
   * The project's files at the base, path → contents: the only way to give an in-memory
   * project a base (they are committed with git in its temporary directory), and only for
   * one. `base` then only labels the answer.
   */
  baseFiles?: Record<string, string> | undefined;
}

/** One place an affected symbol appears in a living page. */
export interface ImpactReference {
  symbol: string;
  page: string;
  line: number;
  /** An attestation's kind, or `mention`: a code span that names the symbol unproven. */
  kind: Kind | 'mention';
  /** The sentence an attestation defends, or the sentence holding the mention. */
  claim: string;
  /**
   * Whether an attestation docattest runs or compares proves the claim; `false` for mentions,
   * and for cited tests, which docattest checks are there but never runs.
   */
  proven: boolean;
}

/** The prose a change touches: what `docattest impact` prints. */
export interface Impact {
  /** The revision (or label) the working tree was compared with. */
  base: string;
  /**
   * Surface exports whose declaration changed, or that reach a changed declaration through
   * the code they use. Sorted.
   */
  symbols: string[];
  /**
   * Manifests that facts read with `manifest("<path>")` and the change touched, sorted; only
   * when there are any. A reference to one has its call as its `symbol`.
   */
  manifests?: string[];
  /**
   * Files that facts read with `file("<path>")` and the change touched, sorted; only when
   * there are any. A reference to one has its call as its `symbol`.
   */
  files?: string[];
  /**
   * Commands built from a changed file (the config's `commands`), sorted; only when there are
   * any. A reference to one, a block that runs it, has its call as its `symbol`: `$ mytool`.
   */
  commands?: string[];
  /** By symbol, then page, then line. */
  references: ImpactReference[];
}
