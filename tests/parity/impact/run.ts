// Running one impact case (tests/parity/impact.ts): a fresh project and home directory, the
// case's steps run there one by one, and what each printed and changed, normalised so that
// it reads the same on any machine.
import { Buffer } from 'node:buffer';
import { execFileSync, spawn } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { writeFiles } from '../../lib/project.ts';
import { changedFiles, type ObservedFiles, type Step } from '../recording-format.ts';
import { DEAD, URL_BASE } from './server.ts';
import { stepSpec, type ImpactCase, type StepSpec } from './types.ts';

/** What one run of a command printed, and how it ended. */
interface Ran {
  code: number | string | null;
  stdout: string;
  stderr: string;
}

/** Where, with what, and with what on stdin, a command runs. */
interface RunOptions {
  cwd: string;
  env: NodeJS.ProcessEnv;
  stdin: string | undefined;
}

function run(command: readonly string[], args: readonly string[], options: RunOptions) {
  const [program = '', ...rest] = command;
  return new Promise<Ran>((done) => {
    const child = spawn(program, [...rest, ...args], { cwd: options.cwd, env: options.env });
    const out: Buffer[] = [];
    const err: Buffer[] = [];
    child.stdout.on('data', (d: Buffer) => out.push(d));
    child.stderr.on('data', (d: Buffer) => err.push(d));
    child.on('error', (e) => {
      done({ code: `spawn: ${e.message}`, stdout: '', stderr: '' });
    });
    child.on('close', (code) => {
      done({
        code,
        stdout: Buffer.concat(out).toString('utf8'),
        stderr: Buffer.concat(err).toString('utf8'),
      });
    });
    child.stdin.on('error', () => undefined);
    child.stdin.end(options.stdin ?? '');
  });
}

/** Every file and directory below `dir`, with contents, `.git` aside. */
function tree(dir: string, rel = ''): Record<string, string> {
  const out: Record<string, string> = {};
  for (const entry of readdirSync(join(dir, rel), { withFileTypes: true }).sort((a, b) =>
    a.name < b.name ? -1 : 1,
  )) {
    if (entry.name === '.git') continue;
    const child = rel ? `${rel}/${entry.name}` : entry.name;
    if (entry.isDirectory()) Object.assign(out, { [`${child}/`]: 'directory' }, tree(dir, child));
    else out[child] = readFileSync(join(dir, child)).toString('latin1');
  }
  return out;
}

/** The hook `hooks install --git` writes where git looks by default, with its mode. */
function gitHook(root: string): Record<string, string> {
  const hook = join(root, '.git/hooks/pre-commit');
  if (!existsSync(hook)) return {};
  const mode = (statSync(hook).mode & 0o777).toString(8);
  return { '.git/hooks/pre-commit': `${mode} ${readFileSync(hook).toString('latin1')}` };
}

// The server's ports differ from run to run; recordings name them by what they are.
const normPorts = (text: string): string =>
  text.split(URL_BASE).join('http://<server>').split(DEAD).join('http://<dead>');

/** A case's project and home directory, and how text that names them is normalised. */
class Sandbox {
  readonly top = realpathSync(mkdtempSync(join(tmpdir(), 'docattest-impact-parity-')));
  readonly root = join(this.top, 'project');
  readonly home = join(this.top, 'home');

  constructor() {
    mkdirSync(this.root);
    mkdirSync(this.home);
  }

  /** `__ROOT__` and `__HOME__` in a case's text, made real. */
  subst(text: string): string {
    return text.split('__ROOT__').join(this.root).split('__HOME__').join(this.home);
  }

  /** The project and home directory in what a run printed, named instead. */
  norm(text: string): string {
    return normPorts(text.split(this.root).join('<root>').split(this.home).join('<home>'));
  }

  /** Every file of the project and (as `~/…`) of the home directory, normalised. */
  files(): ObservedFiles {
    const home = Object.entries(tree(this.home)).map(([k, v]): [string, string] => [`~/${k}`, v]);
    const all = { ...tree(this.root), ...gitHook(this.root), ...Object.fromEntries(home) };
    return Object.fromEntries(Object.entries(all).map(([k, v]) => [k, this.norm(v)]));
  }
}

/** Run one step in the sandbox, as the recordings hold it, files aside. */
async function runStep(
  command: readonly string[],
  spec: StepSpec,
  box: Sandbox,
  env: NodeJS.ProcessEnv,
): Promise<Omit<Step, 'files'>> {
  const { args, stdin, env: extra, cwd } = spec;
  const result = await run(
    command,
    args.map((arg) => box.subst(arg)),
    {
      cwd: cwd ? join(box.root, cwd) : box.root,
      env: { ...env, ...extra },
      stdin: stdin === undefined ? undefined : box.subst(stdin),
    },
  );
  // An internal error's stack names the implementation's own files: keep its first two lines.
  const stderr =
    result.code === 2
      ? box.norm(result.stderr).split('\n').slice(0, 2).join('\n')
      : box.norm(result.stderr);
  return { label: args.join(' '), code: result.code, stdout: box.norm(result.stdout), stderr };
}

/** Every step of a case, run by `command` in a fresh project and home directory. */
export async function side(
  command: readonly string[],
  testCase: ImpactCase,
  base: NodeJS.ProcessEnv,
): Promise<Step[]> {
  const box = new Sandbox();
  try {
    writeFiles(box.root, testCase.files);
    writeFiles(box.home, testCase.home ?? {});
    const env = { ...base, HOME: box.home };
    const git = (...args: string[]): void => {
      execFileSync('git', args, { cwd: box.root, env, stdio: 'ignore' });
    };
    testCase.prepare?.(box.root, git, (more) => {
      writeFiles(box.root, more);
    });
    const steps: Step[] = [];
    let before = box.files();
    for (const step of testCase.steps) {
      const ran = await runStep(command, stepSpec(step), box, env);
      // What the step changed under the project and the home directory, and nothing else:
      // the files before the first step are the case's own.
      const after = box.files();
      steps.push({ ...ran, files: changedFiles(before, after) });
      before = after;
    }
    return steps;
  } finally {
    rmSync(box.top, { recursive: true, force: true });
  }
}
