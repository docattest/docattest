import { posix } from 'node:path';
import type { Change, Report, Result } from '../adapters/typescript/src/api/report.ts';

/** Something in a report that needs attention, at a line of a page. */
export interface Finding {
  /** The page's path, from the directory given to {@link findings}, with `/` between its
   *  parts. */
  page: string;
  line: number;
  level: 'error' | 'warning';
  title: string;
  message: string;
}

/** What needs attention in a report and in its members' reports: each result that fails or
 *  errors, and each thing a change did to the proofs of the base. Passes and skips need
 *  nothing, so they are left out. `directory` is where docattest ran, and each page's path
 *  starts from the directory above it. */
export function findings(report: Report, directory = ''): Finding[] {
  return [
    ...report.results.filter(needsAttention).map((result) => fromResult(result, directory)),
    ...(report.changes ?? []).map((change) => fromChange(change, directory)),
    ...(report.members ?? []).flatMap((member) =>
      findings(member.report, posix.join(directory, member.path)),
    ),
  ];
}

function needsAttention(result: Result): boolean {
  return result.status === 'fail' || result.status === 'error';
}

function fromResult(result: Result, directory: string): Finding {
  return {
    page: posix.join(directory, result.page),
    line: result.line,
    level: 'error',
    title: `docattest ${result.kind}: ${result.status}`,
    message: result.message,
  };
}

function fromChange(change: Change, directory: string): Finding {
  return {
    page: posix.join(directory, change.page),
    line: change.line,
    level: change.severity === 'fail' ? 'error' : 'warning',
    title: `docattest change: ${change.type}`,
    message: change.message,
  };
}
