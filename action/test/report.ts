// Reports for the action's tests: only what a test is about is written out; the rest of the
// report is empty.
import type { Report, Result } from '../../adapters/typescript/src/api/report.ts';

export function report(fields: Partial<Report>): Report {
  return {
    results: [],
    coverage: {
      living: 0,
      covered: 0,
      uncovered: [],
      frozen: [],
      attestations: 0,
      weak: 0,
      cited: 0,
    },
    surfaces: {},
    snapshots: [],
    exitCode: 0,
    ...fields,
  };
}

export function result(fields: Pick<Result, 'page' | 'line' | 'status'> & Partial<Result>): Result {
  return { kind: 'fact', symbols: [], message: `${fields.status} at ${fields.page}`, ...fields };
}
