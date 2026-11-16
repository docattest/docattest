#!/usr/bin/env node
// Install the packages a release builds into fresh projects, as a user would, and run them:
//
//   node tests/smoke/run.ts --npm <dir> --wheels <dir> [--go <dir>] [--work <dir>]
//     [--python <exe>] [--typescript <version>]
//
// <dir> of --npm holds the packed docattest package (docattest-<version>.tgz) and the packed
// package that carries this platform's core; --wheels holds the wheels (this platform's, and
// any others, which pip must refuse). Nothing of this checkout is used but the Python
// fixture: no DOCATTEST_* variable is set, and npm, pip, the home directory and every cache
// are new, under --work (a new temporary directory by default). --go holds the Go package's
// release archives (this platform's is used), and needs a go command on the PATH, as a Go
// project does. --typescript is the TypeScript a project installs beside the package (by
// default the one the package is developed with; the oldest it supports in the job that
// holds the documented minimums). The steps:
//
// - npm: `docattest --version`, which core the launcher runs, `check` passing and failing,
//   `hooks install`, `createProject` from a script, the API's types;
// - pip: the same with no Node on the PATH, on adapters/python/tests/fixture;
// - go (with --go): the archive unpacked, and its command on adapters/go/testdata/fixture;
// - both installed in one project: either command checks a Python and a TypeScript project;
// - without the package that carries the core, npm's launcher says what is missing; pip
//   refuses a wheel for another platform.
//
// The release workflow runs this on every platform it builds for (.github/workflows).
import process from 'node:process';
import { cpSync, mkdirSync, mkdtempSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { commandLine } from '../../scripts/lib/command-line.ts';
import { version } from '../../scripts/lib/core-pin.ts';
import { fail } from '../../scripts/lib/fail.ts';
import { npmName } from '../../scripts/lib/platforms.ts';
import { readJson, ROOT } from '../../scripts/lib/repository.ts';
import { writeFiles } from '../lib/project.ts';
import { cleanEnvironment } from './environment.ts';
import { goInstall, goProject } from './go.ts';
import { PLATFORM, WINDOWS } from './machine.ts';
import { pathFirst } from './search-path.ts';
import { Steps } from './steps.ts';
import { TS_FILES, install, npmProject, npmWithoutCore, type Packages } from './npm.ts';
import { pipInstall, pipProject, pipWrongPlatform, venvBin } from './pip.ts';

const USAGE =
  'usage: node tests/smoke/run.ts --npm <dir> --wheels <dir> [--go <dir>] [--work <dir>] ' +
  '[--python <exe>] [--typescript <version>]';
const { values: args } = commandLine(
  {
    options: {
      npm: { type: 'string' },
      wheels: { type: 'string' },
      go: { type: 'string' },
      work: { type: 'string' },
      python: { type: 'string' },
      typescript: { type: 'string' },
    },
  },
  USAGE,
);
if (args.npm === undefined || args.wheels === undefined) fail(USAGE);
const npmDir = resolve(args.npm);
const given = resolve(args.work ?? mkdtempSync(join(tmpdir(), 'docattest-smoke-')));
mkdirSync(given, { recursive: true });
// The launcher reports real paths, and a temporary directory is often behind a symlink.
const work = realpathSync(given);
const pinned = version();
const coreName = npmName(PLATFORM);
const tarball = (name: string): string =>
  join(npmDir, `${name.replace(/^@/, '').replace('/', '-')}-${pinned}.tgz`);
const packages: Packages = {
  version: pinned,
  coreName,
  main: tarball('docattest'),
  core: tarball(coreName),
  typescript: args.typescript ?? developedWith(),
};
const env = cleanEnvironment(work);
const from = work;
const steps = new Steps();
const shared = { packages, steps, env, from };
const python = args.python ?? (WINDOWS ? 'python' : 'python3');
const fixture = join(ROOT, 'adapters', 'python', 'tests', 'fixture');
const wheels = resolve(args.wheels);

/** A project a launcher is told to check: where it is, and which pages (all, when none). */
interface Checked {
  cwd: string;
  pages: string[];
}

/** One project with both packages: either launcher checks either language's project. */
function both(project: string): void {
  cpSync(fixture, project, { recursive: true });
  install(project, [packages.main, packages.core], shared);
  const venv = join(project, '.venv');
  pipInstall(venv, { python, wheels, steps, env, from });
  const active = pathFirst(env, venvBin(venv));
  const pythonProject: Checked = { cwd: project, pages: ['docs/pricing.md', 'docs/shipping.md'] };
  const web: Checked = { cwd: join(project, 'web'), pages: [] };
  writeFiles(web.cwd, {
    ...TS_FILES,
    'package.json': '{ "name": "smoke-project", "type": "module" }\n',
  });
  const npx = (title: string, { cwd, pages }: Checked) =>
    steps.run(title, 'npx', ['--no', '--', 'docattest', 'check', ...pages], {
      cwd,
      env: active,
      from,
    });
  const pip = (title: string, { cwd, pages }: Checked, path = active) =>
    steps.run(title, join(venvBin(venv), 'docattest'), ['check', ...pages], {
      cwd,
      env: path,
      from,
    });
  npx('npm’s docattest, the Python project (its adapter from the virtualenv)', pythonProject);
  pip('pip’s docattest, the Python project', pythonProject);
  npx('npm’s docattest, a TypeScript project inside it', web);
  const bin = pathFirst(active, join(project, 'node_modules', '.bin'));
  pip('pip’s docattest, the TypeScript project (its adapter from node_modules/.bin)', web, bin);
}

try {
  process.stdout.write(`smoke test of docattest ${pinned} on ${PLATFORM}, in ${work}\n`);
  npmProject(join(work, 'npm-project'), shared);
  const venv = join(work, 'venv');
  const picked = pipInstall(venv, { python, wheels, steps, env, from });
  pipProject(join(work, 'pip-project'), venv, { fixture, version: pinned, steps, env, from });
  if (args.go !== undefined) {
    const installed = goInstall(join(work, 'go'), { archives: resolve(args.go), steps, env, from });
    const goFixture = join(ROOT, 'adapters', 'go', 'testdata', 'fixture');
    goProject(join(work, 'go-project'), installed, {
      fixture: goFixture,
      version: pinned,
      steps,
      env,
      from,
    });
  }
  both(join(work, 'both'));
  npmWithoutCore(join(work, 'npm-without-core'), shared);
  pipWrongPlatform(venv, { wheels, picked, steps, env, from });
  writeFileSync(join(work, 'PASSED'), `${steps.count} steps\n`);
  process.stdout.write(`\nsmoke: all ${steps.count} steps passed on ${PLATFORM}\n`);
} catch (error) {
  fail(`\nsmoke: FAILED on ${PLATFORM}: ${error instanceof Error ? error.message : String(error)}`);
}

/** The TypeScript the npm package is developed and tested with: its dev dependency. */
function developedWith(): string {
  const manifest = readJson('adapters/typescript/package.json') as {
    devDependencies: { typescript: string };
  };
  return manifest.devDependencies.typescript;
}
