// protocol/platforms.json: the platforms a core is built for, and the one place the npm
// package name of a core is written.
import { fail } from './fail.ts';
import { readJson, ROOT } from './repository.ts';

/** npm's names for an operating system and a processor. */
export type Os = 'darwin' | 'linux' | 'win32';
export type Cpu = 'x64' | 'arm64';

/** One platform a core is built for, in protocol/platforms.json. */
export interface Platform {
  os: Os;
  cpu: Cpu;
  /** The Rust target triple. */
  rust: string;
  /** The wheel platform tags. */
  wheel: string[];
  /** The core's file name. */
  binary: string;
}

/** protocol/platforms.json. */
export interface Platforms {
  /** The npm package that carries a core, with `{platform}` for `<os>-<arch>`. */
  npmPackage: string;
  platforms: Record<string, Platform>;
}

/** protocol/platforms.json: `{ npmPackage, platforms: { "<os>-<arch>": { os, cpu, … } } }`. */
export const platforms = (root = ROOT): Platforms =>
  readJson('protocol/platforms.json', root) as Platforms;

/** The npm package that carries the core for `platform`. */
export const npmName = (platform: string, root = ROOT): string =>
  platforms(root).npmPackage.replace('{platform}', platform);

/** The npm packages that carry a core, one for each platform, in the file's order. */
export const corePackages = (root = ROOT): string[] =>
  Object.keys(platforms(root).platforms).map((name) => npmName(name, root));

/** One platform's entry, or an error that lists the known ones. */
export function platform(name: string, root = ROOT): Platform {
  const known = platforms(root).platforms;
  const entry = known[name];
  if (!entry) fail(`unknown platform "${name}"; known: ${Object.keys(known).join(', ')}`);
  return entry;
}
