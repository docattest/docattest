#!/usr/bin/env node
// Publish the Go package, so that one `go install` brings a working docattest:
//
//   node scripts/release/go-modules.ts --cores <directory>
//
// <directory> holds the release's cores as the release workflow downloads them
// (<directory>/core-<platform>/docattest, or docattest.exe). It runs in a checkout of the
// release's tag that can push to origin, and each step is skipped when an earlier run of
// the same release already did it:
//
// 1. Each platform's core goes into a Go module of its own (tools/coremodule), committed
//    outside main and tagged go-core/core-<platform>/v<version>.
// 2. The Go package requires those modules (go.mod, go.sum and internal/bundled), committed
//    on top of the release's commit and tagged adapters/go/v<version>: the version that
//    `go install …@latest` takes.
// 3. docattest is installed with `go install`, as a user installs it, and run.
// 4. The core modules' tags of earlier releases are deleted, so that a clone of the
//    repository carries one release's cores. Go's module proxy keeps every version it has
//    served, and step 3 has had it serve this one.
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import { commandLine } from '../lib/command-line.ts';
import { version as releaseVersion } from '../lib/core-pin.ts';
import { fail } from '../lib/fail.ts';
import { platforms } from '../lib/platforms.ts';
import { ROOT } from '../lib/repository.ts';
import { run } from './run.ts';

/** The repository's module path, which every Go module of the release is below. */
const REPOSITORY = 'github.com/docattest/docattest';
/** The Go package's directory in the repository: also the prefix of its version tags. */
const GO_PACKAGE = 'adapters/go';
/** The directory, and module path below the repository, of the core modules. */
const CORES = 'go-core';
/** Who the release's commits are by. */
const BOT = {
  name: 'github-actions[bot]',
  email: '41898282+github-actions[bot]@users.noreply.github.com',
};
/** How long a module the proxy has not seen yet may take to be served. */
const PROXY_ATTEMPTS = 20;
const PROXY_WAIT_MS = 15_000;

const USAGE = 'usage: node scripts/release/go-modules.ts --cores <directory>';
const { values: args } = commandLine({ options: { cores: { type: 'string' } } }, USAGE);
if (args.cores === undefined) fail(USAGE);

const version = `v${releaseVersion()}`;
const names = Object.keys(platforms().platforms);
const coreTag = (name: string): string => `${CORES}/core-${name}/${version}`;
const packageTag = `${GO_PACKAGE}/${version}`;
const packageDir = join(ROOT, GO_PACKAGE);

const work = mkdtempSync(join(tmpdir(), 'docattest-go-modules-'));
writeModules(args.cores, join(work, CORES));
const published = remoteTags('refs/tags/*');
if (names.every((name) => published.includes(coreTag(name)))) {
  console.log(`the core modules of ${version} are tagged already`);
} else {
  pushCoreModules(work);
}
if (published.includes(packageTag)) {
  console.log(`${packageTag} is tagged already`);
} else {
  await requireCoreModules();
  pushPackage();
}
await installAndRun(join(work, 'install'));
pruneEarlierCores();

/** Each platform's core module into `out`, and the files that import them into the package. */
function writeModules(cores: string, out: string): void {
  const pairs = names.map((name) => {
    const binary = platforms().platforms[name]?.binary ?? fail(`no platform ${name}`);
    return `${name}=${join(cores, `core-${name}`, binary)}`;
  });
  const link = join(packageDir, 'internal', 'bundled');
  run(
    'go',
    [
      'run',
      './tools/coremodule',
      '-base',
      `${REPOSITORY}/${CORES}`,
      '-out',
      out,
      '-link',
      link,
      ...pairs,
    ],
    {
      cwd: packageDir,
    },
  );
}

/** The core modules as one commit outside any branch, tagged once for each platform. */
function pushCoreModules(work: string): void {
  // The repository's objects, with `work` as the files and an index of its own, so that the
  // checkout's own index and files are left as they are.
  const env = {
    ...process.env,
    ...identity(),
    GIT_DIR: git(['rev-parse', '--absolute-git-dir']),
    GIT_WORK_TREE: work,
    GIT_INDEX_FILE: join(work, 'index'),
  };
  git(['add', CORES], env, work);
  const tree = git(['write-tree'], env, work);
  const message = `docattest ${version}: the core of each platform as a Go module`;
  const commit = git(['commit-tree', tree, '-m', message], env, work);
  run('git', ['push', 'origin', ...names.map((name) => `${commit}:refs/tags/${coreTag(name)}`)], {
    cwd: ROOT,
  });
}

/** go.mod and go.sum require this release's core modules, read through Go's module proxy. */
async function requireCoreModules(): Promise<void> {
  const requires = names.map((name) => `-require=${REPOSITORY}/${CORES}/core-${name}@${version}`);
  run('go', ['mod', 'edit', ...requires], { cwd: packageDir });
  await untilServed('go mod tidy', () => tries('go', ['mod', 'tidy'], packageDir));
}

/** The Go package that requires its cores, committed on the release's commit and tagged. */
function pushPackage(): void {
  git(['add', 'go.mod', 'go.sum', 'internal/bundled'], process.env, packageDir);
  const message = `Go package ${version}: requires the core modules of its release`;
  git(['commit', '-m', message], { ...process.env, ...identity() });
  run('git', ['push', 'origin', `HEAD:refs/tags/${packageTag}`], { cwd: ROOT });
}

/** `go install` the package's commands into an empty directory; run docattest, cache empty. */
async function installAndRun(dir: string): Promise<void> {
  const bin = join(dir, 'bin');
  mkdirSync(bin, { recursive: true });
  const env = {
    ...process.env,
    GOBIN: bin,
    DOCATTEST_CORE: '',
    DOCATTEST_CACHE: join(dir, 'cache'),
  };
  const target = `${REPOSITORY}/${GO_PACKAGE}/cmd/...@${version}`;
  await untilServed(`go install ${target}`, () => tries('go', ['install', target], dir, env));
  const ran = spawnSync(join(bin, 'docattest'), ['--version'], { cwd: dir, env, encoding: 'utf8' });
  const said = ran.stdout.trim();
  if (said !== `docattest ${releaseVersion()}`) {
    fail(`the installed docattest said "${said}" ${ran.stderr}`);
  }
  console.log(`go install ${target} brings ${said}`);
}

/** Delete the tags of every earlier release's core modules. */
function pruneEarlierCores(): void {
  const earlier = remoteTags(`refs/tags/${CORES}/*`).filter((tag) => !tag.endsWith(`/${version}`));
  if (earlier.length === 0) return;
  run('git', ['push', 'origin', '--delete', ...earlier.map((tag) => `refs/tags/${tag}`)], {
    cwd: ROOT,
  });
}

/** Retry `attempt` while Go's module proxy has yet to serve what was just pushed. */
async function untilServed(what: string, attempt: () => boolean): Promise<void> {
  for (let n = 1; n <= PROXY_ATTEMPTS; n++) {
    if (attempt()) return;
    console.log(`${what}: not served yet (attempt ${n} of ${PROXY_ATTEMPTS})`);
    await sleep(PROXY_WAIT_MS);
  }
  fail(`${what} still fails after ${PROXY_ATTEMPTS} attempts`);
}

/** Run a program with its output shown; whether it succeeded. */
function tries(program: string, argv: readonly string[], cwd: string, env = process.env): boolean {
  return spawnSync(program, argv, { cwd, env, stdio: 'inherit' }).status === 0;
}

/** The names of origin's tags that match `pattern`. */
function remoteTags(pattern: string): string[] {
  const listed = git(['ls-remote', '--tags', '--refs', 'origin', pattern]);
  return listed
    .split('\n')
    .filter(Boolean)
    .map((line) => line.split('\t')[1]?.replace('refs/tags/', '') ?? '');
}

/** Run git in the repository (or `cwd`) and return what it printed, trimmed. */
function git(argv: readonly string[], env = process.env, cwd = ROOT): string {
  const done = spawnSync('git', argv, { cwd, env, encoding: 'utf8' });
  if (done.status !== 0) fail(`git ${argv.join(' ')}: ${done.stderr}`);
  return done.stdout.trim();
}

/** The release's commits are by the workflow's bot. */
function identity(): Record<string, string> {
  return {
    GIT_AUTHOR_NAME: BOT.name,
    GIT_AUTHOR_EMAIL: BOT.email,
    GIT_COMMITTER_NAME: BOT.name,
    GIT_COMMITTER_EMAIL: BOT.email,
  };
}
