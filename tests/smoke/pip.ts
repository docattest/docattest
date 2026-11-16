// The Python half of the install smoke test: a fresh virtualenv installs docattest from the
// built wheels alone (no index, so pip has to pick this platform's wheel by its tags), and
// runs it with no Node on the PATH.
import process from 'node:process';
import { cpSync, readdirSync } from 'node:fs';
import { join, sep } from 'node:path';
import { expect } from './expect.ts';
import { WINDOWS } from './machine.ts';
import { withoutNode } from './search-path.ts';
import type { StepContext } from './steps.ts';

/** The executables of a virtualenv. */
export const venvBin = (venv: string): string => join(venv, WINDOWS ? 'Scripts' : 'bin');
const exe = (venv: string, name: string): string =>
  join(venvBin(venv), WINDOWS ? `${name}.exe` : name);

/** A virtualenv at `venv`, with docattest installed from `wheels`. */
export function pipInstall(
  venv: string,
  { python, wheels, steps, env, from }: StepContext & { python: string; wheels: string },
): string {
  steps.run('create a virtualenv', python, ['-m', 'venv', venv], { cwd: from, env, from });
  const args = ['-m', 'pip', 'install', '--no-index', '--find-links', wheels, 'docattest'];
  const done = steps.run('pip install docattest from the wheels', exe(venv, 'python'), args, {
    cwd: from,
    env,
    from,
  });
  const picked = /docattest-[^\s/\\]+\.whl/.exec(done.stdout)?.[0] ?? '(not shown)';
  process.stdout.write(`pip picked ${picked}\n`);
  return picked;
}

/** The Python fixture, checked by the installed command with no Node anywhere on the PATH. */
export function pipProject(
  project: string,
  venv: string,
  { fixture, version, steps, env, from }: StepContext & { fixture: string; version: string },
): void {
  cpSync(fixture, project, { recursive: true });
  const bare = withoutNode(env, venvBin(venv));
  const node = steps.run('is there a node?', exe(venv, 'python'), ['-c', NODE_PROBE], {
    cwd: project,
    env: bare,
    from,
  });
  expect(node.stdout.trim() === 'None', 'no node on the PATH');
  const run = (title: string, args: string[], code = 0, extra: NodeJS.ProcessEnv = {}) =>
    steps.run(title, exe(venv, 'docattest'), args, {
      cwd: project,
      env: { ...bare, ...extra },
      code,
      from,
    });
  const told = run('docattest --version', ['--version']);
  expect(told.stdout.trim() === `docattest ${version}`, `it is core ${version}`);
  const traced = run('which core the launcher runs', ['--version'], 0, { DOCATTEST_TRACE: '1' });
  const bundled = `${sep}site-packages${sep}docattest${sep}bin${sep}docattest`;
  expect(traced.stderr.includes(bundled), 'the launcher runs the core from the wheel');
  run('docattest check: the true pages', ['check', 'docs/pricing.md', 'docs/shipping.md']);
  const all = run('docattest check: every page, one of them false on purpose', ['check'], 1);
  expect(all.stdout.includes('docs/broken.md'), 'the false page fails');
}

const NODE_PROBE = 'import shutil; print(shutil.which("node"))';

/** A wheel for another platform than this one is refused by pip. */
export function pipWrongPlatform(
  venv: string,
  { wheels, picked, steps, env, from }: StepContext & { wheels: string; picked: string },
): void {
  const other = readdirSync(wheels).find((name) => name.endsWith('.whl') && name !== picked);
  if (!other) {
    process.stdout.write('\n(no wheel for another platform to try)\n');
    return;
  }
  const args = ['-m', 'pip', 'install', '--no-index', '--force-reinstall', join(wheels, other)];
  const done = steps.run(`pip install ${other}`, exe(venv, 'python'), args, {
    cwd: from,
    env,
    code: 1,
    from,
  });
  expect(done.stderr.includes('is not a supported wheel on this platform'), 'pip refuses it');
}
