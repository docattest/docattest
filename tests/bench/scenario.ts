// What a benchmark scenario is, how a part of the scenarios adds one, and how its verdict
// is reached: shared by the benchmark (tests/bench/run.ts), the parity harness
// (tests/parity/bench.ts) and the mirror test (tests/mirror.ts).
import type { ProjectFiles } from '../lib/project.ts';
import type { Answer } from './answer.ts';

/** What a scenario measures; `left-to-binding` is drift only a directive would catch. */
export type Category = 'audit' | 'left-to-binding' | 'guard' | 'impact' | 'citations';

/** Positive: drift the tool should surface. Negative: a state it must stay silent on. */
export type Polarity = 'positive' | 'negative';

/** What a command did: its exit code, its output, and that output parsed when it is JSON. */
export interface Outcome {
  code: number | null;
  stdout: string;
  stderr: string;
  json: Answer | undefined;
}

/** How a scenario judges what its command did. */
export type Judge = (out: Outcome) => boolean;

/** What a scenario sets up and runs, and how it judges what the command did. */
export interface ScenarioBody {
  /** The project, written before anything runs (and committed when there is a `head`). */
  files: ProjectFiles<string>;
  /** The change made on top of the committed `files`; `null` removes a file. */
  head?: ProjectFiles<string> | undefined;
  /** The command line's arguments. */
  run: string[];
  /** Whether the command did what the scenario expects. */
  expect: Judge;
}

/** One benchmark scenario. */
export interface Scenario extends ScenarioBody {
  category: Category;
  polarity: Polarity;
  id: string;
  title: string;
}

/** How a part of the scenarios adds one: `add(category, polarity, id, title, body)`. */
export type Add = (
  category: Category,
  polarity: Polarity,
  id: string,
  title: string,
  body: ScenarioBody,
) => void;

/** The `add` of a part of the scenarios, which appends to its list. */
export const adder =
  (scenarios: Scenario[]): Add =>
  (category, polarity, id, title, body) => {
    scenarios.push({ category, polarity, id, title, ...body });
  };

/** A scenario's verdict: met, or not, with what the expectation threw when it threw. */
export type Verdict = { met: true } | { met: false; thrown: string | undefined };

/** Judge an outcome by the scenario's own expectation; one that throws is not met. */
export function judge(scenario: Pick<ScenarioBody, 'expect'>, out: Outcome): Verdict {
  try {
    return scenario.expect(out) ? { met: true } : { met: false, thrown: undefined };
  } catch (error) {
    return { met: false, thrown: error instanceof Error ? error.message : String(error) };
  }
}
