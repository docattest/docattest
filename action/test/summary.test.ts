import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { findings } from '../findings.ts';
import { jobSummary, verdict } from '../summary.ts';
import { report, result } from './report.ts';

const failing = report({
  exitCode: 1,
  results: [
    result({ page: 'docs/a.md', line: 1, status: 'pass' }),
    result({ page: 'docs/a.md', line: 3, status: 'fail', message: 'expected `a|b`, got c' }),
  ],
  members: [
    {
      path: 'tools',
      adapter: 'go',
      report: report({ results: [result({ page: 'README.md', line: 5, status: 'skipped' })] }),
    },
  ],
});

describe('verdict', () => {
  it("counts the results of the report and its members' reports", () => {
    assert.equal(verdict(failing), 'docattest: 1 pass, 1 fail, 0 error, 1 skipped: FAILED');
  });

  it('says ok when the exit code is 0', () => {
    assert.match(verdict(report({})), /: ok$/);
  });
});

describe('jobSummary', () => {
  it('lists each finding in a table row, with a | in the message escaped', () => {
    const summary = jobSummary(failing, findings(failing));
    assert.ok(summary.startsWith('### ✗ docattest: 1 pass, 1 fail'));
    assert.ok(summary.includes('| docs/a.md:3 | docattest fact: fail | expected `a\\|b`, got c |'));
  });

  it('is only the verdict when nothing needs attention', () => {
    assert.equal(
      jobSummary(report({}), []),
      '### ✓ docattest: 0 pass, 0 fail, 0 error, 0 skipped: ok\n\n',
    );
  });
});
