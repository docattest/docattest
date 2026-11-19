import type { NodeTestLine } from './node-test-reporter.js';
import type { Ended, ReportedTest, RunnerReport } from './outcome.js';

/**
 * What `node --test <file>` reported, from the lines its reporter wrote (`node-test-reporter.ts`).
 * A test's path is rebuilt from the tests started around it: Node reports a test's start, and
 * then its end, in the order the file declares them, each with how deep it is nested. Node
 * reports a file that cannot run as a failed test named by the path it was given, with what
 * the file printed to stderr.
 */
export function readNodeTestReport(text: string, file: string): RunnerReport {
  const open: string[] = [];
  const tests: ReportedTest[] = [];
  const stderr: string[] = [];
  let fileFailed = false;
  for (const line of text.split('\n').filter(Boolean)) {
    const read = JSON.parse(line) as NodeTestLine;
    if (read.event === 'stderr') stderr.push(read.message);
    else if (read.event === 'start') {
      open.length = read.nesting;
      open.push(read.name);
    } else if (read.event === 'fail' && read.nesting === 0 && read.name === file) fileFailed = true;
    else if (!read.suite) {
      tests.push({ path: [...open.slice(0, read.nesting), read.name], ended: endedOf(read) });
    }
  }
  return { tests, fileFailure: fileFailed ? stderr.join('') || 'it failed' : undefined };
}

function endedOf(line: Extract<NodeTestLine, { event: 'pass' | 'fail' }>): Ended {
  if (line.todo !== false) {
    const reason = typeof line.todo === 'string' ? `: ${line.todo}` : '';
    return { status: 'skipped', reason: `it is marked todo${reason}` };
  }
  if (line.skip !== false) {
    return { status: 'skipped', reason: typeof line.skip === 'string' ? line.skip : undefined };
  }
  if (line.event === 'pass') return { status: 'passed' };
  return { status: 'failed', message: line.message ?? '' };
}
