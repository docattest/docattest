#!/usr/bin/env node
// Extra cases for `check --base <ref>`, beyond the guard scenarios of tests/bench/scenarios.ts:
// each change type for each attestation kind, pages renamed, deleted, moved and frozen,
// sections reordered, claims reworded, symbols renamed, text as well as JSON, page filters,
// bases without a config, and every user error, following the rules in docs/changes.md.
//
// Each case is a throwaway git repository: `base` is committed, `head` is written over it
// (null deletes a file), then the candidate runs the arguments there and must print the
// bytes, and exit with the code, recorded in tests/parity/recordings/changes/
// (tests/parity/recordings.ts says where recordings come from).
//
// The cases are in tests/parity/changes/, by what they cover; this file runs them in the order
// below, which is the order they are recorded and printed in.
//
//   node tests/parity/changes.ts --candidate "<command line>" [--only <text>] [--jobs 6] [--update]
import { execFile, execFileSync } from 'node:child_process';
import { mkdtempSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { commandLine } from '../../scripts/lib/command-line.ts';
import { FIXED_GIT_DATES, gitEnv, writeFiles } from '../lib/project.ts';
import { pageFilters, several, theBase, userErrors } from './changes/command.ts';
import { downgrades, frozen, weakened } from './changes/downgrades.ts';
import { rewording, unchanged } from './changes/edits.ts';
import type { ChangeCase } from './changes/fixtures.ts';
import { orphaned } from './changes/orphaned.ts';
import { clockEnv } from './clock-env.ts';
import { HARNESS_OPTIONS, harnessOptions } from './harness-options.ts';
import type { Step } from './recording-format.ts';
import { explain, recordings } from './recordings.ts';

const USAGE =
  'usage: node tests/parity/changes.ts --candidate "<command line>" [--only <text>] ' +
  '[--jobs 6] [--update --source "<what changed>"] [--recordings <dir>] [--show <text>]';
const here = dirname(fileURLToPath(import.meta.url));
const { values } = commandLine(
  {
    options: {
      ...HARNESS_OPTIONS,
      jobs: { type: 'string', default: '6' },
      show: { type: 'string' },
    },
  },
  USAGE,
);
const { candidate, update, dir, only, source } = harnessOptions(
  values,
  resolve(here, 'recordings/changes'),
);
const jobs = Number(values.jobs);

const GIT_ENV = { ...gitEnv('parity'), ...FIXED_GIT_DATES };

const cases: ChangeCase[] = [
  ...unchanged,
  ...orphaned,
  ...rewording,
  ...frozen,
  ...downgrades,
  ...weakened,
  ...several,
  ...pageFilters,
  ...theBase,
  ...userErrors,
];

// ---------------------------------------------------------------------------------------
// Running
const git = (cwd: string, ...args: string[]): void => {
  execFileSync('git', args, { cwd, env: GIT_ENV, stdio: 'ignore' });
};
function commit(repository: string): void {
  git(repository, 'add', '-A');
  git(repository, 'commit', '-q', '-m', 'commit');
}

/** The repository of a case, and the project inside it. */
function setUp(c: ChangeCase): { repository: string; root: string } {
  const repository = mkdtempSync(join(tmpdir(), 'docattest-changes-'));
  const root = c.inside ? join(repository, c.inside) : repository;
  writeFiles(root, c.base);
  if (c.outside) writeFiles(repository, c.outside);
  if (c.git !== false) {
    git(repository, 'init', '-q', '-b', 'main');
    commit(repository);
    for (const files of c.commits ?? []) {
      writeFiles(root, files);
      commit(repository);
    }
  }
  writeFiles(root, c.head);
  return { repository, root };
}

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
  env: Record<string, string> = {},
): Promise<Run> {
  const [program = '', ...rest] = command;
  return new Promise((done) => {
    execFile(
      program,
      [...rest, ...args],
      { cwd, env: { ...GIT_ENV, ...env }, encoding: 'utf8', maxBuffer: 1 << 28 },
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
  cases.map((c) => c.name),
  { update, source },
);

/** Every run of a case, in its own repository, as the recordings hold them. */
async function steps(c: ChangeCase): Promise<Step[]> {
  const { repository, root } = setUp(c);
  const cwd = c.cwd === 'repository' ? repository : root;
  const real = realpathSync(repository);
  const norm = (t: string): string => t.split(real).join('<root>').split(repository).join('<root>');
  const out: Step[] = [];
  try {
    for (const args of c.runs) {
      const result = await run(
        candidate,
        args.map((x) => x.replace('__ROOT__', root)),
        cwd,
        { ...clockEnv(recorded.recordedAt), ...c.env },
      );
      // An internal error's stack names the implementation's own files; keep its first two lines.
      const stderr =
        result.code === 2
          ? norm(result.stderr).split('\n').slice(0, 2).join('\n')
          : norm(result.stderr);
      out.push({ label: args.join(' '), code: result.code, stdout: norm(result.stdout), stderr });
    }
  } finally {
    rmSync(repository, { recursive: true, force: true });
  }
  return out;
}

/** Every case's steps, `jobs` cases at a time, in the cases' order. */
async function runAll(chosen: readonly ChangeCase[]): Promise<Step[][]> {
  const results: Step[][] = new Array<Step[]>(chosen.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: jobs }, async () => {
      for (let k = next++; k < chosen.length; k = next++) {
        const c = chosen[k];
        if (c) results[k] = await steps(c);
      }
    }),
  );
  return results;
}

const chosen = cases.filter((c) => !only || c.name.includes(only));
const results = await runAll(chosen);
const seen = { changes: new Set<string>(), codes: new Set<Step['code']>() };
let [identical, total] = [0, 0];
const { show } = values;
chosen.forEach((c, k) => {
  const mine = results[k] ?? [];
  for (const s of mine) {
    seen.codes.add(s.code);
    for (const m of s.stdout.matchAll(/(orphaned-claim|frozen-page|downgrade|weakened)/g)) {
      seen.changes.add(m[1] ?? '');
    }
    if (show !== undefined && c.name.includes(show)) {
      console.log(`${c.name}: ${s.label}\n${s.stdout}${s.stderr}`);
    }
  }
  total += mine.length;
  const result = recorded.check(c.name, mine);
  identical += result.steps;
  if (!result.same) explain(c.name, result);
});
const complete = recorded.finish({ all: !only });
console.log(
  `the candidate saw: ${[...seen.changes].sort().join(', ')}; exit codes ${[...seen.codes].map(String).sort().join(', ')}`,
);
console.log(
  `changes parity: ${identical}/${total} identical to the recordings (${chosen.length} cases)`,
);
process.exitCode = complete && identical === total ? 0 : 1;
