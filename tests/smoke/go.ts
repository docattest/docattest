// The Go half of the install smoke test: this platform's release archive is unpacked into
// a fresh directory, and its `docattest` checks the Go fixture with nothing told to it: the
// core is the one beside the launcher, and the adapter the one beside it too.
import process from 'node:process';
import { cpSync, existsSync, mkdirSync, readdirSync } from 'node:fs';
import { join, sep } from 'node:path';
import { expect } from './expect.ts';
import { PLATFORM, WINDOWS } from './machine.ts';
import { pathFirst } from './search-path.ts';
import type { StepContext } from './steps.ts';

/** Unpack this platform's archive from `archives` into `into`; the directory it holds. */
export function goInstall(
  into: string,
  { archives, steps, env, from }: StepContext & { archives: string },
): string {
  const archive = readdirSync(archives).find((name) => name.endsWith(`-${PLATFORM}.tar.gz`));
  if (!archive) throw new Error(`no docattest-go archive for ${PLATFORM} in ${archives}`);
  mkdirSync(into, { recursive: true });
  steps.run(`unpack ${archive}`, tar(), ['-xzf', join(archives, archive), '-C', into], {
    cwd: from,
    env,
    from,
  });
  return join(into, archive.replace(/\.tar\.gz$/, ''));
}

/**
 * The tar a user of this platform unpacks with. Windows ships one (bsdtar) in System32; the
 * `tar` found first on a Windows PATH is often Git's GNU tar, which reads `D:\…` as a remote
 * host named `D`.
 */
function tar(): string {
  return WINDOWS ? join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe') : 'tar';
}

/** The Go fixture, checked by the unpacked command. */
export function goProject(
  project: string,
  installed: string,
  { fixture, version, steps, env, from }: StepContext & { fixture: string; version: string },
): void {
  expect(existsSync(join(installed, 'docs', 'facts.md')), "the guide's pages are beside it");
  cpSync(fixture, project, { recursive: true });
  const active = pathFirst(env, installed);
  const docattest = join(installed, WINDOWS ? 'docattest.exe' : 'docattest');
  const run = (title: string, args: string[], code = 0, extra: NodeJS.ProcessEnv = {}) =>
    steps.run(title, docattest, args, { cwd: project, env: { ...active, ...extra }, code, from });
  const told = run('docattest --version', ['--version']);
  expect(told.stdout.trim() === `docattest ${version}`, `it is core ${version}`);
  const traced = run('which core the launcher runs', ['--version'], 0, { DOCATTEST_TRACE: '1' });
  expect(traced.stderr.includes(`${sep}docattest-core`), 'the launcher runs the core beside it');
  run('docattest check: the true pages', ['check', 'docs/pricing.md', 'docs/shipping.md']);
  const all = run('docattest check: every page, one of them false on purpose', ['check'], 1);
  expect(all.stdout.includes('docs/broken.md'), 'the false page fails');
}
