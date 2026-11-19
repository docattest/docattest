import { isObject, isStringArray, objectOrNothing, type JsonObject } from '../../json.js';
import type { Ended, ReportedTest, RunnerReport } from './outcome.js';

/**
 * What a run reported of `file` (absolute), from the JSON report Jest writes with `--json`,
 * which Vitest's `json` reporter writes too; undefined when `text` is no such report or holds
 * nothing of the file. A file that holds no result and a message could not run.
 */
export function readJsonReport(text: string, file: string): RunnerReport | undefined {
  const report = objectOrNothing(text);
  if (!report || !Array.isArray(report.testResults)) return undefined;
  const result = report.testResults.find((r): r is JsonObject => isObject(r) && r.name === file);
  if (!result) return undefined;
  const assertions = Array.isArray(result.assertionResults) ? result.assertionResults : [];
  const tests = assertions.flatMap((a) => (isObject(a) ? (reportedTest(a) ?? []) : []));
  const message = typeof result.message === 'string' ? result.message : '';
  return { tests, fileFailure: !assertions.length && message ? message : undefined };
}

function reportedTest(assertion: JsonObject): ReportedTest | undefined {
  const { ancestorTitles, title, status, failureMessages } = assertion;
  if (!isStringArray(ancestorTitles) || typeof title !== 'string') return undefined;
  const messages = isStringArray(failureMessages) ? failureMessages : [];
  return { path: [...ancestorTitles, title], ended: endedOf(status, messages) };
}

// Jest also says `pending` and `disabled` for a test it skipped, and Vitest `pending`.
function endedOf(status: unknown, messages: readonly string[]): Ended {
  if (status === 'passed') return { status: 'passed' };
  if (status === 'failed') return { status: 'failed', message: messages.join('\n') };
  if (status === 'todo') return { status: 'skipped', reason: 'it is marked todo' };
  return { status: 'skipped', reason: undefined };
}
