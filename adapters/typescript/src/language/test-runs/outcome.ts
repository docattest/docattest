import type { TestRun } from '../../api/adapter.js';

/** How a runner reports that a test ended. */
export type Ended =
  | { status: 'passed' }
  | { status: 'failed'; message: string }
  | { status: 'skipped'; reason: string | undefined };

/** A test as its runner reported it, by its full path: describe titles, then its title. */
export interface ReportedTest {
  path: string[];
  ended: Ended;
}

/** What one run of a runner reported of the file it ran. */
export interface RunnerReport {
  tests: ReportedTest[];
  /** What the runner said when the file could not run at all, such as an import it lacks. */
  fileFailure: string | undefined;
}

/**
 * How a cited test ran, from the report of the run that selected it. `reported` holds the
 * full paths the runner reports it by: one, or one for each row of a table cited as a whole,
 * which passes only when every row does. A test's own subtests count as part of it.
 */
export function outcomeOf(
  report: RunnerReport,
  reported: readonly string[][],
  runner: string,
): TestRun {
  if (!reported.length) {
    return { status: 'notRun', message: `${runner} has nothing to run: the table has no rows` };
  }
  const ran = reported.map((path) => report.tests.filter((t) => samePath(t.path, path)));
  const missing = reported.find((_, k) => !ran[k]?.length);
  if (missing) {
    const message = report.fileFailure
      ? `${runner} could not run the file: ${withoutStack(report.fileFailure)}`
      : `${runner} selected no test named ${pathText(missing)}`;
    return { status: 'notRun', message };
  }
  const failures = failuresWithin(report.tests, reported);
  if (failures.length) return { status: 'failed', message: failures.join('\n') };
  const skipped = ran.flat().flatMap((t) => (t.ended.status === 'skipped' ? [t.ended] : []));
  if (!skipped.length) return { status: 'passed' };
  const reasons = [...new Set(skipped.flatMap((s) => s.reason ?? []))];
  return { status: 'skipped', message: reasons.join('\n') || `${runner} skipped it` };
}

// The failures that are not only a summary of others: a test whose subtests failed is
// reported as failed too, and its own message says no more than that. Each one that is not
// the cited test itself is named.
function failuresWithin(tests: readonly ReportedTest[], reported: readonly string[][]): string[] {
  const failed = tests.flatMap(({ path, ended }) =>
    ended.status === 'failed' && reported.some((cited) => startsWith(path, cited))
      ? [{ path, message: withoutStack(ended.message) || 'it failed, with no message' }]
      : [],
  );
  const leaves = failed.filter(
    (t) => !failed.some((other) => other !== t && startsWith(other.path, t.path)),
  );
  return leaves.map(({ path, message }) => {
    const itself = reported.length === 1 && samePath(path, reported[0] ?? []);
    return itself ? message : `${path.at(-1) ?? ''}: ${message}`;
  });
}

/** A runner's message without its stack frames and blank lines. */
function withoutStack(message: string): string {
  return message
    .split('\n')
    .filter((line) => line.trim() !== '' && !/^\s+at /.test(line))
    .join('\n');
}

const samePath = (a: readonly string[], b: readonly string[]) =>
  a.length === b.length && startsWith(a, b);

const startsWith = (path: readonly string[], prefix: readonly string[]) =>
  path.length >= prefix.length && prefix.every((title, k) => path[k] === title);

/** A test's path as messages write it: `"describe" > "title"`. */
export const pathText = (path: readonly string[]) => path.map((title) => `"${title}"`).join(' > ');
