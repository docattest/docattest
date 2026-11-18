#!/usr/bin/env node
// Make the archive a Go user installs docattest from, for one platform, from a core built
// for it:
//
//   node scripts/release/go-archive.ts --platform linux-x64 \
//     --core core/target/release/docattest --out <directory>
//
// writes <directory>/docattest-go-<version>-<platform>.tar.gz, which unpacks to one
// directory holding the launcher (`docattest`), the Go adapter (`docattest-adapter-go`) and
// the core beside them (`docattest-core`), where the launcher looks for it, with the
// license and the guide (`README.md` and its pages, `docs/`). `go install`
// cannot bring the core, which is not written in Go; this archive is how the Go package
// delivers it. The two Go programs are cross-compiled here, so any runner makes every
// platform's archive. The core is refused when its header says it was built for another
// platform.
import process from 'node:process';
import { chmodSync, copyFileSync, cpSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { commandLine } from '../lib/command-line.ts';
import { version } from '../lib/core-pin.ts';
import { fail } from '../lib/fail.ts';
import { platform, type Platform } from '../lib/platforms.ts';
import { ROOT } from '../lib/repository.ts';
import { builtFor } from './built-for.ts';
import { run } from './run.ts';

const USAGE =
  'usage: node scripts/release/go-archive.ts --platform <os-arch> --core <binary> ' +
  '--out <directory>';
const PACKAGE = join(ROOT, 'adapters', 'go');

/** Go's names for a platform's system and processor. */
const goTarget = (entry: Platform): { GOOS: string; GOARCH: string } => ({
  GOOS: entry.os === 'win32' ? 'windows' : entry.os,
  GOARCH: entry.cpu === 'x64' ? 'amd64' : entry.cpu,
});

/** The name of the archive for `name`, without its extension: also its one directory. */
const archiveName = (name: string): string => `docattest-go-${version()}-${name}`;

const { values: args } = commandLine(
  { options: { platform: { type: 'string' }, core: { type: 'string' }, out: { type: 'string' } } },
  USAGE,
);
if (args.platform === undefined || args.core === undefined || args.out === undefined) fail(USAGE);
const entry = platform(args.platform);
const found = builtFor(readFileSync(args.core));
if (found?.os !== entry.os || found.cpu !== entry.cpu) {
  fail(`${args.core} is not a core for ${args.platform} (${entry.os} ${entry.cpu})`);
}
const out = resolve(args.out);
const name = archiveName(args.platform);
const stage = join(out, name);
rmSync(stage, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });
run('go', ['build', '-trimpath', '-o', `${stage}/`, './cmd/...'], {
  cwd: PACKAGE,
  env: { ...process.env, ...goTarget(entry), CGO_ENABLED: '0' },
});
const core = join(stage, entry.binary.replace('docattest', 'docattest-core'));
copyFileSync(args.core, core);
chmodSync(core, 0o755);
for (const file of ['LICENSE', 'README.md']) copyFileSync(join(PACKAGE, file), join(stage, file));
// The README links to the guide's pages, one for each kind of proof.
cpSync(join(PACKAGE, 'docs'), join(stage, 'docs'), { recursive: true });
run('tar', ['-czf', `${name}.tar.gz`, name], { cwd: out });
rmSync(stage, { recursive: true });
process.stdout.write(`${join(out, `${name}.tar.gz`)}\n`);
