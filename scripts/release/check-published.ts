#!/usr/bin/env node
// Refuse to publish the docattest package before the packages that carry its core: run by
// `prepublishOnly`, and by the release workflow before it publishes the main package.
//
//   node scripts/release/check-published.ts
//
// The main package names one optional dependency per platform at its own exact version; npm
// skips an optional dependency it cannot fetch without a word, so a main package published
// first would install everywhere and run nowhere. Each one is asked of the registry. That
// the main package names each of them at the release's version is `scripts/version.ts
// check`'s to hold, and both callers run it first.
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { version } from '../lib/core-pin.ts';
import { fail } from '../lib/fail.ts';
import { corePackages } from '../lib/platforms.ts';

const pinned = version();
const missing = corePackages().filter((name) => {
  const asked = spawnSync('npm', ['view', `${name}@${pinned}`, 'version'], {
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });
  return asked.status !== 0 || asked.stdout.trim() !== pinned;
});
if (missing.length > 0) {
  fail(
    `not published yet, so the docattest package would install without a core:\n` +
      missing.map((name) => `  ${name}@${pinned}\n`).join('') +
      'Publish the platform packages first (see CONTRIBUTING.md, "Releasing").',
  );
}
process.stdout.write(`every platform package of ${pinned} is on the registry\n`);
