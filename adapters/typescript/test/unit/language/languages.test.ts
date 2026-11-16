import { describe, expect, it } from 'vitest';
import { isSourceFile, SOURCE_FILES } from '../../../src/language/languages.js';
import { sourceFileMatcher } from '../../../src/files/glob.js';

// The rule as the adapter wrote it before it became data: a script, not a declaration file.
const script = (file: string) => /\.(m|c)?[jt]sx?$/.test(file) && !/\.d\.(m|c)?ts$/.test(file);

const DIRS = ['', 'src/', 'a/b/c/', '.hidden/', 'a/.b/', 'sp ace/', 'ü/', 'a.ts/', 'x.d.ts/'];
const ODD = ['[b]/', '(p)/', '{q}/', '!n/', '+p/', '@s/', '*/', '?/', 'UP/', '..a/', 'a../', '$/'];
const STEMS = ['a', '', '.', 'A', 'a.b', 'a.d', '.d', 'd', 'a.D', 'x y', '[z]', 'a.test', '*', '?'];
const MORE = ['!', 'é', 'a..d', '(a)', '{a}', 'a.d.d', '.a.d', 'a.ts', '@', '+', 'a,b', 'a|b'];
const SCRIPTS = [
  'ts',
  'tsx',
  'mts',
  'mtsx',
  'cts',
  'ctsx',
  'js',
  'jsx',
  'mjs',
  'mjsx',
  'cjs',
  'cjsx',
];
const DECLARATIONS = ['d.ts', 'd.mts', 'd.cts', 'd.tsx', 'd.js', 'd.mtsx', 'D.ts', 'd.TS'];
const OTHERS = ['TS', 'Ts', 'md', 'json', 'tsxx', 'xts', 'ats', 'ts.map', 'ts.md', 't', 's', 'jts'];
const REST = ['mmts', 'dts', 'sx', 'jsxx', 'cs', 'mjx', 'vue', ''];

/** Project-relative paths of every shape a project can hold: normalised, `/`-separated. */
function paths(): string[] {
  const out: string[] = [];
  for (const dir of [...DIRS, ...ODD]) {
    for (const inner of ['', ...DIRS]) {
      for (const stem of [...STEMS, ...MORE]) {
        for (const ext of [...SCRIPTS, ...DECLARATIONS, ...OTHERS, ...REST]) {
          out.push(`${dir}${inner}${stem}.${ext}`, `${dir}${inner}${stem}${ext}`);
        }
      }
    }
  }
  return out;
}

describe('the source files of the TypeScript adapter', () => {
  it('are, as globs, exactly the scripts that are not declaration files', () => {
    const all = paths();
    expect(all.length).toBeGreaterThan(300_000);
    expect(all.filter((file) => isSourceFile(file) !== script(file))).toEqual([]);
    expect(all.filter(isSourceFile).length).toBeGreaterThan(50_000);
  });

  it('are one rule: the predicate the adapter uses is the data the core is given', () => {
    const fromData = sourceFileMatcher(SOURCE_FILES);
    for (const file of ['a.ts', 'src/a.tsx', 'a.mjs', '.ts', 'x.d.tsx', '.hidden/a.cts']) {
      expect([file, isSourceFile(file), fromData(file)]).toEqual([file, true, true]);
    }
    for (const file of ['a.d.ts', 'a.d.mts', 'a.d.cts', 'a.md', 'ts', 'a.TS', 'a.ts.map', '']) {
      expect([file, isSourceFile(file), fromData(file)]).toEqual([file, false, false]);
    }
  });
});
