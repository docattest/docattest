#!/usr/bin/env node
// Extra cases for the Rust core, beyond tests/bench/scenarios.ts: malformed configs, unclosed
// directives, tables in lists, CRLF, non-ASCII text, empty projects, page filters, the
// queries and the command line itself. Each case is a small project on disk; the candidate
// runs the arguments there and must print the bytes, and exit with the code, that are
// recorded in tests/parity/recordings/extra/ (tests/parity/recordings.ts says where
// recordings come from).
//
// The cases are in tests/parity/extra/: cases.ts, compared with the recordings, and
// differing.ts, whose recordings are the core's own and which also state what they show.
//
//   node tests/parity/extra.ts --candidate "<command line>" [--only <text>] [--update]
//
// `--update` records what the candidate prints instead, and says which cases changed.
import { spawnSync } from 'node:child_process';
import { chmodSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { commandLine } from '../../scripts/lib/command-line.ts';
import { writeFiles, type ProjectFiles } from '../lib/project.ts';
import { clockEnv } from './clock-env.ts';
import { cases } from './extra/cases.ts';
import { differing } from './extra/differing.ts';
import type { Run } from './extra/fixtures.ts';
import { HARNESS_OPTIONS, harnessOptions } from './harness-options.ts';
import type { Step } from './recording-format.ts';
import { explain, recordings } from './recordings.ts';

const USAGE =
  'usage: node tests/parity/extra.ts --candidate "<command line>" [--only <text>] ' +
  '[--update --source "<what changed>"] [--recordings <dir>]';
const here = dirname(fileURLToPath(import.meta.url));
const { values } = commandLine({ options: HARNESS_OPTIONS }, USAGE);
const { candidate, update, dir, only, source } = harnessOptions(
  values,
  resolve(here, 'recordings/extra'),
);

function run(
  command: readonly string[],
  args: readonly string[],
  cwd: string,
  env: Record<string, string>,
): Run {
  const [program = '', ...rest] = command;
  const out = spawnSync(program, [...rest, ...args], {
    cwd,
    env: { ...process.env, ...env },
    encoding: 'utf8',
  });
  return { code: out.status, stdout: out.stdout, stderr: out.stderr };
}

// Files under a `.bin/` directory are programs (the fake adapter).
function write(root: string, files: ProjectFiles<string>): void {
  writeFiles(root, files);
  for (const path of Object.keys(files).filter((file) => file.includes('/.bin/'))) {
    chmodSync(join(root, path), 0o755);
  }
}

const names = [...cases.map(([name]) => name), ...differing.map((d) => d.name)];
const recorded = recordings(dir, names, { update, source });
const ADAPTERS = { DOCATTEST_ADAPTER_PYTHON: '', DOCATTEST_ADAPTER_RUST_2024: '' };

/** Run one case in a fresh project and return the step as the recordings hold it. */
function outcome(
  files: ProjectFiles<string>,
  args: readonly string[],
  fake?: string,
): { raw: Run; step: Step } {
  const root = mkdtempSync(join(tmpdir(), 'docattest-extra-'));
  try {
    write(root, files);
    const realArgs = args.map((x) => x.replace('__ROOT__', root));
    const env = {
      ...ADAPTERS,
      ...clockEnv(recorded.recordedAt),
      DOCATTEST_ADAPTER_FAKE: fake ? JSON.stringify([join(root, fake)]) : '',
    };
    const out = run(candidate, realArgs, root, env);
    const real = realpathSync(root);
    const norm = (t: string): string => t.split(real).join('<root>').split(root).join('<root>');
    // An internal error's stack names the implementation's own files; keep its first two lines.
    const head = (t: string): string => (out.code === 2 ? t.split('\n').slice(0, 2).join('\n') : t);
    return {
      raw: out,
      step: {
        label: args.join(' '),
        code: out.code,
        stdout: norm(out.stdout),
        stderr: head(norm(out.stderr)),
      },
    };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

let [same, total] = [0, 0];
for (const [name, files, args] of cases) {
  if (only && !name.includes(only)) continue;
  total++;
  const result = recorded.check(name, [outcome(files, args).step]);
  if (result.same) same++;
  else explain(`${name}: ${args.join(' ')}`, result);
}
let [asDesigned, designed] = [0, 0];
for (const { name, files, args, candidate: wanted } of differing) {
  if (only && !name.includes(only)) continue;
  designed++;
  const { raw, step } = outcome(files, args, 'my tools/.bin/docattest-adapter-fake');
  if (!wanted(raw)) {
    console.log(
      `✗ ${name}: ${args.join(' ')} does not behave as designed, so it is not compared or recorded`,
    );
    console.log(
      `  exit ${String(raw.code)} ${JSON.stringify(raw.stdout + raw.stderr).slice(0, 400)}`,
    );
    continue;
  }
  const result = recorded.check(name, [step]);
  if (result.same) asDesigned++;
  else explain(`${name}: ${args.join(' ')}`, result);
}
const complete = recorded.finish({ all: !only });
console.log(`extra parity: ${same}/${total} identical to the recordings`);
console.log(`differing by design: ${asDesigned}/${designed} as designed and as recorded`);
process.exitCode = complete && same === total && asDesigned === designed ? 0 : 1;
