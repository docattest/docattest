#!/usr/bin/env node
// Parity: run the docattest core on every benchmark scenario and on real projects, and
// compare what it prints with recorded outputs (tests/parity/recordings.ts says where they
// come from); a deliberate change of behaviour is recorded again with --update, which says
// what changed.
//
//   node tests/parity/bench.ts --candidate "<command line>" [--project <dir> [--recordings <dir>]]...
//                         [--runs 5] [--update] [--only <text>] [--max-ratio <x>]
//
// The scenarios' recordings are in tests/parity/recordings/scenarios/, this repository's (as a
// project) in tests/parity/recordings/project/. A project outside the repository keeps its
// recordings outside it too: name their directory with --recordings right after its
// --project. Each project's recordings directory also holds timing.json, the medians of
// `check`, `index` and `audit` on that project when it was recorded, by what ran them; every
// run is compared with the `reference` medians, and `--record-timing <label>` stores this run's
// medians under that label instead. Exit code 1 when any output or exit code differs from
// its recording, or (with --max-ratio) when a median is more than that many times the
// reference's.
import { execFileSync, spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { commandLine } from '../../scripts/lib/command-line.ts';
import { ROOT } from '../../scripts/lib/repository.ts';
import { FIXED_GIT_DATES, gitEnv, writeFiles } from '../lib/project.ts';
import { scenarios } from '../bench/scenarios.ts';
import type { Scenario } from '../bench/scenario.ts';
import { clockEnv } from './clock-env.ts';
import { recordedFrom } from './harness-options.ts';
import type { Step } from './recording-format.ts';
import { explain, recordings, type Recordings } from './recordings.ts';

const USAGE =
  'usage: node tests/parity/bench.ts --candidate "<command line>" ' +
  '[--project <dir> [--recordings <dir>]]... [--runs 5] [--update --source "<what changed>"] ' +
  '[--only <text>] [--max-ratio <x>] [--record-timing <label>]';
const here = dirname(fileURLToPath(import.meta.url));
const { values, tokens } = commandLine(
  {
    tokens: true,
    options: {
      candidate: { type: 'string' },
      project: { type: 'string', multiple: true },
      recordings: { type: 'string', multiple: true },
      runs: { type: 'string', default: '5' },
      update: { type: 'boolean', default: false },
      only: { type: 'string' },
      'max-ratio': { type: 'string' },
      'record-timing': { type: 'string' },
      source: { type: 'string' },
    },
  },
  USAGE,
);
const candidate = (values.candidate ?? '').split(' ').filter(Boolean);
const runs = Number(values.runs);
const { update, only } = values;
const maxRatio = values['max-ratio'] === undefined ? undefined : Number(values['max-ratio']);
const timingLabel = values['record-timing'];
const source = recordedFrom(values.source, update);
const [program, ...programArgs] = candidate;
if (program === undefined) throw new Error('pass --candidate "<command line>"');
const command: string = program;

/** A project given with --project, and where its recordings are. */
interface Project {
  dir: string;
  recordings: string;
}

/** Each --project, with the --recordings that follows it, if any. */
const options = tokens.filter((token) => token.kind === 'option');
const projects = options.flatMap((token, k): Project[] => {
  const previous = options[k - 1];
  if (token.name === 'recordings' && previous?.name !== 'project') {
    throw new Error(
      '--recordings <dir>: name it right after the --project it is the recordings of',
    );
  }
  if (token.name !== 'project') return [];
  const dir = resolve(token.value);
  const next = options[k + 1];
  const given = next?.name === 'recordings' ? resolve(next.value) : undefined;
  if (!given && dir !== resolve(ROOT)) {
    throw new Error(
      `--project ${dir}: name where its recordings are with --recordings <dir> right after it ` +
        '(recordings of a project outside this repository are kept outside it)',
    );
  }
  return [{ dir, recordings: given ?? join(here, 'recordings/project') }];
});

const PROJECT_COMMANDS = [
  ['check', '--json'],
  ['check'],
  ['where', '--json'],
  ['uncovered', '--json'],
  ['audit', '--json'],
  ['audit', '--inventory', '--json'],
];
const TIMED = [
  ['check', '--json'],
  ['where', '--json'],
  ['audit', '--json'],
];
const GIT_ENV = { ...gitEnv('bench'), ...FIXED_GIT_DATES };

// What a project's own examples need to run docattest themselves (this repository's docs
// call `createProject` and the launcher): the candidate, when it is a core binary.
const CORE_ENV =
  programArgs.length === 0 && existsSync(command) ? { DOCATTEST_CORE: resolve(command) } : {};

/** One run of the candidate: what it printed, and how long it took. */
interface Run {
  code: number | null;
  stdout: string;
  stderr: string;
  ms: number;
}

function run(args: readonly string[], cwd: string, clock: string): Run {
  const started = process.hrtime.bigint();
  const out = spawnSync(command, [...programArgs, ...args], {
    cwd,
    env: { ...GIT_ENV, ...CORE_ENV, ...clockEnv(clock) },
    encoding: 'utf8',
    maxBuffer: 1 << 28,
  });
  const ms = Number(process.hrtime.bigint() - started) / 1e6;
  return { code: out.status, stdout: out.stdout, stderr: out.stderr, ms };
}

// Temporary directories differ between two runs of a scenario; nothing else may.
function step(args: readonly string[], cwd: string, clock: string): Step {
  const real = realpathSync(cwd);
  const norm = (text: string): string => text.split(real).join('<root>').split(cwd).join('<root>');
  const out = run(args, cwd, clock);
  return {
    label: args.join(' '),
    code: out.code,
    stdout: norm(out.stdout),
    stderr: norm(out.stderr),
  };
}

function scenarioDir(scenario: Scenario): string {
  const root = mkdtempSync(join(tmpdir(), 'docattest-parity-'));
  writeFiles(root, scenario.files);
  if (scenario.head) {
    for (const args of [
      ['init', '-q', '-b', 'main'],
      ['add', '-A'],
      ['commit', '-q', '-m', 'base'],
    ]) {
      execFileSync('git', args, { cwd: root, env: GIT_ENV, stdio: 'ignore' });
    }
    writeFiles(root, scenario.head);
  }
  return root;
}

const median = (values: readonly number[]): number =>
  [...values].sort((x, y) => x - y)[Math.floor(values.length / 2)] ?? NaN;

let [identical, total] = [0, 0];
let complete = true;
function compare(recorded: Recordings, name: string, steps: Step[]): void {
  const result = recorded.check(name, steps);
  total += steps.length;
  identical += result.steps;
  if (!result.same) explain(name, result);
}

const scenarioRecordings = recordings(
  join(here, 'recordings/scenarios'),
  scenarios.map((s) => s.id),
  { update, source },
);
for (const scenario of scenarios) {
  if (only && !scenario.id.includes(only)) continue;
  const root = scenarioDir(scenario);
  try {
    const clock = scenarioRecordings.recordedAt;
    const steps = [step(scenario.run, root, clock)];
    if (scenario.run[0] !== 'check') steps.push(step(['check', '--json'], root, clock));
    compare(scenarioRecordings, scenario.id, steps);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
complete = scenarioRecordings.finish({ all: !only }) && complete;

/** timing.json: medians in ms by what ran them (`reference`, `core`, or a label). */
interface Timing {
  medians?: Record<string, Record<string, number>>;
  [key: string]: unknown;
}

/** One line of the timing table, as console.table prints it. */
interface TimingRow {
  project: string;
  command: string;
  'reference ms (recorded)': number | string;
  'core ms (recorded)': number | string;
  'now ms': number;
  'now / reference': string;
}

/** This run's median of each timed command on `project`, and a row of the table for each. */
function time(project: Project, baseline: Timing, clock: string): Record<string, number> {
  const now: Record<string, number> = {};
  for (const args of TIMED) {
    const command = args.join(' ');
    const ms = Math.round(
      median(Array.from({ length: runs }, () => run(args, project.dir, clock).ms)),
    );
    now[command] = ms;
    const reference = baseline.medians?.reference?.[command];
    timings.push({
      project: project.dir.split('/').slice(-2).join('/'),
      command,
      'reference ms (recorded)': reference ?? '-',
      'core ms (recorded)': baseline.medians?.core?.[command] ?? '-',
      'now ms': ms,
      'now / reference': reference ? (ms / reference).toFixed(2) : '-',
    });
  }
  return now;
}

/** Store this run's medians under --record-timing's label. */
function recordTiming(
  file: string,
  baseline: Timing,
  now: Record<string, number>,
  label: string,
): void {
  const next = {
    ...baseline,
    note:
      'Median wall-clock ms of each command on this project, by what ran it, when the ' +
      'recordings were made. Fixed: tests/parity/bench.ts compares every run with them.',
    runs,
    machine: `${process.platform}-${process.arch}`,
    medians: { ...baseline.medians, [label]: now },
  };
  writeFileSync(file, `${JSON.stringify(next, null, 2)}\n`);
  console.log(`timing: recorded this run's medians as "${label}" in ${file}`);
}

const timings: TimingRow[] = [];
for (const project of projects) {
  const recorded = recordings(
    project.recordings,
    PROJECT_COMMANDS.map((args) => args.join(' ')),
    { update, source },
  );
  for (const args of PROJECT_COMMANDS) {
    if (only && !args.join(' ').includes(only)) continue;
    compare(recorded, args.join(' '), [step(args, project.dir, recorded.recordedAt)]);
  }
  complete = recorded.finish({ all: !only }) && complete;
  const timingFile = join(project.recordings, 'timing.json');
  // The repository's own file, written by this harness.
  const baseline = existsSync(timingFile)
    ? (JSON.parse(readFileSync(timingFile, 'utf8')) as Timing)
    : {};
  const now = time(project, baseline, recorded.recordedAt);
  if (timingLabel) recordTiming(timingFile, baseline, now, timingLabel);
}

console.log(`parity: ${identical}/${total} identical to the recordings`);
let slow = false;
if (timings.length > 0) {
  console.table(timings);
  for (const t of timings) {
    if (
      maxRatio !== undefined &&
      t['now / reference'] !== '-' &&
      Number(t['now / reference']) > maxRatio
    ) {
      console.log(
        `✗ ${t.project}: ${t.command} took ${t['now / reference']} times the reference's median`,
      );
      slow = true;
    }
  }
}
process.exitCode = complete && identical === total && !slow ? 0 : 1;
