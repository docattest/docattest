import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { findings } from '../findings.ts';
import { report, result } from './report.ts';

describe('findings', () => {
  it('keeps the results that fail or error, and leaves out passes and skips', () => {
    const found = findings(
      report({
        results: [
          result({ page: 'docs/a.md', line: 1, status: 'pass' }),
          result({ page: 'docs/a.md', line: 3, status: 'fail', message: 'expected 3, got 5' }),
          result({ page: 'docs/b.md', line: 2, status: 'skipped' }),
          result({ page: 'docs/b.md', line: 7, status: 'error', kind: 'example' }),
        ],
      }),
    );
    assert.deepEqual(found, [
      {
        page: 'docs/a.md',
        line: 3,
        level: 'error',
        title: 'docattest fact: fail',
        message: 'expected 3, got 5',
      },
      {
        page: 'docs/b.md',
        line: 7,
        level: 'error',
        title: 'docattest example: error',
        message: 'error at docs/b.md',
      },
    ]);
  });

  it('marks a change that fails as an error and one that warns as a warning', () => {
    const found = findings(
      report({
        changes: [
          {
            type: 'orphaned-claim',
            page: 'docs/a.md',
            line: 4,
            severity: 'fail',
            message: 'a claim lost its proof',
          },
          {
            type: 'weakened',
            page: 'docs/a.md',
            line: 9,
            severity: 'warning',
            message: 'an example asserts less',
          },
        ],
      }),
    );
    assert.deepEqual(
      found.map((finding) => [finding.level, finding.title, finding.line]),
      [
        ['error', 'docattest change: orphaned-claim', 4],
        ['warning', 'docattest change: weakened', 9],
      ],
    );
  });

  it("places a member's pages under the member's directory", () => {
    const member = report({ results: [result({ page: 'docs/x.md', line: 2, status: 'fail' })] });
    const found = findings(
      report({ members: [{ path: 'packages/api', adapter: 'python', report: member }] }),
    );
    assert.deepEqual(
      found.map((finding) => `${finding.page}:${finding.line}`),
      ['packages/api/docs/x.md:2'],
    );
  });
});
