// The shape of an impact case (tests/parity/impact.ts runs them): a project, maybe a home
// directory, what to do to them before the first step, and the steps.
import type { ProjectFiles } from '../../lib/project.ts';

/** Files that are all text: what a project of the fixtures is made of. */
export type TextFiles = Record<string, string>;

/** Runs git in the project, with the case's environment, quietly. */
export type Git = (...args: string[]) => void;

/** Writes more files into the project. */
export type Put = (files: ProjectFiles) => void;

/** What a case does to its project before the first step. */
export type Prepare = (root: string, git: Git, put: Put) => void;

/**
 * One run of the candidate: its arguments (`__ROOT__` and `__HOME__` stand for the project
 * and the home directory), and what it reads on stdin, adds to the environment, and runs in
 * (relative to the project).
 */
export interface StepSpec {
  args: readonly string[];
  stdin?: string;
  env?: Readonly<Record<string, string>>;
  cwd?: string;
}

/** A step: bare arguments, or a StepSpec. */
export type CaseStep = readonly string[] | StepSpec;

/** One impact case: its name is its recording's. */
export interface ImpactCase {
  name: string;
  files: ProjectFiles;
  home?: ProjectFiles;
  prepare?: Prepare;
  steps: readonly CaseStep[];
}

/** A step as a StepSpec, whichever way it was written. */
export function stepSpec(step: CaseStep): StepSpec {
  return 'args' in step ? step : { args: step };
}
