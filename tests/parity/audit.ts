#!/usr/bin/env node
// Extra cases for `audit` and `inventory` in the Rust core, beyond tests/bench/scenarios.ts: every
// target kind, ties, thresholds at their boundaries, the adapter's walk at its bounds,
// non-ASCII items, CRLF pages, bound and frozen pages, several surfaces, surfaces that do not
// load, empty results and searches. Each case is a small project on disk; the candidate runs
// the arguments there and must print the bytes, and exit with the code, recorded in
// tests/parity/recordings/audit/ (tests/parity/recordings.ts says where recordings come from).
// A case without arguments runs `audit`, `audit --json`, `inventory` and `inventory --json`.
//
// The cases are in tests/parity/audit/, by what they cover; this file runs them in the order
// below, which is the order they are recorded and printed in.
//
//   node tests/parity/audit.ts --candidate "<command line>" [--only <text>] [--update] [--show]
import { execFile } from 'node:child_process';
import { mkdtempSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { commandLine } from '../../scripts/lib/command-line.ts';
import { writeFiles, type ProjectFiles } from '../lib/project.ts';
import { ALL, type AuditCase } from './audit/fixtures.ts';
import { inventory } from './audit/inventory.ts';
import { kinds, suggestions, unitTests } from './audit/matching.ts';
import { boundAndFrozen, columns, crlf, nonAscii } from './audit/pages.ts';
import { evaluatedAgain, failingSurfaces, severalSurfaces } from './audit/surfaces.ts';
import { thresholds, ties, walk } from './audit/thresholds.ts';
import { clockEnv } from './clock-env.ts';
import { HARNESS_OPTIONS, harnessOptions } from './harness-options.ts';
import type { Step } from './recording-format.ts';
import { explain, recordings } from './recordings.ts';

const USAGE =
  'usage: node tests/parity/audit.ts --candidate "<command line>" [--only <text>] ' +
  '[--update --source "<what changed>"] [--recordings <dir>] [--show]';
const here = dirname(fileURLToPath(import.meta.url));
const { values } = commandLine(
  { options: { ...HARNESS_OPTIONS, show: { type: 'boolean', default: false } } },
  USAGE,
);
const { candidate, update, dir, only, source } = harnessOptions(
  values,
  resolve(here, 'recordings/audit'),
);

// [name, files, argument lists (default: all four)]
const cases: AuditCase[] = [
  ...kinds,
  ...unitTests,
  ...suggestions,
  ...ties,
  ...thresholds,
  ...walk,
  ...nonAscii,
  ...crlf,
  ...boundAndFrozen,
  ...columns,
  ...severalSurfaces,
  ...failingSurfaces,
  ...evaluatedAgain,
  ...inventory,
];

/** What the candidate did: its exit code (-1 when it has none) and its output. */
interface Run {
  code: number;
  stdout: string;
  stderr: string;
}

function run(
  command: readonly string[],
  args: readonly string[],
  cwd: string,
  env: Record<string, string>,
): Promise<Run> {
  const [program = '', ...rest] = command;
  return new Promise((done) => {
    execFile(
      program,
      [...rest, ...args],
      { cwd, env: { ...process.env, ...env }, encoding: 'utf8', maxBuffer: 1 << 28 },
      (error, stdout, stderr) => {
        done({
          code: error ? (typeof error.code === 'number' ? error.code : -1) : 0,
          stdout,
          stderr,
        });
      },
    );
  });
}

const recorded = recordings(
  dir,
  cases.map(([name]) => name),
  { update, source },
);

/** One run of a case, in a fresh project, as the recordings hold it. */
async function step(files: ProjectFiles<string>, args: readonly string[]): Promise<Step> {
  const root = mkdtempSync(join(tmpdir(), 'docattest-audit-'));
  try {
    writeFiles(root, files);
    const realArgs = args.map((x) => x.replace('__ROOT__', root));
    const out = await run(candidate, realArgs, root, clockEnv(recorded.recordedAt));
    const real = realpathSync(root);
    const norm = (t: string): string => t.split(real).join('<root>').split(root).join('<root>');
    // An internal error's stack names the implementation's own files; keep its first two lines.
    const head = (t: string): string => (out.code === 2 ? t.split('\n').slice(0, 2).join('\n') : t);
    return {
      label: args.join(' '),
      code: out.code,
      stdout: norm(out.stdout),
      stderr: head(norm(out.stderr)),
    };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

/** Run every job, four at a time, and return their steps in the jobs' order. */
async function runAll(
  jobs: readonly (readonly [ProjectFiles<string>, readonly string[]])[],
): Promise<Step[]> {
  const steps: Step[] = new Array<Step>(jobs.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: 4 }, async () => {
      for (let at = next++; at < jobs.length; at = next++) {
        const [files, args] = jobs[at] ?? [{}, []];
        steps[at] = await step(files, args);
      }
    }),
  );
  return steps;
}

const chosen = cases.filter(([name]) => !only || name.includes(only));
const steps = await runAll(
  chosen.flatMap(([, files, runs = ALL]) => runs.map((args) => [files, args] as const)),
);
const tally = { exit0: 0, exit1: 0, other: 0, suggestions: 0 };
let [identical, offset] = [0, 0];
for (const [name, , runs = ALL] of chosen) {
  const mine = steps.slice(offset, offset + runs.length);
  offset += runs.length;
  for (const s of mine) {
    tally[s.code === 0 ? 'exit0' : s.code === 1 ? 'exit1' : 'other']++;
    if (s.label.startsWith('audit') && /"directive"| {2}\? L/.test(s.stdout)) tally.suggestions++;
    if (values.show) {
      console.log(`--- ${name}: ${s.label} (exit ${String(s.code)})\n${s.stdout}${s.stderr}`);
    }
  }
  const result = recorded.check(name, mine);
  identical += result.steps;
  if (!result.same) explain(name, result);
}
const complete = recorded.finish({ all: !only });
console.log(
  `audit parity: ${identical}/${steps.length} identical to the recordings (${tally.exit0} exit 0, ${tally.exit1} exit 1, ${tally.other} other; ${tally.suggestions} audits with suggestions)`,
);
process.exitCode = complete && identical === steps.length ? 0 : 1;
