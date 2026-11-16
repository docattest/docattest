import type { Attestation } from '../../api/attestation.js';
import type { Status } from '../../api/constants.js';
import type { Result } from '../../api/report.js';

/** Optional parts of a result. */
interface ResultDetail {
  line?: number | undefined;
  expected?: string | undefined;
  actual?: string | undefined;
}

/** A result located at an attestation (or at one of its rows, via `detail.line`). */
export function resultFor(
  attestation: Pick<Attestation, 'page' | 'line' | 'kind'>,
  status: Status,
  message: string,
  detail: ResultDetail = {},
): Result {
  const { page, kind } = attestation;
  const result: Result = {
    page,
    line: detail.line ?? attestation.line,
    kind,
    status,
    symbols: [],
    message,
  };
  if (detail.expected !== undefined) result.expected = detail.expected;
  if (detail.actual !== undefined) result.actual = detail.actual;
  return result;
}
