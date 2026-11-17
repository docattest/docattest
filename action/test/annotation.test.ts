import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { annotation } from '../annotation.ts';
import type { Finding } from '../findings.ts';

const finding: Finding = {
  page: 'docs/retries.md',
  line: 3,
  level: 'error',
  title: 'docattest fact: fail',
  message: 'expected 3, got 5 from RETRY.maxAttempts',
};

describe('annotation', () => {
  it('names the file, the line and the title, then the message', () => {
    assert.equal(
      annotation(finding),
      '::error file=docs/retries.md,line=3,title=docattest fact%3A fail::expected 3, got 5 from RETRY.maxAttempts',
    );
  });

  it('keeps a message with line breaks and percent signs to one command', () => {
    const command = annotation({ ...finding, message: '100% sure\nsecond line' });
    assert.equal(command.split('\n').length, 1);
    assert.ok(command.endsWith('::100%25 sure%0Asecond line'));
  });

  it('escapes a comma in the page so it does not start another property', () => {
    assert.match(annotation({ ...finding, page: 'docs/a,b.md' }), /file=docs\/a%2Cb\.md,line=3,/);
  });

  it('writes a warning as a warning', () => {
    assert.match(annotation({ ...finding, level: 'warning' }), /^::warning /);
  });
});
