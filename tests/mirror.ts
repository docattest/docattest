#!/usr/bin/env node
// The mirror test: benchmark scenarios from tests/bench/scenarios.ts, and the mirror's own
// (tests/mirror/scenarios.ts), each run through the same Rust core once as written
// (TypeScript adapter) and once for every other language, translated to it by hand
// (tests/mirror/: Python, Go, Rust), and judged by the scenario's own `expect`. Same verdict
// and same exit code as the TypeScript side is the bar. Every project can import this
// checkout's docattest package, as one that installed it can.
//
//   node tests/mirror.ts --core /path/to/docattest [--keep]
//
// The TypeScript side needs the built TypeScript adapter (npm run build). The Go side
// needs a go command and the Rust side a cargo, which build their adapters; a side whose
// tool is missing is left out, and said so.
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { commandLine } from '../scripts/lib/command-line.ts';
import { ROOT } from '../scripts/lib/repository.ts';
import { answerOf, type Answer } from './bench/answer.ts';
import { judge, type Outcome } from './bench/scenario.ts';
import { scenarios } from './bench/scenarios.ts';
import { buildGoAdapter } from './lib/go.ts';
import { gitEnv, writeFiles, type ProjectFiles } from './lib/project.ts';
import { buildRustAdapter } from './lib/rust.ts';
import { go } from './mirror/go.ts';
import { python } from './mirror/python.ts';
import { rust } from './mirror/rust.ts';
import { agreements, type Agreement } from './mirror/scenarios.ts';
import { restated, translate, type Language } from './mirror/translation.ts';

const { values } = commandLine(
  { options: { core: { type: 'string' }, keep: { type: 'boolean', default: false } } },
  'usage: node tests/mirror.ts --core /path/to/docattest [--keep]',
);
const CORE = resolve(values.core ?? join(ROOT, 'core/target/release/docattest'));
const KEEP = values.keep;

const CHOSEN = [
  'audit-01',
  'audit-02',
  'audit-03',
  'audit-n3',
  'audit-r8',
  'guard-01',
  'guard-06',
  'guard-07',
  'guard-08',
  'guard-n5',
  'impact-01',
  'impact-02',
  'impact-03',
  'impact-04',
  'impact-n2',
  'cite-01',
  'cite-02',
  'cite-03',
];

const goAdapter = buildGoAdapter();
const rustAdapter = buildRustAdapter();
/** The languages each scenario is translated to. */
const LANGUAGES: readonly Language[] = [
  python,
  ...(goAdapter ? [go] : []),
  ...(rustAdapter ? [rust] : []),
];

// --- running --------------------------------------------------------------------------

const GIT_ENV = {
  ...gitEnv('bench'),
  DOCATTEST_ADAPTER_TYPESCRIPT: `${process.execPath} ${join(ROOT, 'adapters/typescript/dist/protocol/main.js')}`,
  DOCATTEST_ADAPTER_PYTHON: `python3 ${join(ROOT, 'adapters/python/bin/docattest-adapter-python')}`,
  // Where a setup module finds `docattest.cells`, as in an environment it is installed in.
  PYTHONPATH: join(ROOT, 'adapters/python'),
  ...(goAdapter ? { DOCATTEST_ADAPTER_GO: JSON.stringify(goAdapter.command) } : {}),
  ...(rustAdapter ? { DOCATTEST_ADAPTER_RUST: JSON.stringify(rustAdapter) } : {}),
};

/** How one side of a scenario went: its verdict, exit code, and what the core said. */
interface Side {
  pass: boolean;
  code: number | null;
  what: string;
  root: string;
}

/** A scenario's project and its change: as written, or as translated to a language. */
interface Project {
  files: ProjectFiles<string>;
  head: ProjectFiles<string> | undefined;
  /** The language the project was translated to; none for the scenario as written. */
  language?: Language;
}

function run(scenario: Agreement, { files, head, language }: Project): Side {
  const root = mkdtempSync(join(tmpdir(), 'docattest-mirror-'));
  try {
    writeFiles(root, files);
    if (!language) installNpmPackage(root);
    if (head) {
      const git = (...args: string[]): void => {
        execFileSync('git', args, { cwd: root, env: GIT_ENV, stdio: 'ignore' });
      };
      git('init', '-q', '-b', 'main');
      git('add', '-A');
      git('commit', '-q', '-m', 'base');
      writeFiles(root, head);
    }
    const done = spawnSync(CORE, scenario.run, {
      cwd: root,
      env: GIT_ENV,
      encoding: 'utf8',
      timeout: 120_000,
    });
    // Not JSON: the scenario's own `expect` judges what was printed.
    const stdout = language ? restated(language, done.stdout) : done.stdout;
    const out: Outcome = { code: done.status, stdout, stderr: done.stderr, json: answerOf(stdout) };
    const verdict = judge(scenario, out);
    const thrown =
      verdict.met || verdict.thrown === undefined ? '' : `; it threw: ${verdict.thrown}`;
    return { pass: verdict.met, code: out.code, what: `${summary(out)}${thrown}`, root };
  } finally {
    if (!KEEP) rmSync(root, { recursive: true, force: true });
  }
}

/** The npm package, installed in the project at `root` as a workspace links this checkout's. */
function installNpmPackage(root: string): void {
  mkdirSync(join(root, 'node_modules'));
  symlinkSync(join(ROOT, 'adapters/typescript'), join(root, 'node_modules/docattest'));
}

// What the core said, small enough to compare by eye.
function summary({ json, stderr }: Outcome): string {
  if (json === undefined) return `stderr: ${String(stderr.trim().split('\n')[0]?.slice(0, 110))}`;
  if (Array.isArray(json)) return `listed ${json.join(',') || '-'}`;
  if (json.suggestions) {
    return (
      json.suggestions
        .map((s) => `${s.page}:${s.line} ${s.target} ${s.status} [${s.expression}]`)
        .join('; ') || 'no suggestions'
    );
  }
  if (json.symbols) return impactSummary(json);
  const results = (json.results ?? []).map((r) => `${r.kind}:${r.status}`).join(' ');
  const changes = json.changes
    ? `; changes ${json.changes.map((c) => `${c.type}/${c.severity}@${c.line}`).join(' ') || '-'}`
    : '';
  return `${results}${changes}`;
}

function impactSummary(json: Answer): string {
  const symbols = (json.symbols ?? []).join(',') || '-';
  const references = (json.references ?? [])
    .map((r) => `${r.symbol}@${r.page}:${r.line}:${r.kind}`)
    .join(' ');
  return `symbols ${symbols}; refs ${references || '-'}`;
}

/** The scenario as `language` writes it. */
const mirrored = (scenario: Agreement, language: Language): Project => ({
  files: translate(language, scenario.files),
  head: scenario.head && translate(language, scenario.head),
  language,
});

const verdict = (side: Side): string => `${side.pass ? 'pass' : 'FAIL'} exit ${String(side.code)}`;

const benchmarked = CHOSEN.map((id) => {
  const scenario = scenarios.find((s) => s.id === id);
  if (!scenario) throw new Error(`no scenario ${id}`);
  return scenario;
});
const ALL: readonly Agreement[] = [...benchmarked, ...agreements];

let differing = 0;
for (const scenario of ALL) {
  const ts = run(scenario, { files: scenario.files, head: scenario.head });
  const others = LANGUAGES.map((language) => run(scenario, mirrored(scenario, language)));
  const same = others.every((side) => side.pass === ts.pass && side.code === ts.code);
  if (!same) differing++;
  const verdicts = others.map((side, k) => `${LANGUAGES[k]?.label ?? ''}: ${verdict(side)}`);
  console.log(
    `${same ? 'same' : 'DIFF'}  ${scenario.id.padEnd(10)} ts: ${verdict(ts)} | ${verdicts.join(' | ')}  ${scenario.title}`,
  );
  console.log(`        ts  ${ts.what}`);
  others.forEach((side, k) => {
    console.log(`        ${LANGUAGES[k]?.label ?? ''}  ${side.what}`);
  });
  if (KEEP) console.log(`        kept ${[ts, ...others].map((side) => side.root).join(' ')}`);
}
if (goAdapter && !KEEP) rmSync(goAdapter.dir, { recursive: true, force: true });
if (!goAdapter) {
  console.log('\nno go command: the Go adapter was not built, and its side was not run');
}
if (!rustAdapter) {
  console.log('\nno cargo: the Rust adapter was not built, and its side was not run');
}
console.log(
  `\n${ALL.length - differing} of ${ALL.length} scenarios: same verdict and exit code in ${LANGUAGES.length + 1} languages; ${differing} differ`,
);
process.exit(differing ? 1 : 0);
