import assert from 'node:assert/strict';
import { describe, expect, it } from 'vitest';
import { describeError, lineIn, show } from '../../../../src/language/worker/describe.js';

const thrown = (run: () => void): unknown => {
  try {
    run();
  } catch (err) {
    return err;
  }
  throw new Error('expected a throw');
};

describe('describeError', () => {
  it('names both sides and the assert call written on the failing line', () => {
    const err = thrown(() => {
      assert.equal(2 + 2, 5);
    });
    expect(describeError(err, 2, 'const x = 1;\nassert.equal(x, 5);')).toEqual({
      message: 'AssertionError: expected 5, got 4 (assert.equal, block line 2)',
      expected: '5',
      actual: '4',
    });
  });

  it('keeps a custom assertion message and says the negation plainly', () => {
    expect(
      describeError(
        thrown(() => {
          assert.ok(false, 'nope');
        }),
      ).message,
    ).toBe('AssertionError: nope (assert.ok)');
    const err = thrown(() => {
      assert.notEqual(1, 1);
    });
    expect(describeError(err).message).toBe(
      'AssertionError: expected anything but 1, got 1 (assert.notEqual)',
    );
  });

  it('describes other errors and non-errors', () => {
    expect(describeError(new TypeError('bad\nmore'), 3).message).toBe(
      'TypeError: bad (block line 3)',
    );
    expect(describeError('plain').message).toBe("threw 'plain'");
  });
});

describe('show', () => {
  it('is one bounded line', () => {
    expect(show({ a: [1, 2] })).toBe('{ a: [ 1, 2 ] }');
    expect(show(new Error('x\ny'))).toBe('[Error: x]');
    expect(show('z'.repeat(1000)).length).toBeLessThanOrEqual(400);
  });
});

describe('lineIn', () => {
  it('maps the first stack frame in the block back to its source line', () => {
    const err = new Error('x');
    err.stack = 'Error: x\n    at /p/docs/__b.ts:3:5\n    at elsewhere.js:1:1';
    expect(lineIn(err, '/p/docs/__b.ts', [undefined, 0, undefined, 1])).toBe(1);
    expect(lineIn(err, '/p/docs/__b.ts', [undefined, 4])).toBe(5);
    expect(lineIn(err, '/other.ts', [0])).toBeUndefined();
  });
});
