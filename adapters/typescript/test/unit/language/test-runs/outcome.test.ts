import { describe, expect, it } from 'vitest';
import {
  outcomeOf,
  type Ended,
  type ReportedTest,
  type RunnerReport,
} from '../../../../src/language/test-runs/outcome.js';

const PASSED: Ended = { status: 'passed' };
const failed = (message: string): Ended => ({ status: 'failed', message });
const skipped = (reason?: string): Ended => ({ status: 'skipped', reason });
const test = (path: string[], ended: Ended): ReportedTest => ({ path, ended });
const report = (...tests: ReportedTest[]): RunnerReport => ({ tests, fileFailure: undefined });

describe('outcomeOf', () => {
  it('reads the test of exactly the cited path, not one whose titles join to the same name', () => {
    const ran = report(test(['price', 'rounds'], failed('wrong')), test(['price rounds'], PASSED));
    expect(outcomeOf(ran, [['price rounds']], 'Vitest')).toEqual({ status: 'passed' });
    expect(outcomeOf(ran, [['price', 'rounds']], 'Vitest')).toEqual({
      status: 'failed',
      message: 'wrong',
    });
    const skippedOne = report(test(['price', 'rounds'], skipped()), test(['price rounds'], PASSED));
    expect(outcomeOf(skippedOne, [['price rounds']], 'Vitest')).toEqual({ status: 'passed' });
  });

  it("gives a failure's own lines, without its stack", () => {
    const message = 'AssertionError: one is not two\n\n- 2\n+ 1\n    at f (/p/a.test.ts:3:9)\n';
    expect(outcomeOf(report(test(['a'], failed(message))), [['a']], 'Vitest')).toEqual({
      status: 'failed',
      message: 'AssertionError: one is not two\n- 2\n+ 1',
    });
    expect(outcomeOf(report(test(['a'], failed(''))), [['a']], 'Jest')).toEqual({
      status: 'failed',
      message: 'it failed, with no message',
    });
  });

  it('passes a table cited as a whole only when every row does, and names a failing row', () => {
    const rows = [
      ['t', 'adds 1'],
      ['t', 'adds 2'],
    ];
    const good = report(test(['t', 'adds 1'], PASSED), test(['t', 'adds 2'], PASSED));
    expect(outcomeOf(good, rows, 'Vitest')).toEqual({ status: 'passed' });
    const bad = report(test(['t', 'adds 1'], PASSED), test(['t', 'adds 2'], failed('3 is not 4')));
    expect(outcomeOf(bad, rows, 'Vitest')).toEqual({
      status: 'failed',
      message: 'adds 2: 3 is not 4',
    });
    const one = report(test(['t', 'adds 1'], PASSED));
    expect(outcomeOf(one, rows, 'Vitest')).toEqual({
      status: 'notRun',
      message: 'Vitest selected no test named "t" > "adds 2"',
    });
  });

  it("fails a test whose subtest failed with the subtest's message, not the summary", () => {
    const ran = report(
      test(['cases', 'first'], PASSED),
      test(['cases', 'second'], failed('second broke')),
      test(['cases'], failed('1 subtest failed')),
    );
    expect(outcomeOf(ran, [['cases']], 'node:test')).toEqual({
      status: 'failed',
      message: 'second: second broke',
    });
  });

  it('says a test was skipped, with the reason when the runner gives one', () => {
    expect(outcomeOf(report(test(['a'], skipped())), [['a']], 'Vitest')).toEqual({
      status: 'skipped',
      message: 'Vitest skipped it',
    });
    expect(outcomeOf(report(test(['a'], skipped('not today'))), [['a']], 'node:test')).toEqual({
      status: 'skipped',
      message: 'not today',
    });
  });

  it('runs nothing of a path the run selected no test by, or of a file that cannot run', () => {
    expect(outcomeOf(report(), [['a']], 'Jest')).toEqual({
      status: 'notRun',
      message: 'Jest selected no test named "a"',
    });
    const broken = { tests: [], fileFailure: 'Error: no module\n    at load (x.js:1:1)' };
    expect(outcomeOf(broken, [['a']], 'Jest')).toEqual({
      status: 'notRun',
      message: 'Jest could not run the file: Error: no module',
    });
    expect(outcomeOf(report(), [], 'Vitest')).toEqual({
      status: 'notRun',
      message: 'Vitest has nothing to run: the table has no rows',
    });
  });
});
