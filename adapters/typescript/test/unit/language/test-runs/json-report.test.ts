import { describe, expect, it } from 'vitest';
import { readJsonReport } from '../../../../src/language/test-runs/json-report.js';

const FILE = '/project/test/v.test.ts';

const assertion = (
  ancestorTitles: string[],
  title: string,
  status: string,
  failureMessages: string[] = [],
) => ({
  ancestorTitles,
  fullName: [...ancestorTitles, title].join(' '),
  status,
  title,
  failureMessages,
  meta: {},
});

// What Vitest 3.2 wrote with `--reporter=json` and `-t '^price (adds 3 and 4|rounds|todo one)$'`,
// cut to its test results, with the path made neutral: the tests not selected are `skipped`.
const VITEST = JSON.stringify({
  numTotalTests: 6,
  success: false,
  testResults: [
    {
      name: '/project/test/other.test.ts',
      status: 'passed',
      message: '',
      assertionResults: [assertion([], 'rounds', 'passed')],
    },
    {
      name: FILE,
      status: 'failed',
      message: '',
      assertionResults: [
        assertion(['price'], 'adds 1 and 2', 'skipped'),
        assertion(['price'], 'adds 3 and 4', 'passed'),
        assertion(['price'], 'rounds', 'failed', [
          'AssertionError: expected 1 to be 2 // Object.is equality\n    at /project/test/v.test.ts:5:34',
        ]),
        assertion(['price'], 'todo one', 'todo'),
        assertion([], 'price rounds', 'skipped'),
      ],
    },
  ],
});

// Jest's report has the same shape (Jest is not installed here, so this one is written by
// hand from its documented format): `pending` for a skipped test, and a file that cannot
// run has a message and no results.
const JEST = JSON.stringify({
  success: false,
  testResults: [
    {
      name: FILE,
      status: 'passed',
      message: '',
      assertionResults: [
        assertion(['retry'], 'gives up', 'passed'),
        assertion(['retry'], 'waits', 'pending'),
        assertion(['retry'], 'later', 'disabled'),
      ],
    },
    {
      name: '/project/test/broken.test.ts',
      status: 'failed',
      message:
        "  ● Test suite failed to run\n\n    Cannot find module './gone' from 'broken.test.ts'",
      assertionResults: [],
    },
  ],
});

describe('readJsonReport', () => {
  it("reads the file's tests from Vitest's report by their titles, whatever else ran", () => {
    expect(readJsonReport(VITEST, FILE)).toEqual({
      tests: [
        { path: ['price', 'adds 1 and 2'], ended: { status: 'skipped', reason: undefined } },
        { path: ['price', 'adds 3 and 4'], ended: { status: 'passed' } },
        {
          path: ['price', 'rounds'],
          ended: {
            status: 'failed',
            message:
              'AssertionError: expected 1 to be 2 // Object.is equality\n    at /project/test/v.test.ts:5:34',
          },
        },
        { path: ['price', 'todo one'], ended: { status: 'skipped', reason: 'it is marked todo' } },
        { path: ['price rounds'], ended: { status: 'skipped', reason: undefined } },
      ],
      fileFailure: undefined,
    });
  });

  it("reads Jest's report, and a file that could not run", () => {
    expect(readJsonReport(JEST, FILE)?.tests.map((t) => t.ended.status)).toEqual([
      'passed',
      'skipped',
      'skipped',
    ]);
    expect(readJsonReport(JEST, '/project/test/broken.test.ts')).toEqual({
      tests: [],
      fileFailure:
        "  ● Test suite failed to run\n\n    Cannot find module './gone' from 'broken.test.ts'",
    });
  });

  it('finds nothing of a file the report does not hold, and no report in other text', () => {
    expect(readJsonReport(VITEST, '/project/test/none.test.ts')).toBeUndefined();
    expect(readJsonReport('not json', FILE)).toBeUndefined();
    expect(readJsonReport('{"numTotalTests":0}', FILE)).toBeUndefined();
  });
});
