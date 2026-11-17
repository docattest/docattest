import type { Finding } from './findings.ts';

/** The workflow command that makes GitHub mark a finding on its line: in the job's log, on
 *  the run's page, and on the pull request's diff when the page is part of it. GitHub reads
 *  the finding's page as a path from the repository's root. */
export function annotation(finding: Finding): string {
  const properties = [
    `file=${escapeProperty(finding.page)}`,
    `line=${finding.line}`,
    `title=${escapeProperty(finding.title)}`,
  ].join(',');
  return `::${finding.level} ${properties}::${escapeData(finding.message)}`;
}

/** A message may hold anything except what ends the command: a line break. `%` is escaped
 *  first, so the escapes themselves are read back as written. */
function escapeData(text: string): string {
  return text.replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
}

/** A property's value also ends at `:` (the start of the message) and `,` (the next
 *  property). */
function escapeProperty(text: string): string {
  return escapeData(text).replaceAll(':', '%3A').replaceAll(',', '%2C');
}
