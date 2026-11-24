import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { ciVerdict } from './ci-runs.ts';

const passed = { status: 'completed', conclusion: 'success' };
const failed = { status: 'completed', conclusion: 'failure' };
const running = { status: 'in_progress', conclusion: null };

describe('ciVerdict', () => {
  it('a commit with a successful run has passed, even beside a failed one', () => {
    assert.equal(ciVerdict([passed]), 'passed');
    assert.equal(ciVerdict([failed, passed]), 'passed');
  });

  it('a commit whose runs all finished without success has failed', () => {
    assert.equal(ciVerdict([failed]), 'failed');
    assert.equal(ciVerdict([failed, { status: 'completed', conclusion: 'cancelled' }]), 'failed');
  });

  it('a commit with an unfinished run and no success is still running', () => {
    assert.equal(ciVerdict([running]), 'running');
    assert.equal(ciVerdict([failed, running]), 'running');
  });

  it('a commit CI never ran has no verdict', () => {
    assert.equal(ciVerdict([]), 'none');
  });
});
