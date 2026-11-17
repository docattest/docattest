// The action's entry point (action.yml): check the project, mark each finding on its line,
// write the run's summary, and exit as docattest did.
import { appendFileSync } from 'node:fs';
import { relative, resolve, sep } from 'node:path';
import process from 'node:process';
import { annotation } from './annotation.ts';
import { findings } from './findings.ts';
import { runCheck } from './runCheck.ts';
import { jobSummary, verdict } from './summary.ts';

try {
  const workspace = process.env.GITHUB_WORKSPACE ?? process.cwd();
  const directory = resolve(workspace, input('working-directory') || '.');
  const report = runCheck(directory, input('args'));
  const found = findings(report, relative(workspace, directory).split(sep).join('/'));
  for (const finding of found) process.stdout.write(`${annotation(finding)}\n`);
  process.stdout.write(`${verdict(report)}\n`);
  const summaryFile = process.env.GITHUB_STEP_SUMMARY;
  if (summaryFile) appendFileSync(summaryFile, jobSummary(report, found));
  process.exitCode = report.exitCode;
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stdout.write(`::error title=docattest::${message}\n`);
  process.exitCode = 1;
}

/** An input of action.yml, as the runner passes it: in INPUT_<NAME>. */
function input(name: string): string {
  return (process.env[`INPUT_${name.toUpperCase()}`] ?? '').trim();
}
