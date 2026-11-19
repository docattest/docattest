import { describe, expect, it } from 'vitest';
import {
  invocationOf,
  namePattern,
  type NodeRuntime,
} from '../../../../src/language/test-runs/invocation.js';

const NODE: NodeRuntime = {
  execPath: '/bin/node',
  stripsTypes: true,
  reporter: 'file:///adapter/reporter.js',
};
const REQUEST = {
  file: '/p/test/a.test.ts',
  paths: [['retry', 'gives up'], ['alone']],
  prefixes: [],
  reportFile: '/tmp/report',
};

describe('namePattern', () => {
  it('matches exactly the full names, every character of a title taken literally', () => {
    const titles = ['a.b (1+1)*', 'x|y', '[^$]\\d{2}?'];
    const pattern = new RegExp(namePattern([titles.slice(0, 2), titles.slice(2)]));
    expect(pattern.test('a.b (1+1)* x|y')).toBe(true);
    expect(pattern.test('[^$]\\d{2}?')).toBe(true);
    expect(pattern.test('axb (1+1)* x|y')).toBe(false);
    expect(pattern.test('a.b (1+1)* x')).toBe(false);
    expect(pattern.test('y')).toBe(false);
    expect(pattern.test('one [^$]\\d{2}?')).toBe(false);
  });

  it('joins describe titles and title by single spaces, and names a name once', () => {
    expect(namePattern([['price', 'rounds'], ['price rounds']])).toBe('^(?:price rounds)$');
  });

  it('selects every test under describe titles, for a table whose rows are not known', () => {
    const pattern = new RegExp(namePattern([], [['code (spans)']]));
    expect(pattern.test('code (spans) one row')).toBe(true);
    expect(pattern.test('code (spans) ')).toBe(false);
    expect(pattern.test('code (spans)')).toBe(false);
    expect(pattern.test('codes one row')).toBe(false);
    expect(new RegExp(namePattern([], [[]])).test('any test at all')).toBe(true);
  });
});

describe('invocationOf', () => {
  it('runs Vitest on the file alone, selecting by name, with a JSON report', () => {
    const runner = { kind: 'vitest', dir: '/p', bin: '/p/node_modules/vitest/vitest.mjs' } as const;
    expect(invocationOf(runner, REQUEST, NODE)).toEqual({
      command: '/bin/node',
      args: [
        '/p/node_modules/vitest/vitest.mjs',
        'run',
        '/p/test/a.test.ts',
        '--testNamePattern=^(?:retry gives up|alone)$',
        '--reporter=json',
        '--outputFile=/tmp/report',
      ],
      cwd: '/p',
    });
  });

  it('runs Jest on exactly the file by its path, with its JSON report', () => {
    const runner = { kind: 'jest', dir: '/p', bin: '/p/node_modules/jest/bin/jest.js' } as const;
    expect(invocationOf(runner, REQUEST, NODE).args).toEqual([
      '/p/node_modules/jest/bin/jest.js',
      '--json',
      '--outputFile=/tmp/report',
      '--testNamePattern=^(?:retry gives up|alone)$',
      '--runTestsByPath',
      '/p/test/a.test.ts',
    ]);
  });

  it("runs node:test with the adapter's reporter, stripping types where Node does not", () => {
    const runner = { kind: 'node:test', dir: '/p' } as const;
    const args = [
      '--test',
      '--test-name-pattern=^(?:retry gives up|alone)$',
      '--test-reporter=file:///adapter/reporter.js',
      '--test-reporter-destination=/tmp/report',
      '/p/test/a.test.ts',
    ];
    expect(invocationOf(runner, REQUEST, NODE)).toEqual({ command: '/bin/node', args, cwd: '/p' });
    const older = invocationOf(runner, REQUEST, { ...NODE, stripsTypes: false });
    expect(older.args).toEqual([
      '--experimental-strip-types',
      '--disable-warning=ExperimentalWarning',
      ...args,
    ]);
  });
});
