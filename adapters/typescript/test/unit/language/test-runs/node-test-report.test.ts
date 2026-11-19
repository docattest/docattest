import { describe, expect, it } from 'vitest';
import { readNodeTestReport } from '../../../../src/language/test-runs/node-test-report.js';

const FILE = '/project/test/a.test.ts';

// What the adapter's reporter wrote for `node --test` on Node 22.12 and 24 alike (the same
// lines on both), with the file's path made neutral.
const RECORDED = [
  '{"event":"start","name":"rounding","nesting":0}',
  '{"event":"start","name":"rounds up","nesting":1}',
  '{"event":"fail","name":"rounds up","nesting":1,"suite":false,"skip":false,"todo":false,"message":"5 cents is too many"}',
  '{"event":"fail","name":"rounding","nesting":0,"suite":true,"skip":false,"todo":false,"message":"1 subtest failed"}',
  '{"event":"start","name":"rounding rounds up","nesting":0}',
  '{"event":"pass","name":"rounding rounds up","nesting":0,"suite":false,"skip":false,"todo":false}',
  '{"event":"start","name":"multiplies","nesting":0}',
  '{"event":"pass","name":"multiplies","nesting":0,"suite":false,"skip":true,"todo":false}',
  '{"event":"start","name":"skips inside","nesting":0}',
  '{"event":"pass","name":"skips inside","nesting":0,"suite":false,"skip":"no reason today","todo":false}',
  '{"event":"start","name":"todo one","nesting":0}',
  '{"event":"pass","name":"todo one","nesting":0,"suite":false,"skip":false,"todo":true}',
  '{"event":"start","name":"has subtests","nesting":0}',
  '{"event":"start","name":"sub one","nesting":1}',
  '{"event":"pass","name":"sub one","nesting":1,"suite":false,"skip":false,"todo":false}',
  '{"event":"start","name":"sub two","nesting":1}',
  '{"event":"fail","name":"sub two","nesting":1,"suite":false,"skip":false,"todo":false,"message":"sub two broke"}',
  '{"event":"fail","name":"has subtests","nesting":0,"suite":false,"skip":false,"todo":false,"message":"1 subtest failed"}',
].join('\n');

const CANNOT_LOAD = [
  '{"event":"stderr","message":"Error [ERR_MODULE_NOT_FOUND]: Cannot find module \'/project/missing.ts\'\\n"}',
  '{"event":"stderr","message":"    at finalizeResolution (node:internal/modules/esm/resolve:274:11)\\n"}',
  `{"event":"start","name":"${FILE}","nesting":0}`,
  `{"event":"fail","name":"${FILE}","nesting":0,"suite":false,"skip":false,"todo":false,"message":"test failed"}`,
].join('\n');

describe('readNodeTestReport', () => {
  it('reads each test by the path of the tests around it, and leaves describe blocks out', () => {
    expect(readNodeTestReport(RECORDED, FILE)).toEqual({
      tests: [
        {
          path: ['rounding', 'rounds up'],
          ended: { status: 'failed', message: '5 cents is too many' },
        },
        { path: ['rounding rounds up'], ended: { status: 'passed' } },
        { path: ['multiplies'], ended: { status: 'skipped', reason: undefined } },
        { path: ['skips inside'], ended: { status: 'skipped', reason: 'no reason today' } },
        { path: ['todo one'], ended: { status: 'skipped', reason: 'it is marked todo' } },
        { path: ['has subtests', 'sub one'], ended: { status: 'passed' } },
        {
          path: ['has subtests', 'sub two'],
          ended: { status: 'failed', message: 'sub two broke' },
        },
        { path: ['has subtests'], ended: { status: 'failed', message: '1 subtest failed' } },
      ],
      fileFailure: undefined,
    });
  });

  it('reads a file that cannot run from what it printed, and as no test', () => {
    expect(readNodeTestReport(CANNOT_LOAD, FILE)).toEqual({
      tests: [],
      fileFailure:
        "Error [ERR_MODULE_NOT_FOUND]: Cannot find module '/project/missing.ts'\n" +
        '    at finalizeResolution (node:internal/modules/esm/resolve:274:11)\n',
    });
  });

  it('reads an empty report as no test run', () => {
    expect(readNodeTestReport('', FILE)).toEqual({ tests: [], fileFailure: undefined });
  });
});
