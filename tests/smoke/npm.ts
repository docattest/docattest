// The npm half of the install smoke test: a fresh project installs the packed docattest
// package and the packed package that carries this platform's core, as a user would.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, sep } from 'node:path';
import { writeFiles } from '../lib/project.ts';
import { expect } from './expect.ts';
import type { StepContext } from './steps.ts';

/** The packed packages under test: the version, and the tarballs of both packages. */
export interface Packages {
  version: string;
  /** The npm package that carries this platform's core. */
  coreName: string;
  /** docattest's tarball. */
  main: string;
  /** The core package's tarball. */
  core: string;
  /** The TypeScript installed beside them, the peer dependency: a version or range for npm. */
  typescript: string;
}

/** What every project of the smoke test is given: the packages, and how its steps run. */
export interface Shared extends StepContext {
  packages: Packages;
}

/** A small TypeScript project: one page with a fact and an example, both true. */
export const TS_FILES = {
  'package.json': '{ "name": "smoke-project", "private": true, "type": "module" }\n',
  'docattest.config.json':
    '{ "pages": ["docs/**/*.md"], "surfaces": { "default": "./src/index.ts" }, ' +
    '"adapter": "typescript" }\n',
  'src/index.ts': 'export const RETRY_DEFAULTS = { maxAttempts: 3, backoffMs: 250 };\n',
  'docs/retries.md': [
    '# Retries',
    '',
    'A failing call is tried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->',
    '',
    '```ts check',
    "import { RETRY_DEFAULTS } from 'smoke-project';",
    'assert.ok(RETRY_DEFAULTS.backoffMs > 0);',
    '```',
    '',
  ].join('\n'),
  // The programmatic API, run from a script, and type-checked as a consumer would.
  'api.mjs': [
    "import { createProject } from 'docattest';",
    'const report = await createProject({ root: process.cwd() }).check();',
    'const lines = report.results.map((r) => `${r.page}:${r.line} ${r.kind} ${r.status}`);',
    'console.log(JSON.stringify({ exitCode: report.exitCode, results: lines }));',
    '',
  ].join('\n'),
  'consumer.ts': [
    "import { createProject, type Report } from 'docattest';",
    'export const run = async (): Promise<number> => {',
    "  const report: Report = await createProject({ root: '.' }).check();",
    '  return report.exitCode;',
    '};',
    '',
  ].join('\n'),
};

/** Install the tarballs (and TypeScript, the peer dependency) into a fresh project. */
export function install(
  project: string,
  tarballs: readonly string[],
  { packages, steps, env, from }: Shared,
  options: readonly string[] = [],
): void {
  const typescript = `typescript@${packages.typescript}`;
  const args = ['install', ...tarballs, typescript, '--no-audit', '--no-fund', ...options];
  steps.run('npm install into an empty project', 'npm', args, { cwd: project, env, from });
}

/** The npm package's commands and API in a project that installed it with its core. */
export function npmProject(project: string, shared: Shared): void {
  const { packages, steps, env, from } = shared;
  writeFiles(project, TS_FILES);
  install(project, [packages.main, packages.core], shared);
  const run = (title: string, args: string[], code = 0, extra: NodeJS.ProcessEnv = {}) =>
    steps.run(title, 'npx', ['--no', '--', 'docattest', ...args], {
      cwd: project,
      env: { ...env, ...extra },
      code,
      from,
    });
  const version = run('npx docattest --version', ['--version']);
  expect(
    version.stdout.trim() === `docattest ${packages.version}`,
    `it is core ${packages.version}`,
  );
  const traced = run('which core the launcher runs', ['--version'], 0, { DOCATTEST_TRACE: '1' });
  const bundled = join(project, 'node_modules', ...packages.coreName.split('/'), 'bin');
  expect(
    traced.stderr.includes(`[trace launcher] core ${bundled}${sep}docattest`),
    `the launcher runs the core from ${packages.coreName}`,
  );
  run('npx docattest check: a true page', ['check']);
  const page = join(project, 'docs', 'retries.md');
  writeFileSync(page, readFileSync(page, 'utf8').replace('most `3`', 'most `4`'));
  const failed = run('npx docattest check: a page that is no longer true', ['check'], 1);
  expect(failed.stdout.includes('expected 4, got 3'), 'the failure names both sides');
  writeFileSync(page, TS_FILES['docs/retries.md']);
  run('npx docattest hooks install (in the temporary project)', ['hooks', 'install']);
  const settings = readFileSync(join(project, '.claude', 'settings.json'), 'utf8');
  expect(settings.includes('npx --no docattest'), 'the hook runs the project’s own docattest');
  const api = steps.run('createProject from a script', 'node', ['api.mjs'], {
    cwd: project,
    env,
    from,
  });
  // What api.mjs printed: `{ exitCode, results }`.
  const printed = JSON.parse(api.stdout) as { exitCode: number };
  expect(printed.exitCode === 0, 'the API ran the same core and passed');
  const tsc = ['--no', '--', 'tsc', '--noEmit', '--strict', '--module', 'nodenext', 'consumer.ts'];
  steps.run('the API type-checks from the packed types', 'npx', tsc, { cwd: project, env, from });
}

/** Without the package that carries the core, the launcher says what is missing. */
export function npmWithoutCore(project: string, shared: Shared): void {
  const { packages, steps, env, from } = shared;
  writeFiles(project, TS_FILES);
  // Optional packages left out, as npm leaves them when told to: the platform packages are
  // on the registry, so without it npm would fetch the core from there.
  install(project, [packages.main], shared, ['--omit=optional']);
  expect(!existsSync(join(project, 'node_modules', ...packages.coreName.split('/'))), 'no core');
  const done = steps.run(
    'npx docattest check with no core',
    'npx',
    ['--no', '--', 'docattest', 'check'],
    {
      cwd: project,
      env,
      code: 1,
      from,
    },
  );
  const wanted = `${packages.coreName}@${packages.version}`;
  expect(
    done.stderr.includes(`The core for `) && done.stderr.includes(wanted),
    `it names ${wanted}`,
  );
  expect(done.stderr.includes('npm install --include=optional'), 'it says how to fix it');
}
