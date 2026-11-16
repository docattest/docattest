#!/usr/bin/env node
// Extra cases for `impact` (and `impact --file`), the two hook modes for every shipped agent
// and for one nobody ships, `hooks install` (also `--git`), `agents install`, `skill
// install`, finding the project root, and `refresh`, beyond tests/bench/scenarios.ts. Each case
// builds a project in a temporary directory (with a temporary home directory), runs the
// candidate there step by step, and compares what each step prints, its exit code, and every
// file under the project and the home directory that the step changed, with
// tests/parity/recordings/impact/ (tests/parity/recordings.ts says where recordings come from).
//
// Nothing here touches the network: `refresh` talks to an HTTP server this script starts on
// a local port. Nothing is written outside the temporary directories.
//
// The cases are in tests/parity/impact/, by what they cover; this file runs them in the order
// below, which is the order they are recorded and printed in.
//
//   node tests/parity/impact.ts --candidate "<command line>" [--only <text>] [--update]
import process from 'node:process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { commandLine } from '../../scripts/lib/command-line.ts';
import { ROOT } from '../../scripts/lib/repository.ts';
import { FIXED_GIT_DATES, gitEnv } from '../lib/project.ts';
import { agentsInstall, otherAgents } from './impact/agents.ts';
import { checkHook, impactHook } from './impact/hook-modes.ts';
import { hooksInstall, hooksInstallGit } from './impact/hooks-install.ts';
import { findingTheRoot, impactFile } from './impact/impact-file.ts';
import { impact } from './impact/impact.ts';
import { refresh } from './impact/refresh.ts';
import { side } from './impact/run.ts';
import { server } from './impact/server.ts';
import { skillInstall } from './impact/skill.ts';
import type { ImpactCase } from './impact/types.ts';
import { clockEnv } from './clock-env.ts';
import { HARNESS_OPTIONS, harnessOptions } from './harness-options.ts';
import type { Step } from './recording-format.ts';
import { explain, recordings, type Recordings } from './recordings.ts';

const USAGE =
  'usage: node tests/parity/impact.ts --candidate "<command line>" [--only <text>] ' +
  '[--update --source "<what changed>"] [--recordings <dir>] [--verbose]';
const here = dirname(fileURLToPath(import.meta.url));
const { values } = commandLine(
  { options: { ...HARNESS_OPTIONS, verbose: { type: 'boolean', default: false } } },
  USAGE,
);
const { candidate, update, dir, only, source } = harnessOptions(
  values,
  resolve(here, 'recordings/impact'),
);
const adapter = `${process.execPath} ${join(ROOT, 'adapters/typescript/dist/protocol/main.js')}`;

const ENV: NodeJS.ProcessEnv = {
  ...gitEnv('parity'),
  ...FIXED_GIT_DATES,
  DOCATTEST_ADAPTER_TYPESCRIPT: process.env.DOCATTEST_ADAPTER_TYPESCRIPT ?? adapter,
  // The bare binary is told what the npm launcher would tell it: how a project that
  // installed docattest from npm runs it. `hooks install` writes that command.
  DOCATTEST_COMMAND: 'npx --no docattest',
};

const cases: readonly ImpactCase[] = [
  ...impact,
  ...impactHook,
  ...checkHook,
  ...hooksInstall,
  ...impactFile,
  ...findingTheRoot,
  ...otherAgents,
  ...agentsInstall,
  ...hooksInstallGit,
  ...skillInstall,
  ...refresh,
];

// ---------------------------------------------------------------------------------------
// Running and comparing.
// ---------------------------------------------------------------------------------------

/** The group a case is counted in: its first word, or two for commands with a mode. */
function groupOf(name: string): string {
  const twoWords =
    name.startsWith('impact --hook') ||
    name.startsWith('check --hook') ||
    /^(hooks|skill|agents) /.test(name) ||
    name.startsWith('impact --file');
  return name
    .split(/[ :,]/)
    .slice(0, twoWords ? 2 : 1)
    .join(' ');
}

function printSteps(name: string, steps: readonly Step[]): void {
  for (const step of steps) {
    console.log(
      `  ${name}: ${step.label}  → exit ${String(step.code)}\n    ${JSON.stringify(step.stdout).slice(0, 300)}\n    ${JSON.stringify(step.stderr).slice(0, 300)}`,
    );
  }
}

/** Steps identical to the recordings, of all steps run, overall and by group. */
interface Tally {
  same: number;
  total: number;
  groups: Record<string, [number, number]>;
}

/** Run the bare candidate on every case chosen, and compare it with the recordings. */
async function runCases(recorded: Recordings, env: NodeJS.ProcessEnv): Promise<Tally> {
  const tally: Tally = { same: 0, total: 0, groups: {} };
  const { verbose } = values;
  for (const testCase of cases) {
    if (only && !testCase.name.includes(only)) continue;
    const steps = await side(candidate, testCase, env);
    const group = (tally.groups[groupOf(testCase.name)] ??= [0, 0]);
    const result = recorded.check(testCase.name, steps);
    tally.total += steps.length;
    group[1] += steps.length;
    tally.same += result.steps;
    group[0] += result.steps;
    if (verbose) printSteps(testCase.name, steps);
    if (!result.same) explain(testCase.name, result);
  }
  return tally;
}

// The same core as a project that installed docattest from npm runs it: through the npm
// launcher, which is told nothing here. It finds the adapter itself and says what command a
// hook should run, so what `hooks install` writes must still be what is recorded, and a
// check and an impact must still find the TypeScript adapter. These
// runs are only ever compared: the recordings are the bare core's.
async function throughTheLauncher(
  recorded: Recordings,
  clocked: NodeJS.ProcessEnv,
): Promise<[number, number]> {
  const launcher = [
    process.execPath,
    resolve(here, '../../adapters/typescript/dist/launcher/main.js'),
  ];
  const launched: NodeJS.ProcessEnv = { ...clocked, DOCATTEST_CORE: resolve(candidate[0] ?? '') };
  delete launched.DOCATTEST_ADAPTER_TYPESCRIPT;
  delete launched.DOCATTEST_COMMAND;
  const firstOf = (prefix: string): ImpactCase[] =>
    cases.filter((c) => c.name.startsWith(prefix)).slice(0, 1);
  const chosen = [
    ...cases.filter(
      (c) => c.name.startsWith('hooks install') || c.name.startsWith('agents install'),
    ),
    ...firstOf('impact --hook'),
    ...firstOf('check --hook'),
    ...firstOf('impact'),
    ...firstOf('impact --file'),
    ...firstOf('root found'),
  ];
  let [same, total] = [0, 0];
  for (const testCase of candidate.length === 1 && !only ? chosen : []) {
    const steps = await side(launcher, testCase, launched);
    const result = recorded.check(testCase.name, steps, { write: false });
    total += steps.length;
    same += result.steps;
    if (!result.same) explain(`through the npm launcher: ${testCase.name}`, result);
  }
  return [same, total];
}

const recorded = recordings(
  dir,
  cases.map((c) => c.name),
  { update, source },
);
const clocked = { ...ENV, ...clockEnv(recorded.recordedAt) };
const { same, total, groups } = await runCases(recorded, clocked);
const [launchedSame, launchedTotal] = await throughTheLauncher(recorded, clocked);
server.close();
const complete = recorded.finish({ all: !only });
for (const [group, [ok, n]] of Object.entries(groups)) console.log(`  ${group}: ${ok}/${n}`);
console.log(`  through the npm launcher: ${launchedSame}/${launchedTotal}`);
console.log(`impact parity: ${same}/${total} identical to the recordings`);
process.exitCode = complete && same === total && launchedSame === launchedTotal ? 0 : 1;
