// Generates glob-cases.json, the differential fixture for src/files/glob/, from the real
// picomatch the npm package uses (adapters/typescript/src/files/glob.ts):
//
//   globMatcher(globs) = picomatch(globs.map((g) => g.replace(/^\.\//, '')), { dot: true })
//   isGlob(pattern)    = picomatch.scan(pattern).isGlob
//
// Run from anywhere: `node core/tests/fixtures/gen-glob.ts [path/to/picomatch]`. By default
// picomatch resolves from this file's location (the repository's node_modules). The output is
// deterministic (seeded PRNG). For wider exploratory runs, GLOB_CASES_SEED, GLOB_CASES_SCALE
// (a multiplier on the random case counts) and GLOB_CASES_OUT override the defaults.
//
// Fixture shape:
//   paths:  every path string tested (sets refer to them by index)
//   sets:   { globs, error | null, source?, valid?, p: [path index…], r: "0101…" }
//           `source` is the string picomatch passed to `new RegExp` (single-glob sets only;
//           `sourceUnits`, its UTF-16 code units, when it holds a lone surrogate) and `valid`
//           whether that compiled; `r[k]` is the match result for path `p[k]`.
//           Where picomatch never returns, a set is just { globs, hangs: true }.
//   isGlob: [pattern, boolean]…
//
// The patterns, the paths and the PRNG are in gen-glob/; picomatch is loaded and run here.
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import type picomatchModule from 'picomatch';
import { curated } from './gen-glob/curated.ts';
import { curatedPaths, instantiate, randomPath } from './gen-glob/paths.ts';
import { randomPattern, realisticPattern, regexPaths, regexPattern } from './gen-glob/patterns.ts';
import { int, pick, rand, scale } from './gen-glob/random.ts';

/** One set of globs matched together, as the fixture holds it (see the shape above). */
interface GlobSet {
  globs: string[];
  error?: string | null;
  source?: string;
  sourceUnits?: number[];
  valid?: boolean | null;
  p?: number[];
  r?: string;
  hangs?: true;
}

/** glob-cases.json. */
interface GlobCases {
  picomatch: string;
  options: { dot: true };
  paths: string[];
  sets: GlobSet[];
  isGlob: [pattern: string, isGlob: boolean][];
}

/** picomatch's internal module, whose `toRegex` makes the RegExp of a source. */
interface PicomatchLib {
  toRegex: (source: string, options?: unknown) => RegExp;
}

const require = createRequire(import.meta.url);
const target = process.argv[2] ?? 'picomatch';
const base = target.endsWith('/') ? target : `${target}/`;
const picomatch = require(target) as typeof picomatchModule;
const lib = require(`${base}lib/picomatch.js`) as PicomatchLib;
const { version } = require(`${base}package.json`) as { version: string };
if (version !== '4.0.7') throw new Error(`expected picomatch 4.0.7, found ${version}`);

/** The source picomatch last handed to `new RegExp`, and whether it compiled. */
class RegexRecorder {
  private source: string | null = null;
  private valid: boolean | null = null;

  constructor(into: PicomatchLib) {
    const toRegex = into.toRegex;
    into.toRegex = (source, options) => {
      this.source = source;
      try {
        new RegExp(source);
        this.valid = true;
      } catch {
        this.valid = false;
      }
      return toRegex(source, options);
    };
  }

  reset(): void {
    this.source = null;
    this.valid = null;
  }

  last(): { source: string | null; valid: boolean | null } {
    return { source: this.source, valid: this.valid };
  }
}
const recorder = new RegexRecorder(lib);

// --- run picomatch -------------------------------------------------------------------------
const pathIndex = new Map<string, number>();
const paths: string[] = [];
const indexOf = (p: string): number => {
  if (!p.isWellFormed()) throw new Error(`lone surrogate in ${JSON.stringify(p)}`);
  let index = pathIndex.get(p);
  if (index === undefined) {
    index = paths.length;
    pathIndex.set(p, index);
    paths.push(p);
  }
  return index;
};
for (const p of curatedPaths) indexOf(p);

// picomatch loops forever on some patterns (a backslash then 3+ backslashes at the end);
// probe anything ending in backslashes in a child process with a timeout.
const hangs = (pattern: string): boolean => {
  if (!/\\\\$/.test(pattern)) return false;
  const script = `const pm=require(${JSON.stringify(require.resolve(target))});pm(${JSON.stringify(pattern)},{dot:true});`;
  const r = spawnSync(process.execPath, ['-e', script], { timeout: 3000 });
  return (
    (r.error as NodeJS.ErrnoException | undefined)?.code === 'ETIMEDOUT' || r.signal === 'SIGTERM'
  );
};

const sets: GlobSet[] = [];
const isGlob: GlobCases['isGlob'] = [];
const seenGlobs = new Set<string>();

const matcherFor = (globs: readonly string[]): ((path: string) => boolean) =>
  globs.length === 0
    ? () => false
    : picomatch(
        globs.map((g) => g.replace(/^\.\//, '')),
        { dot: true },
      );

// A source can split a surrogate pair (`\\😀` escapes each half); JSON cannot carry that as
// a string, so such sources are stored as UTF-16 code units.
const recordSource = (set: GlobSet, source: string, valid: boolean | null): void => {
  if (source.isWellFormed()) set.source = source;
  else set.sourceUnits = Array.from({ length: source.length }, (_, i) => source.charCodeAt(i));
  set.valid = valid;
};

const candidatesFor = (extraPaths: readonly string[], allCurated: boolean): Set<string> => {
  const candidates = new Set(extraPaths);
  if (allCurated) for (const p of curatedPaths) candidates.add(p);
  else for (let i = 0; i < 12; i++) candidates.add(pick(curatedPaths));
  return candidates;
};

const addResults = (
  set: GlobSet,
  candidates: Set<string>,
  matcher: (path: string) => boolean,
): void => {
  set.p = [];
  set.r = '';
  for (const p of candidates) {
    if (!p.isWellFormed()) continue;
    set.p.push(indexOf(p));
    set.r += matcher(p) ? '1' : '0';
  }
};

const addSet = (globs: string[], extraPaths: readonly string[], allCurated: boolean): void => {
  const key = JSON.stringify(globs);
  if (seenGlobs.has(key)) return;
  seenGlobs.add(key);
  if (globs.some((g) => hangs(g.replace(/^\.\//, '')))) {
    // The port reports an error instead of spinning.
    sets.push({ globs, hangs: true });
    return;
  }
  const set: GlobSet = { globs, error: null };
  let matcher: (path: string) => boolean;
  recorder.reset();
  try {
    matcher = matcherFor(globs);
  } catch (e) {
    set.error = (e as Error).message;
    sets.push(set);
    return;
  }
  const { source, valid } = recorder.last();
  if (globs.length === 1 && source !== null) recordSource(set, source, valid);
  addResults(set, candidatesFor(extraPaths, allCurated), matcher);
  sets.push(set);
};

const withExamples = (globs: readonly string[], count: number): string[] => {
  const extra: string[] = [];
  for (let i = 0; i < count; i++) {
    const g = pick(globs);
    extra.push(instantiate(g));
    extra.push(randomPath());
  }
  return extra;
};
for (const g of curated) addSet([g], withExamples([g], 4), true);
addSet([], [], true);
addSet([''], [], false);
addSet(['./'], [], false);
addSet(['a', ''], [], false);
addSet(['a/'.repeat(32769)], ['a'], false);
addSet(['*'.repeat(65537)], ['a'], false);
addSet(['x/\\\\\\\\'], [], false);
addSet(['a/\\\\\\\\\\'], [], false);
addSet(['docs/*.md', 'src/**/*.ts'], ['docs/a.md', 'src/a/b.ts', 'src/a.md'], true);
addSet(['!docs/**', 'docs/a.md'], ['docs/a.md', 'docs/b.md', 'x'], true);
addSet(['README.md', '.github/**', '**/*.{md,mdx}'], [], true);

for (let i = 0; i < 1500 * scale; i++) {
  const g = randomPattern();
  addSet([g], withExamples([g], 3), false);
}
for (let i = 0; i < 800 * scale; i++) {
  const g = realisticPattern();
  addSet([g], withExamples([g], 4), false);
}
for (let i = 0; i < 700 * scale; i++) {
  const g = regexPattern();
  addSet([g], [...regexPaths(), ...withExamples([g], 2)], false);
}
for (let i = 0; i < 200 * scale; i++) {
  const globs: string[] = [];
  const n = 1 + int(3);
  for (let k = 0; k < n; k++) globs.push(rand() < 0.5 ? realisticPattern() : randomPattern());
  addSet(globs, withExamples(globs, 3), false);
}

const globSamples = new Set([...curated]);
for (let i = 0; i < 3000; i++) globSamples.add(randomPattern());
for (let i = 0; i < 500; i++) globSamples.add(realisticPattern());
for (const p of curatedPaths) globSamples.add(p);
for (const g of globSamples) isGlob.push([g, picomatch.scan(g).isGlob]);

const fixture: GlobCases = { picomatch: version, options: { dot: true }, paths, sets, isGlob };
const out = process.env.GLOB_CASES_OUT
  ? new URL(process.env.GLOB_CASES_OUT, `file://${process.cwd()}/`)
  : new URL('./glob-cases.json', import.meta.url);
writeFileSync(out, `${JSON.stringify(fixture)}\n`);
const matchCases = sets.reduce((n, s) => n + (s.p ? s.p.length : 1), 0);
console.log(
  `${sets.length} sets, ${matchCases} match cases, ${isGlob.length} isGlob cases -> ${out.pathname}`,
);
