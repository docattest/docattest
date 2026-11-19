import type { Kind } from './constants.js';

/** Options for `mutate`. */
export interface MutateOptions {
  /** Paths or globs, relative to the project root, of the pages whose proofs to judge. */
  pages?: readonly string[] | undefined;
}

/** A proof `mutate` judged: where it is, and its verdict. */
export type MutatedProof = {
  page: string;
  line: number;
  kind: Kind;
  claim: string;
  /** For a row of a table: its first cell, which names it. */
  row?: string;
} & (
  | {
      /** Some change to what it reads made it fail. */
      status: 'killed';
      /** The mutants run against it that built. */
      mutants: number;
      /** The mutants that left the code unable to build or load, which kill nothing. */
      stillborn: number;
      /** The first change that made it fail. */
      killedBy: string;
      /** For a row: the columns no change of which made it fail. */
      ignoredCells?: string[];
    }
  | {
      /** No change to what it reads made it fail. */
      status: 'survived';
      mutants: number;
      stillborn: number;
      survivors: string[];
    }
  | {
      /** Nothing it reads could be changed. */
      status: 'notMutated';
      why: string;
    }
);

/** What `mutate` found: a verdict on each proof, and the counts. */
export interface Mutation {
  proofs: MutatedProof[];
  summary: {
    proofs: number;
    killed: number;
    survived: number;
    notMutated: number;
    mutants: number;
    /** How many of the mutants left the code unable to build or load. */
    stillborn: number;
  };
}
