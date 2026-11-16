#!/usr/bin/env node
// Runs a command with DOCATTEST_CORE set to the core built in this checkout
// (core/target/release/docattest), for development: the npm scripts run docattest through
// the npm package's launcher (adapters/typescript/dist/launcher/main.js) and its API, which
// find the core as a user's install does (PROTOCOL.md §1), and DOCATTEST_CORE is the first
// place they look. A DOCATTEST_CORE that is already set is left alone.
//
//   node scripts/with-core.ts <command> [args...]
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import process from 'node:process';
import { fileURLToPath, URL } from 'node:url';

const name = process.platform === 'win32' ? 'docattest.exe' : 'docattest';
const built = fileURLToPath(new URL(`../core/target/release/${name}`, import.meta.url));
// An empty DOCATTEST_CORE is not set.
const core = process.env.DOCATTEST_CORE === '' ? built : (process.env.DOCATTEST_CORE ?? built);
if (!existsSync(core)) {
  process.stderr.write(`with-core: no core at ${core}; build it with \`npm run build:core\`\n`);
  process.exit(1);
}
const [command, ...args] = process.argv.slice(2);
if (!command) {
  process.stderr.write('usage: node scripts/with-core.ts <command> [args...]\n');
  process.exit(1);
}
const done = spawnSync(command, args, {
  stdio: 'inherit',
  env: { ...process.env, DOCATTEST_CORE: core },
  shell: process.platform === 'win32',
});
if (done.error) {
  process.stderr.write(`with-core: cannot run ${command}: ${done.error.message}\n`);
  process.exit(1);
}
process.exit(done.status ?? 1);
