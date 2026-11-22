import { describe, expect, it } from 'vitest';
import { compareOutput, expectedOutput } from '../../../../src/language/syntax/output.js';

describe('expectedOutput', () => {
  it('parses single-line, multi-line and unordered output comments', () => {
    expect(expectedOutput("console.log('hi');\n// Output: hi\n")).toEqual({
      text: 'hi',
      unordered: false,
    });
    expect(expectedOutput("console.log('b');\n// Unordered output:\n// a\n// b\n")).toEqual({
      text: 'a\nb',
      unordered: true,
    });
    expect(expectedOutput('// Output: 1\nconsole.log(1);\n// trailing remark\n')).toBeUndefined();
  });

  it('compares ordered and unordered stdout', () => {
    expect(compareOutput('b\na\n', { text: 'a\nb', unordered: true }).status).toBe('pass');
    const bad = compareOutput('b\na\n', { text: 'a\nb', unordered: false });
    expect(bad.status).toBe('fail');
    expect(bad.expected).toBe('a\nb');
    expect(bad.actual).toBe('b\na');
  });
});
