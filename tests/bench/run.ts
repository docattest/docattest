#!/usr/bin/env node
// The drift benchmark. Each scenario builds a small project on disk, optionally commits it
// and applies a change, runs the built command line (the launcher and the core) and judges
// the JSON it prints. See README.md.
//
//   node tests/bench/run.ts [--label <name>] [--extra <scenarios.ts>] [--only <id-prefix>]
//                           [--record-baseline]
import { execFileSync, spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { commandLine } from '../../scripts/lib/command-line.ts';
import { ROOT } from '../../scripts/lib/repository.ts';
import { gitEnv, writeFiles } from '../lib/project.ts';
import { answerOf } from './answer.ts';
import { compareWithBaseline, readBaseline, writeBaseline } from './baseline.ts';
import { scenarios as devScenarios } from './scenarios.ts';
import {
  judge,
  type Category,
  type Outcome,
  type Polarity,
  type Scenario,
  type Verdict,
} from './scenario.ts';

const here = dirname(fileURLToPath(import.meta.url));
// The npm package's `docattest`: the launcher, which runs the core (DOCATTEST_CORE in
// development; `npm run bench` sets it to the core built in this checkout).
const CLI = join(ROOT, 'adapters/typescript/dist/launcher/main.js');
const CONCURRENCY = 6;
const TIMEOUT_MS = 120_000;

const USAGE =
  'usage: node tests/bench/run.ts [--label <name>] [--extra <scenarios.ts>] ' +
  '[--only <id-prefix>] [--record-baseline]';
const { values } = commandLine(
  {
    options: {
      label: { type: 'string', default: 'run' },
      extra: { type: 'string' },
      only: { type: 'string' },
      'record-baseline': { type: 'boolean', default: false },
    },
  },
  USAGE,
);
const { label, only, extra } = values;

/** The scenarios of one set: `dev`, and `heldout` when --extra names a module of them. */
interface ScenarioSet {
  set: string;
  scenarios: readonly Scenario[];
}

const sets: ScenarioSet[] = [{ set: 'dev', scenarios: devScenarios }];
if (extra) sets.push({ set: 'heldout', scenarios: await heldOut(extra) });

/** The scenarios of a module written like scenarios.ts: `export const scenarios: Scenario[]`. */
async function heldOut(file: string): Promise<readonly Scenario[]> {
  const module: unknown = await import(pathToFileURL(resolve(file)).href);
  const scenarios: unknown = (module as { scenarios?: unknown }).scenarios;
  if (!Array.isArray(scenarios) || !scenarios.every(isScenario)) {
    throw new Error(`${file}: expected \`export const scenarios: Scenario[]\``);
  }
  return scenarios;
}

/** Whether `value` has what the bench reads of a scenario. */
function isScenario(value: unknown): value is Scenario {
  const s = value as Partial<Record<keyof Scenario, unknown>> | null;
  return (
    typeof s === 'object' &&
    s !== null &&
    ['category', 'polarity', 'id', 'title'].every(
      (key) => typeof s[key as keyof Scenario] === 'string',
    ) &&
    typeof s.files === 'object' &&
    Array.isArray(s.run) &&
    typeof s.expect === 'function'
  );
}

const GIT_ENV = gitEnv('bench');

function git(root: string, ...args: string[]): void {
  execFileSync('git', args, { cwd: root, env: GIT_ENV, stdio: 'ignore' });
}

function cli(root: string, args: readonly string[]): Promise<Outcome> {
  return new Promise((done) => {
    const child = spawn(process.execPath, [CLI, ...args], { cwd: root, env: GIT_ENV });
    let stdout = '';
    let stderr = '';
    const timer = setTimeout(() => child.kill('SIGKILL'), TIMEOUT_MS);
    child.stdout.on('data', (chunk: Buffer) => (stdout += chunk.toString()));
    child.stderr.on('data', (chunk: Buffer) => (stderr += chunk.toString()));
    child.on('close', (code) => {
      clearTimeout(timer);
      done({ code, stdout, stderr, json: answerOf(stdout) });
    });
  });
}

/** How one scenario went: whether its expectation held, and why not when it did not. */
interface Ran {
  pass: boolean;
  note: string;
  answered: boolean;
}

async function runScenario(scenario: Scenario): Promise<Ran> {
  const root = mkdtempSync(join(tmpdir(), 'docattest-bench-'));
  try {
    writeFiles(root, scenario.files);
    if (scenario.head) {
      git(root, 'init', '-q', '-b', 'main');
      git(root, 'add', '-A');
      git(root, 'commit', '-q', '-m', 'base');
      writeFiles(root, scenario.head);
    }
    const out = await cli(root, scenario.run);
    const verdict = judge(scenario, out);
    return { pass: verdict.met, note: noteOn(verdict, out), answered: out.json !== undefined };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

/** Why a scenario did not pass: what its expectation threw, or what the command printed. */
function noteOn(verdict: Verdict, out: Outcome): string {
  if (verdict.met) return '';
  if (verdict.thrown) return verdict.thrown;
  return out.json === undefined ? firstLine(out.stderr || out.stdout) : 'expectation not met';
}

const firstLine = (text: string): string => text.trim().split('\n')[0]?.slice(0, 120) ?? '';

async function pool<T, R>(items: readonly T[], worker: (item: T) => Promise<R>): Promise<R[]> {
  const results = new Array<R>(items.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: CONCURRENCY }, async () => {
      while (next < items.length) {
        const at = next++;
        results[at] = await worker(items[at] as T);
      }
    }),
  );
  return results;
}

/** A scenario's verdict, with what identifies it. */
interface Row extends Ran {
  set: string;
  id: string;
  category: Category;
  polarity: Polarity;
  title: string;
}

function pick({ id, category, polarity, title }: Scenario): Omit<Row, 'set' | keyof Ran> {
  return { id, category, polarity, title };
}

const rows: Row[] = [];
for (const { set, scenarios } of sets) {
  const chosen = scenarios.filter((s) => !only || s.id.startsWith(only));
  const verdicts = await pool(chosen, async (s): Promise<Row> => {
    const verdict = await runScenario(s);
    return { set, ...pick(s), ...verdict };
  });
  rows.push(...verdicts);
}

/** One category of one set: what was detected, and what stayed silent. */
interface Count {
  set: string;
  category: Category;
  positives: number;
  detected: number;
  negatives: number;
  silent: number;
}

/** A category's count, and its false alarms once it detects anything (see `summarise`). */
type Cell = Count &
  ({ supported: true; falseAlarms: number } | { supported: false; falseAlarms: null });

// A negative scenario ("stay silent here") only means something once the capability
// exists: a missing command is silent about everything. So negatives count only when at
// least one positive of the same category passed in the same set.
function summarise(rows: readonly Row[]): Record<string, Cell> {
  const counts: Record<string, Count> = {};
  for (const row of rows) {
    const count = (counts[`${row.set}/${row.category}`] ??= {
      set: row.set,
      category: row.category,
      positives: 0,
      detected: 0,
      negatives: 0,
      silent: 0,
    });
    if (row.polarity === 'positive') {
      count.positives++;
      if (row.pass) count.detected++;
    } else {
      count.negatives++;
      if (row.pass) count.silent++;
    }
  }
  return Object.fromEntries(
    Object.entries(counts).map(([key, count]): [string, Cell] => [
      key,
      count.detected > 0
        ? { ...count, supported: true, falseAlarms: count.negatives - count.silent }
        : { ...count, supported: false, falseAlarms: null },
    ]),
  );
}

const summary = summarise(rows);
const table = Object.values(summary).map((c) => ({
  set: c.set,
  category: c.category,
  detected: `${c.detected}/${c.positives}`,
  'false alarms': c.supported ? `${String(c.falseAlarms)}/${c.negatives}` : 'n/a',
}));
console.table(table);
for (const row of rows.filter((r) => !r.pass)) {
  const counted = row.polarity === 'positive' || summary[`${row.set}/${row.category}`]?.supported;
  if (counted) console.log(`  ✗ ${row.set} ${row.id} ${row.title}: ${row.note}`);
}

const BASELINE = join(here, 'baseline.json');
const devRows = rows.filter((r) => r.set === 'dev');
if (values['record-baseline']) {
  if (only) throw new Error('--record-baseline records every scenario: drop --only');
  writeBaseline(BASELINE, devRows);
  console.log(`recorded ${BASELINE}`);
} else {
  const { regressed, improved } = compareWithBaseline(readBaseline(BASELINE), devRows);
  for (const id of improved) console.log(`  + ${id} now passes: record it with --record-baseline`);
  for (const id of regressed) console.log(`  ✗ ${id} passes in baseline.json and fails now`);
  if (regressed.length > 0) process.exitCode = 1;
}

if (label !== 'none') {
  const file = join(here, 'results', `${label}.json`);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(
    file,
    `${JSON.stringify({ label, summary: Object.values(summary), rows }, null, 2)}\n`,
  );
  console.log(`wrote ${file}`);
}
