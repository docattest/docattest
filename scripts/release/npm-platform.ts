#!/usr/bin/env node
// Make the npm package that carries the docattest core for one platform, from a core built
// for it:
//
//   node scripts/release/npm-platform.ts --platform darwin-arm64 \
//     --core core/target/release/docattest --out <directory> [--pack <directory>]
//
// <directory> becomes the package, and holds nothing else: package.json (its name from
// protocol/platforms.json, the version of protocol/core.json, `os` and `cpu` so that npm
// installs it only where it runs) and bin/docattest (bin/docattest.exe on Windows), with the
// executable bit. The core is a static binary on Linux, so no `libc` is named. `--pack`
// also runs `npm pack` into another directory. The binary is refused when its header says it
// was built for another platform.
import process from 'node:process';
import { chmodSync, copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { commandLine } from '../lib/command-line.ts';
import { version } from '../lib/core-pin.ts';
import { fail } from '../lib/fail.ts';
import { npmName, platform, type Platform } from '../lib/platforms.ts';
import { readJson } from '../lib/repository.ts';
import { builtFor } from './built-for.ts';
import { run } from './run.ts';

/** What the platform package copies from the npm package's manifest. */
type Manifest = Record<string, unknown> & { license: string };

const USAGE =
  'usage: node scripts/release/npm-platform.ts --platform <os-arch> --core <binary> ' +
  '--out <directory> [--pack <directory>]';

/** The package.json of the platform package. */
function manifest(name: string, entry: Platform): Record<string, unknown> {
  const main = readJson('adapters/typescript/package.json') as Manifest;
  const optional = ['author', 'repository', 'homepage', 'bugs'];
  return {
    name: npmName(name),
    version: version(),
    description: `The docattest core for ${name}, installed by the docattest package.`,
    license: main.license,
    ...Object.fromEntries(optional.filter((key) => key in main).map((key) => [key, main[key]])),
    os: [entry.os],
    cpu: [entry.cpu],
    // Yarn's Plug'n'Play must leave the binary on disk to run it.
    preferUnplugged: true,
    files: ['bin'],
  };
}

/** Refuse a binary built for another platform than the package's. */
function checkBinary(core: string, name: string, entry: Platform): void {
  const found = builtFor(readFileSync(core));
  if (found?.os !== entry.os || found.cpu !== entry.cpu) {
    const what = found ? `${found.os}-${found.cpu ?? 'unknown'}` : 'no known executable format';
    fail(`${core} is not a core for ${name} (${entry.os} ${entry.cpu}): it is ${what}`);
  }
}

const { values: args } = commandLine(
  {
    options: {
      platform: { type: 'string' },
      core: { type: 'string' },
      out: { type: 'string' },
      pack: { type: 'string' },
    },
  },
  USAGE,
);
if (args.platform === undefined || args.core === undefined || args.out === undefined) fail(USAGE);
const entry = platform(args.platform);
checkBinary(args.core, args.platform, entry);
const out = resolve(args.out);
rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'bin'), { recursive: true });
writeFileSync(
  join(out, 'package.json'),
  `${JSON.stringify(manifest(args.platform, entry), null, 2)}\n`,
);
copyFileSync(args.core, join(out, 'bin', entry.binary));
chmodSync(join(out, 'bin', entry.binary), 0o755);
process.stdout.write(`${out}\n`);
if (args.pack !== undefined) {
  mkdirSync(args.pack, { recursive: true });
  run('npm', ['pack', '--pack-destination', resolve(args.pack)], { cwd: out });
}
