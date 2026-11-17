import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { delimiter, dirname, join } from 'node:path';
import process from 'node:process';
import type { Report } from '../adapters/typescript/src/api/report.ts';

const INSTALL_GUIDE = 'https://github.com/docattest/docattest#install';

/** A report is the whole of what `check --json` prints, and the largest real reports are
 *  far below this. */
const MAX_REPORT_BYTES = 256 * 1024 * 1024;

/** Runs `docattest check --json` in `directory` and returns its report. docattest is the one
 *  the project installed: its npm package (found in node_modules/.bin, as npx finds it) or
 *  any install on the PATH, so the action runs the version the project chose. `args` is
 *  written as on a command line. Throws when docattest gives no report. */
export function runCheck(directory: string, args: string): Report {
  const run = spawnSync(`docattest check --json ${args}`, {
    cwd: directory,
    shell: true,
    encoding: 'utf8',
    maxBuffer: MAX_REPORT_BYTES,
    env: { ...process.env, PATH: withPackageBins(directory, process.env.PATH ?? '') },
  });
  if (run.error) throw run.error;
  process.stderr.write(run.stderr);
  const report = run.status === 0 || run.status === 1 ? parseReport(run.stdout) : undefined;
  if (report) return report;
  throw new Error(
    `docattest check gave no report (exit code ${String(run.status)}). Is docattest ` +
      `installed in an earlier step? See ${INSTALL_GUIDE}`,
  );
}

/** The PATH with the node_modules/.bin of the directory and of each directory above it in
 *  front, nearest first. */
function withPackageBins(directory: string, path: string): string {
  const bins: string[] = [];
  for (let at = directory; ; at = dirname(at)) {
    const bin = join(at, 'node_modules', '.bin');
    if (existsSync(bin)) bins.push(bin);
    if (dirname(at) === at) break;
  }
  return [...bins, path].join(delimiter);
}

/** The report, when the output is one: an object with its results and its exit code. The
 *  action reads nothing else that docattest does not always write. */
function parseReport(output: string): Report | undefined {
  let value: unknown;
  try {
    value = JSON.parse(output);
  } catch {
    return undefined;
  }
  return isReport(value) ? value : undefined;
}

function isReport(value: unknown): value is Report {
  return (
    typeof value === 'object' &&
    value !== null &&
    'results' in value &&
    Array.isArray(value.results) &&
    'exitCode' in value &&
    (value.exitCode === 0 || value.exitCode === 1)
  );
}
