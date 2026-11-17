/**
 * Records what the built docattest prints at each stop in stops.ts, for every setting of the
 * stop's switches, each in a fresh copy of the invented project, and writes the recordings
 * the playground replays (../src/playground/recordings.json). With --check, writes nothing and
 * fails when the recordings on disk are not what docattest prints today.
 *
 *   npm run record --workspace website          (after npm run build:core and npm run build)
 *   npm run record:check --workspace website
 */
import { execFileSync, spawnSync } from 'node:child_process';
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import process from 'node:process';
import { fileURLToPath, URL } from 'node:url';
import type {
  CommandOutput,
  Edit,
  RecordedState,
  RecordedStop,
  Recordings,
} from '../src/playground/recordingFormat.ts';
import { PROJECT_FILES } from './project.ts';
import { STOPS, type Command, type Stop } from './stops.ts';

/** Where docattest is built: its npm package and its core. */
interface Installation {
  readonly packageDir: string;
  readonly corePath: string;
}

const OUTPUT = fileURLToPath(new URL('../src/playground/recordings.json', import.meta.url));
/** The directory name shown in place of the temporary one. */
const SHOWN_ROOT = '/work/acme-client';

/** A fixed author and time, so a commit gets the same hash in every recording. */
const GIT_ENV = {
  GIT_CONFIG_GLOBAL: '/dev/null',
  GIT_CONFIG_SYSTEM: '/dev/null',
  GIT_AUTHOR_NAME: 'Sam',
  GIT_AUTHOR_EMAIL: 'sam@example.invalid',
  GIT_AUTHOR_DATE: '2026-01-01T12:00:00Z',
  GIT_COMMITTER_NAME: 'Sam',
  GIT_COMMITTER_EMAIL: 'sam@example.invalid',
  GIT_COMMITTER_DATE: '2026-01-01T12:00:00Z',
};

const corePath = process.env.DOCATTEST_CORE;
if (corePath === undefined || corePath === '') {
  throw new Error('DOCATTEST_CORE is not set: run this through ../scripts/with-core.ts');
}
const installation: Installation = {
  packageDir: fileURLToPath(new URL('../../adapters/typescript', import.meta.url)),
  corePath,
};
const recorded = `${JSON.stringify(recordAll(installation), null, 1)}\n`;
if (process.argv.includes('--check')) {
  if (readFileSync(OUTPUT, 'utf8') !== recorded) {
    process.stderr.write(
      `${OUTPUT} is not what docattest prints today: run \`npm run record --workspace website\`\n`,
    );
    process.exitCode = 1;
  }
} else {
  writeFileSync(OUTPUT, recorded);
}

function recordAll(installation: Installation): Recordings {
  const stops = STOPS.map((stop): [string, RecordedStop] => [
    stop.id,
    { given: stop.given, switches: stop.switches, states: recordStop(stop, installation) },
  ]);
  return { project: PROJECT_FILES, stops: Object.fromEntries(stops) };
}

/** One state for each setting of the switches, keyed by the chosen option of each. */
function recordStop(stop: Stop, installation: Installation): Record<string, RecordedState> {
  const settings = everySetting(stop.switches.map((spot) => spot.options.length));
  return Object.fromEntries(
    settings.map((chosen) => [chosen.join(''), recordState(stop, chosen, installation)]),
  );
}

function everySetting(optionCounts: readonly number[]): number[][] {
  return optionCounts.reduce<number[][]>(
    (settings, count) =>
      settings.flatMap((chosen) =>
        Array.from({ length: count }, (_unused, option) => [...chosen, option]),
      ),
    [[]],
  );
}

function recordState(
  stop: Stop,
  chosen: readonly number[],
  installation: Installation,
): RecordedState {
  const root = createProject(installation);
  try {
    applyEdits(root, [...stop.given, ...switchEdits(stop, chosen)]);
    const outputs = stop.commands.map((command): [string, CommandOutput] => [
      command.id,
      execute(root, command, installation),
    ]);
    const written = stop.capture.map((file): [string, string] => [
      file,
      shown(root, readFileSync(join(root, file), 'utf8')),
    ]);
    return { outputs: Object.fromEntries(outputs), written: Object.fromEntries(written) };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function switchEdits(stop: Stop, chosen: readonly number[]): Edit[] {
  return stop.switches.flatMap((spot, index) => {
    const [original] = spot.options;
    const option = spot.options[chosen[index] ?? 0];
    if (original === undefined || option === undefined || option === original) return [];
    return [{ file: spot.file, find: original.text, replace: option.text }];
  });
}

/** The project as committed, with docattest installed the way npm would link it. */
function createProject(installation: Installation): string {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'acme-client-')));
  for (const [file, text] of Object.entries(PROJECT_FILES)) writeProjectFile(root, file, text);
  writeProjectFile(root, '.gitignore', 'node_modules\n');
  mkdirSync(join(root, 'node_modules/.bin'), { recursive: true });
  symlinkSync(installation.packageDir, join(root, 'node_modules/docattest'));
  symlinkSync(
    join(installation.packageDir, 'dist/launcher/main.js'),
    join(root, 'node_modules/.bin/docattest'),
  );
  for (const args of [
    ['init', '-q', '-b', 'main'],
    ['add', '-A'],
    ['commit', '-q', '-m', 'base'],
  ]) {
    execFileSync('git', args, { cwd: root, env: { ...process.env, ...GIT_ENV } });
  }
  return root;
}

function writeProjectFile(root: string, file: string, text: string): void {
  mkdirSync(dirname(join(root, file)), { recursive: true });
  writeFileSync(join(root, file), text);
}

function applyEdits(root: string, edits: readonly Edit[]): void {
  for (const edit of edits) {
    const before = readFileSync(join(root, edit.file), 'utf8');
    if (before.split(edit.find).length !== 2) {
      throw new Error(`${edit.file} must contain ${JSON.stringify(edit.find)} exactly once`);
    }
    writeFileSync(join(root, edit.file), before.replace(edit.find, edit.replace));
  }
}

function execute(root: string, command: Command, installation: Installation): CommandOutput {
  const bin = join(root, 'node_modules/.bin');
  const program = command.program === 'docattest' ? join(bin, 'docattest') : 'git';
  const ran = spawnSync(program, command.args, {
    cwd: root,
    encoding: 'utf8',
    env: {
      ...process.env,
      ...GIT_ENV,
      DOCATTEST_CORE: installation.corePath,
      PATH: `${bin}:${process.env.PATH ?? ''}`,
    },
    input: command.stdin === undefined ? '' : JSON.stringify(withAbsolutePath(root, command.stdin)),
  });
  if (ran.error !== undefined) throw ran.error;
  const typed = command.program === 'docattest' ? ['npx', 'docattest'] : ['git'];
  const output = {
    command: [...typed, ...command.args].map(quoted).join(' '),
    stdout: shown(root, ran.stdout),
    stderr: shown(root, ran.stderr),
    exitCode: ran.status ?? 1,
  };
  return command.stdin === undefined ? output : { ...output, stdin: JSON.stringify(command.stdin) };
}

/** An agent names the edited file by its full path; the recording knows it by its project
 *  path. */
function withAbsolutePath(root: string, event: Readonly<Record<string, unknown>>): unknown {
  return JSON.parse(JSON.stringify(event), (key, value: unknown) =>
    key === 'file_path' ? join(root, String(value)) : value,
  );
}

function quoted(word: string): string {
  return /^[\w./=-]+$/.test(word) ? word : `"${word}"`;
}

function shown(root: string, text: string): string {
  return text.replaceAll(root, SHOWN_ROOT);
}
