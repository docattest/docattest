import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { proseLines } from './prose-lines.ts';

const prose = (...lines: string[]): string[] => proseLines(lines.join('\n'));

describe('proseLines', () => {
  it('blanks a fenced block, its fences included, and code spans', () => {
    assert.deepEqual(prose('a [x](x.md)', '```ts', '[y](y.md)', '```', 'b `[z](z.md)` c'), [
      'a [x](x.md)',
      '',
      '',
      '',
      'b  c',
    ]);
  });

  it('opens no fence at a code span of three backticks, whose info string would hold one', () => {
    assert.deepEqual(prose('- ```` ```python check ```` is a block', '## After'), [
      '-  is a block',
      '## After',
    ]);
  });

  it('opens a tilde fence whose info string holds a backtick', () => {
    assert.deepEqual(prose('~~~ `odd`', '## Inside', '~~~', '## After'), ['', '', '', '## After']);
  });

  it('closes a fence only at a fence of its character, as long, with nothing after it', () => {
    assert.deepEqual(prose('````md', '```ts', '```', '~~~~', '```` x', '````', '## After'), [
      '',
      '',
      '',
      '',
      '',
      '',
      '## After',
    ]);
  });
});
