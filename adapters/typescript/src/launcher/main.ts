#!/usr/bin/env node
// docattest, as the npm package runs it: a launcher for the docattest core.
//
// The core is one binary for every language and knows nothing of how it was installed. This
// launcher belongs to the language package: it finds the core the package brought, tells it
// how to start this package's adapter and what command runs docattest in a project that
// installed it from npm, and then gets out of the way: arguments, standard streams and the
// exit code are the core's (PROTOCOL.md §1, "Launchers").
//
// It imports nothing but Node's own modules, so that it costs a process start and no more.
import { spawnSync } from 'node:child_process';
import { constants } from 'node:os';
import { given } from '../core-binary/locate.js';
import { coreEnvironment, packageCore } from '../core-binary/package.js';

const found = packageCore();
if ('error' in found) {
  process.stderr.write(`${found.error}\n`);
  process.exit(1);
}
const { core } = found;
// With a trace asked for (the core's own `DOCATTEST_TRACE`, set to anything but nothing), say
// which core runs.
if (given(process.env.DOCATTEST_TRACE) !== undefined) {
  process.stderr.write(`[trace launcher] core ${core}\n`);
}

const argv = process.argv.slice(2);
const done = spawnSync(core, argv, {
  stdio: 'inherit',
  env: coreEnvironment(process.env, process.cwd(), argv),
});
if (done.error) {
  process.stderr.write(`docattest: cannot run the core (${core}): ${done.error.message}\n`);
  process.exit(1);
}
// A core that died of a signal is reported as a shell reports it.
if (done.signal) process.exit(128 + constants.signals[done.signal]);
process.exit(done.status ?? 1);
