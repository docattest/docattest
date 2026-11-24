// What this npm package tells the core: which core it brought, how to start its adapter, and
// what command runs docattest in a project that installed it. Shared by the `docattest`
// launcher (main.ts) and the programmatic API (src/client/), so that both run the same core
// in the same way. Imports nothing but Node's own modules and ./locate.js.
import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { findCore, given, platformName, surroundings, type Found, type Pinned } from './locate.js';

/** Default command for npm projects: its own copy, never a download. */
const COMMAND = 'npx --no docattest';

const PACKAGE_MANAGERS: readonly [string, string][] = [
  ['pnpm', 'pnpm exec docattest'],
  ['yarn', 'yarn docattest'],
  ['bun', 'bunx docattest'],
  ['npm', COMMAND],
];

const LOCKFILES: readonly [string, string][] = [
  ['pnpm-lock.yaml', 'pnpm exec docattest'],
  ['yarn.lock', 'yarn docattest'],
  ['bun.lock', 'bunx docattest'],
  ['bun.lockb', 'bunx docattest'],
  ['package-lock.json', COMMAND],
];

/**
 * Detect how to run `docattest` in the target project from its `packageManager` field or
 * lockfile (pnpm → `pnpm exec docattest`, yarn → `yarn docattest`, bun → `bunx docattest`,
 * else `npx --no docattest`).
 */
export function detectCommand(cwd: string, argv: readonly string[] = []): string {
  const start = targetDir(cwd, argv);
  for (let dir = start; ; dir = path.dirname(dir)) {
    const found = commandInDir(dir);
    if (found) return found;
    if (path.dirname(dir) === dir) return COMMAND;
  }
}

function targetDir(cwd: string, argv: readonly string[]): string {
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === '--root' && next !== undefined) return path.resolve(cwd, next);
    if (arg?.startsWith('--root=')) return path.resolve(cwd, arg.slice('--root='.length));
  }
  return path.resolve(cwd);
}

function commandInDir(dir: string): string | undefined {
  const pkgPath = path.join(dir, 'package.json');
  if (existsSync(pkgPath)) {
    try {
      const parsed = JSON.parse(readFileSync(pkgPath, 'utf8')) as { packageManager?: unknown };
      if (typeof parsed.packageManager === 'string') {
        const pm = parsed.packageManager.split('@')[0]?.trim();
        const hit = PACKAGE_MANAGERS.find(([name]) => name === pm);
        if (hit) return hit[1];
      }
    } catch {
      // Ignore malformed package.json during command detection.
    }
  }
  for (const [lockfile, command] of LOCKFILES) {
    if (existsSync(path.join(dir, lockfile))) return command;
  }
  return undefined;
}

const here = (relative: string) => fileURLToPath(new URL(relative, import.meta.url));

/**
 * The platforms a core is built for, and the npm package that carries each: a copy of
 * protocol/platforms.json made when this package was built. The package's name is written
 * there once (`@docattest/core-{platform}`), for this launcher and the release scripts alike.
 */
interface Platforms {
  npmPackage: string;
  platforms: Record<string, unknown>;
}

const platforms = (): Platforms =>
  JSON.parse(readFileSync(here('../platforms.json'), 'utf8')) as Platforms;

/** The npm package that carries the core for `platform`. */
const corePackage = (known: Platforms, platform: string) =>
  known.npmPackage.replace('{platform}', platform);

/**
 * The binary this package brought: an optional dependency for each platform, holding
 * `bin/docattest`. npm installs the one whose `os` and `cpu` fit, and skips the others.
 */
function bundled(known: Platforms, platform: string): string | undefined {
  const name = process.platform === 'win32' ? 'docattest.exe' : 'docattest';
  try {
    return createRequire(import.meta.url).resolve(`${corePackage(known, platform)}/bin/${name}`);
  } catch {
    return undefined;
  }
}

/** The core this package was built for, copied from protocol/core.json when it was built. */
export function pinned(): Pinned {
  return JSON.parse(readFileSync(here('../core.json'), 'utf8')) as Pinned;
}

/** What a user with no core is told: why this install has none, and the ways to get one. */
export function remedy(version: string, platform: string, known: Platforms): string {
  const listed = Object.keys(known.platforms);
  if (!listed.includes(platform)) {
    return (
      `docattest brings a core for ${listed.join(', ')}; there is none for ${platform}. ` +
      `Build one from docattest's source (cargo build --release in its core/ directory) and ` +
      `set DOCATTEST_CORE to it.`
    );
  }
  const wanted = `${corePackage(known, platform)}@${version}`;
  return (
    `The core for ${platform} is the package ${wanted}, an optional dependency of docattest, ` +
    `and it is not installed: npm leaves optional dependencies out with --omit=optional ` +
    `(or --no-optional, or omit=optional in .npmrc), and a lockfile written on another ` +
    `platform can leave out this one's. To fix it, install again with them ` +
    `(npm install --include=optional), or add it to the project ` +
    `(npm install --save-optional ${wanted}), or set DOCATTEST_CORE to a docattest core ` +
    `${version}.`
  );
}

/** The core to run, found as PROTOCOL.md §1 says, or why there is none. */
export function packageCore(): Found {
  const pin = pinned();
  const known = platforms();
  const platform = platformName(process.platform, process.arch);
  const advice = remedy(pin.version, platform, known);
  return findCore(pin, surroundings(bundled(known, platform), advice));
}

/**
 * The environment the core runs in: `env`, plus how to start this package's adapter and
 * what command runs docattest. What is already set stands: a developer's own adapter, a
 * wrapper's own command.
 */
export function coreEnvironment(
  env: NodeJS.ProcessEnv,
  cwd: string = process.cwd(),
  argv: readonly string[] = [],
): NodeJS.ProcessEnv {
  const adapter = [process.execPath, here('../protocol/main.js')];
  return {
    ...env,
    DOCATTEST_ADAPTER_TYPESCRIPT:
      given(env.DOCATTEST_ADAPTER_TYPESCRIPT) ?? JSON.stringify(adapter),
    DOCATTEST_COMMAND: given(env.DOCATTEST_COMMAND) ?? detectCommand(cwd, argv),
  };
}
