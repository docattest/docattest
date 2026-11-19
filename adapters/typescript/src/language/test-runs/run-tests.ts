import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import type { CitedTest, DeclaredTest, TestRun } from '../../api/adapter.js';
import { diskFiles } from '../../files/disk.js';
import type { FileSource } from '../../files/source.js';
import { findTests, type FoundTest } from '../citations/tests-in-file.js';
import { pagePath } from '../languages.js';
import { invocationOf, type NodeRuntime, type Runner } from './invocation.js';
import { readJsonReport } from './json-report.js';
import { readNodeTestReport } from './node-test-report.js';
import { outcomeOf, pathText, type RunnerReport } from './outcome.js';
import { runProcess } from './run-process.js';
import { chooseRunner, RUNNER_NAMES } from './runner-choice.js';

/** What every test of an in-memory project answers: there is nothing on disk to run. */
const IN_MEMORY: TestRun = {
  status: 'notRun',
  message: 'the project is in memory (open.files), so none of its tests can run',
};

const NODE: NodeRuntime = {
  execPath: process.execPath,
  stripsTypes: process.features.typescript !== false,
  reporter: new URL('./node-test-reporter.js', import.meta.url).href,
};

/** How long one run of a runner may take, and how many run at once. */
export interface RunLimits {
  timeoutMs: number;
  workers: number;
}

/**
 * Run cited tests with the project's own runners and say how each ran, in order. The tests
 * of one file share one run of its runner, and up to `workers` runs go at once.
 */
export async function runCitedTests(
  files: FileSource,
  tests: readonly CitedTest[],
  limits: RunLimits,
): Promise<TestRun[]> {
  if (files.memory) return tests.map(() => IN_MEMORY);
  const byFile = new Map<string, Numbered[]>();
  tests.forEach((test, k) => byFile.set(test.file, [...(byFile.get(test.file) ?? []), [k, test]]));
  const answers: TestRun[] = [];
  await atMost(limits.workers, [...byFile], async ([file, group]) => {
    for (const [k, run] of await runFile(files, file, group, limits.timeoutMs)) answers[k] = run;
  });
  return answers;
}

/** A cited test, or its run, with its place in the request. */
type Numbered<T = CitedTest> = readonly [number, T];

// Each test of `file` is selected by the paths its runner reports it by.
async function runFile(
  files: FileSource,
  file: string,
  group: readonly Numbered[],
  timeoutMs: number,
): Promise<Numbered<TestRun>[]> {
  const code = files.read(file);
  if (code === undefined) return group.map(([k]) => [k, notRun(`${file} does not exist`)]);
  const fileName = pagePath(files, file);
  const found = findTests(fileName, code);
  const choice = chooseRunner(files, file, found.runner);
  if (!choice.ok) return group.map(([k]) => [k, notRun(choice.message)]);
  const selections = group.map(([k, test]) => [k, selectionOf(found.tests, test, file)] as const);
  const paths = selections.flatMap(([, s]) => (Array.isArray(s) ? s : []));
  const prefixes = selections.flatMap(([, s]) => (isRows(s) ? [s.under] : []));
  const end: RunEnd =
    paths.length || prefixes.length
      ? await runSelected(choice.runner, fileName, { paths, prefixes }, timeoutMs)
      : { report: { tests: [], fileFailure: undefined } };
  const name = RUNNER_NAMES[choice.runner.kind];
  return selections.map(([k, selection]) => [k, answerOf(selection, end, name)]);
}

/**
 * How a cited test is selected: by the full paths its runner reports it by, as the rows of
 * a table whose titles the file does not tell, or not at all, with why.
 */
type Selection = string[][] | TableRows | TestRun;

/**
 * A table cited by its title as written whose rows' titles are only known when it runs: its
 * rows are the tests the runner reports directly under its describe titles, but for the
 * file's other tests there, whose titles are known.
 */
interface TableRows {
  under: string[];
  others: string[][];
  named: string;
}

const isRows = (selection: Selection): selection is TableRows =>
  !Array.isArray(selection) && 'under' in selection;

// How a cited test is selected, from the file's own listing.
function selectionOf(declared: readonly FoundTest[], test: CitedTest, file: string): Selection {
  const sameDescribes = (d: DeclaredTest) =>
    d.describes.length === test.describes.length &&
    d.describes.every((title, k) => title === test.describes[k]);
  const found = declared.find((d) => d.title === test.title && sameDescribes(d));
  const named = pathText([...test.describes, test.title]);
  if (!found) return notRun(`${file} declares no test named ${named}`);
  if (found.reported) return found.reported;
  const others = declared
    .filter((d) => d !== found && sameDescribes(d))
    .flatMap((d) => d.reported ?? []);
  return { under: test.describes, others, named };
}

// How one cited test ran, from what its file's run reported.
function answerOf(selection: Selection, end: RunEnd, runner: string): TestRun {
  if (!Array.isArray(selection) && !isRows(selection)) return selection;
  if ('run' in end) return end.run;
  if (!isRows(selection)) return outcomeOf(end.report, selection, runner);
  const rows = end.report.tests
    .map((t) => t.path)
    .filter((p) => p.length === selection.under.length + 1 && startsWith(p, selection.under))
    .filter((p) => !selection.others.some((other) => samePath(other, p)));
  if (!rows.length) return notRun(`${runner} ran no row of the table ${selection.named}`);
  return outcomeOf(end.report, rows, runner);
}

const samePath = (a: readonly string[], b: readonly string[]) =>
  a.length === b.length && startsWith(a, b);

const startsWith = (path: readonly string[], prefix: readonly string[]) =>
  path.length >= prefix.length && prefix.every((title, k) => path[k] === title);

/** A run's report, or the one answer every test of the run gets when there is none. */
type RunEnd = { report: RunnerReport } | { run: TestRun };

/** The tests a run selects: by full path, and every test under some describe titles. */
interface Selected {
  paths: readonly string[][];
  prefixes: readonly string[][];
}

async function runSelected(
  runner: Runner,
  fileName: string,
  selected: Selected,
  timeoutMs: number,
): Promise<RunEnd> {
  const name = RUNNER_NAMES[runner.kind];
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'docattest-tests-'));
  try {
    // Runners report a file by its real path (macOS's /var is /private/var).
    const file = fs.realpathSync.native(fileName);
    const request = { file, ...selected, reportFile: path.join(dir, 'report') };
    const end = await runProcess(invocationOf(runner, request, NODE), timeoutMs);
    if (end.kind === 'timedOut') {
      const message = `${name} did not finish within ${timeoutMs / 1000} s`;
      return { run: { status: 'failed', message } };
    }
    const text = diskFiles(dir).read('report');
    const report =
      text === undefined
        ? undefined
        : runner.kind === 'node:test'
          ? readNodeTestReport(text, file)
          : readJsonReport(text, file);
    const nothing = `${name} reported nothing of the file: ${tail(end.output)}`;
    return report ? { report } : { run: notRun(nothing) };
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

const notRun = (message: string): TestRun => ({ status: 'notRun', message });

// The last lines a runner printed, which say why it stopped.
function tail(output: string): string {
  const lines = output.split('\n').filter((line) => line.trim() !== '');
  return lines.slice(-10).join('\n') || 'it printed nothing';
}

// `work` on each item, at most `limit` at once.
async function atMost<T>(
  limit: number,
  items: readonly T[],
  work: (item: T) => Promise<void>,
): Promise<void> {
  const queue = items.values();
  const lane = async () => {
    for (const item of queue) await work(item);
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, lane));
}
