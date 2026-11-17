import type { Status } from '../adapters/typescript/src/api/constants.ts';
import type { Report, Result } from '../adapters/typescript/src/api/report.ts';
import type { Finding } from './findings.ts';

/** One line with the results counted across the report and its members, and the verdict,
 *  worded as the text report words them. */
export function verdict(report: Report): string {
  const count = tally(report);
  const outcome = report.exitCode === 0 ? 'ok' : 'FAILED';
  return (
    `docattest: ${count.pass} pass, ${count.fail} fail, ${count.error} error, ` +
    `${count.skipped} skipped: ${outcome}`
  );
}

/** The Markdown the run's summary page shows: the verdict, then a row for each finding. */
export function jobSummary(report: Report, findings: Finding[]): string {
  const heading = `### ${report.exitCode === 0 ? '✓' : '✗'} ${verdict(report)}\n`;
  if (findings.length === 0) return `${heading}\n`;
  const rows = findings.map(
    (finding) =>
      `| ${cell(finding.page)}:${finding.line} | ${cell(finding.title)} | ` +
      `${cell(finding.message)} |`,
  );
  return [heading, '| Where | What | Message |', '| --- | --- | --- |', ...rows, '', ''].join('\n');
}

function tally(report: Report): Record<Status, number> {
  const count: Record<Status, number> = { pass: 0, fail: 0, error: 0, skipped: 0 };
  for (const result of allResults(report)) count[result.status] += 1;
  return count;
}

function allResults(report: Report): Result[] {
  return [
    ...report.results,
    ...(report.members ?? []).flatMap((member) => allResults(member.report)),
  ];
}

/** A table cell holds one line, and a `|` would end it. */
function cell(text: string): string {
  return text.replaceAll('|', '\\|').replaceAll(/\r?\n/g, ' ');
}
