// Where a language package finds the docattest core (PROTOCOL.md §1, "Launchers"). The
// Python launcher (adapters/python/docattest/launcher.py) follows the same rules, so that
// an npm install and a pip install on one machine can share one binary.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import * as path from 'node:path';

/** The core a package was built for: its exact version, and the protocol it speaks. */
export interface Pinned {
  version: string;
  protocol: number;
}

/** What the lookup reads; a test supplies its own. */
export interface Surroundings {
  env: Record<string, string | undefined>;
  /** `process.platform` and `process.arch`. */
  platform: string;
  arch: string;
  home: string;
  /** Where the package's own binary is, when it brought one for this platform. */
  bundled: string | undefined;
  /** `core --version --json`, or undefined when the program does not answer it. */
  ask: (core: string) => string | undefined;
  /** What the package tells a user who has no core: why, and how to get one. */
  remedy?: string | undefined;
}

/** The core to run, or why there is none. */
export type Found = { core: string } | { error: string };

/** A variable's value; one that is set to nothing counts as not set. */
export const given = (value: string | undefined) => (value === '' ? undefined : value);

/** `<os>-<arch>`, in the names every package uses: `darwin-arm64`, `linux-x64`, `windows-x64`. */
export function platformName(platform: string, arch: string): string {
  const os = platform === 'win32' ? 'windows' : platform;
  const cpu = { x86_64: 'x64', amd64: 'x64', aarch64: 'arm64' }[arch.toLowerCase()];
  return `${os}-${cpu ?? arch.toLowerCase()}`;
}

/**
 * docattest's own cache directory: `DOCATTEST_CACHE`, else `docattest` in the user's cache
 * directory (`~/Library/Caches` on macOS, `%LOCALAPPDATA%` on Windows, `$XDG_CACHE_HOME`
 * or `~/.cache` elsewhere).
 */
export function cacheDirectory(around: Pick<Surroundings, 'env' | 'platform' | 'home'>): string {
  const { env, platform, home } = around;
  if (env.DOCATTEST_CACHE) return env.DOCATTEST_CACHE;
  // Each platform's own separator, whatever platform this code runs on.
  const paths = platform === 'win32' ? path.win32 : path.posix;
  if (platform === 'darwin') return paths.join(home, 'Library', 'Caches', 'docattest');
  if (platform === 'win32') {
    const local = given(env.LOCALAPPDATA) ?? paths.join(home, 'AppData', 'Local');
    return paths.join(local, 'docattest');
  }
  const xdg = env.XDG_CACHE_HOME;
  const user = xdg && paths.isAbsolute(xdg) ? xdg : paths.join(home, '.cache');
  return paths.join(user, 'docattest');
}

/** Where a core of the pinned version lies in the shared cache. */
export function cachedCore(pinned: Pinned, around: Surroundings): string {
  const paths = around.platform === 'win32' ? path.win32 : path.posix;
  const name = around.platform === 'win32' ? 'docattest.exe' : 'docattest';
  const platform = platformName(around.platform, around.arch);
  return paths.join(cacheDirectory(around), 'core', pinned.version, platform, name);
}

/**
 * The core, in order of preference: `DOCATTEST_CORE`; the binary the package brought; the
 * shared cache. The last two are trusted without being run: the package brought the first
 * itself, and the cache path names the exact version. A core someone else points at is
 * asked what protocol it speaks, and refused when it is not this package's.
 */
export function findCore(pinned: Pinned, around: Surroundings, exists = existsSync): Found {
  const given = around.env.DOCATTEST_CORE;
  if (given) return checked(given, pinned, around.ask);
  if (around.bundled !== undefined && exists(around.bundled)) return { core: around.bundled };
  const cached = cachedCore(pinned, around);
  if (exists(cached)) return { core: cached };
  const platform = platformName(around.platform, around.arch);
  const brought = around.bundled ?? 'it brought no binary for this platform';
  const remedy = around.remedy === undefined ? '' : `\n${around.remedy}`;
  return {
    error:
      `docattest: no docattest core ${pinned.version} for ${platform}. Looked in: ` +
      `DOCATTEST_CORE (not set); this package (${brought}); the shared cache (${cached})` +
      remedy,
  };
}

function checked(core: string, pinned: Pinned, ask: Surroundings['ask']): Found {
  const given = `docattest: DOCATTEST_CORE (${core})`;
  const told = versionOf(ask(core));
  if (told === undefined) {
    return { error: `${given} is not a docattest core: it does not answer --version --json` };
  }
  if (told.protocol !== pinned.protocol) {
    return {
      error:
        `${given} is core ${told.version}, which speaks adapter protocol ${told.protocol}; ` +
        `this package speaks protocol ${pinned.protocol} (it was built for core ${pinned.version})`,
    };
  }
  return { core };
}

function versionOf(answer: string | undefined): Pinned | undefined {
  try {
    const told: unknown = JSON.parse(answer ?? '');
    if (typeof told !== 'object' || told === null) return undefined;
    const { version, protocol } = told as Record<string, unknown>;
    if (typeof version !== 'string' || typeof protocol !== 'number') return undefined;
    return { version, protocol };
  } catch {
    return undefined;
  }
}

/** Ask a core for its version by running it; nothing else is started by this. */
function askCore(core: string): string | undefined {
  const done = spawnSync(core, ['--version', '--json'], { encoding: 'utf8', timeout: 10_000 });
  return done.status === 0 ? done.stdout : undefined;
}

/** This process's surroundings. */
export function surroundings(bundled: string | undefined, remedy?: string): Surroundings {
  return {
    env: process.env,
    platform: process.platform,
    arch: process.arch,
    home: homedir(),
    bundled,
    ask: askCore,
    remedy,
  };
}
