#!/usr/bin/env node
// What the core really sends, and what the adapters really answer, held to the protocol's
// schemas (protocol/schema). The core is run on real projects with a recorder between it
// and the adapter (`docattest conformance record`); every recorded line must fit
// (`docattest conformance validate`).
//
//   node tests/schema.ts --candidate /path/to/docattest
//
// The TypeScript adapter is recorded on this repository, the Python, Go and Rust adapters
// each on their fixture.
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { commandLine } from '../scripts/lib/command-line.ts';
import { ROOT } from '../scripts/lib/repository.ts';
import { buildGoAdapter } from './lib/go.ts';
import { buildRustAdapter } from './lib/rust.ts';

/** An adapter the core is run with, by the name its variable carries. */
type Adapter = 'typescript' | 'python' | 'go' | 'rust';

/** One adapter's recording: the project it is run on, and the commands. */
type Recorded = [adapter: Adapter, root: string, commands: string[][]];

const { values } = commandLine(
  { options: { candidate: { type: 'string' } } },
  'usage: node tests/schema.ts --candidate /path/to/docattest',
);
const core = values.candidate;
if (!core) throw new Error('pass --candidate <core binary>');
const temp = mkdtempSync(join(tmpdir(), 'docattest-schema-'));
const python = spawnSync('python3', ['--version']).status === 0;
const goAdapter = buildGoAdapter();
const rustAdapter = buildRustAdapter();

const adapters: Readonly<Record<Adapter, string[]>> = {
  typescript: [process.execPath, join(ROOT, 'adapters/typescript/dist/protocol/main.js')],
  python: ['python3', join(ROOT, 'adapters/python/bin/docattest-adapter-python')],
  go: goAdapter?.command ?? [],
  rust: rustAdapter ?? [],
};
/** What each fixture of a language package is asked. */
const FIXTURE_COMMANDS = [['check', '--json'], ['index'], ['audit'], ['inventory'], ['uncovered']];
const runs: Recorded[] = [
  [
    'typescript',
    ROOT,
    [
      ['check', '--json'],
      ['index'],
      ['audit'],
      ['uncovered'],
      ['impact'],
      ['check', '--base', 'HEAD', '--quiet'],
    ],
  ],
  ...(python
    ? [['python', join(ROOT, 'adapters/python/tests/fixture'), FIXTURE_COMMANDS] satisfies Recorded]
    : []),
  ...(goAdapter
    ? [['go', join(ROOT, 'adapters/go/testdata/fixture'), FIXTURE_COMMANDS] satisfies Recorded]
    : []),
  ...(rustAdapter
    ? [['rust', join(ROOT, 'adapters/rust/tests/fixture'), FIXTURE_COMMANDS] satisfies Recorded]
    : []),
];

/** Run the core on `root` with a recorder logging to `log` around `name`'s adapter. */
function record([name, root, commands]: Recorded, log: string, candidate: string): void {
  const variable = `DOCATTEST_ADAPTER_${name.toUpperCase()}`;
  const env = {
    ...process.env,
    [variable]: JSON.stringify([candidate, 'conformance', 'record', log, '--', ...adapters[name]]),
  };
  for (const args of commands) {
    const done = spawnSync(candidate, [...args, '--root', root], { env, encoding: 'utf8' });
    if (done.status === 2 || done.status === null) {
      console.log(
        `✗ ${name}: ${args.join(' ')} exited ${String(done.status)}\n${done.stderr.slice(0, 600)}`,
      );
      process.exitCode = 1;
    }
  }
}

const logs: string[] = [];
try {
  for (const recorded of runs) {
    const log = join(temp, `${recorded[0]}.jsonl`);
    logs.push(log);
    record(recorded, log, core);
  }
  const checked = spawnSync(core, ['conformance', 'validate', ...logs], { stdio: 'inherit' });
  if (checked.status !== 0) process.exitCode = 1;
  if (!python) console.log('python3 not found: the Python adapter was not recorded');
  if (!goAdapter) console.log('go not found: the Go adapter was not recorded');
  if (!rustAdapter) console.log('cargo not found: the Rust adapter was not recorded');
} finally {
  rmSync(temp, { recursive: true, force: true });
  if (goAdapter) rmSync(goAdapter.dir, { recursive: true, force: true });
}
