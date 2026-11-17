import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { CommandOutput } from '../../src/playground/recordingFormat.ts';
import { pageMarks, stampFor, terminalLines } from '../../src/playground/report.ts';

const run = (stdout: string, exitCode = 0, stdin?: string): CommandOutput => ({
  command: 'npx docattest check',
  stdout,
  stderr: '',
  exitCode,
  ...(stdin === undefined ? {} : { stdin }),
});

describe('terminalLines', () => {
  it('shows the prompt, then each printed line in the tone of its verdict', () => {
    const lines = terminalLines(run('docs/retries.md\n  ✓ L5 example\n  ✗ L10 fact\nFAILED\n', 1));
    assert.deepEqual(lines, [
      { text: '$ npx docattest check', tone: 'plain' },
      { text: 'docs/retries.md', tone: 'plain' },
      { text: '  ✓ L5 example', tone: 'good' },
      { text: '  ✗ L10 fact', tone: 'bad' },
      { text: 'FAILED', tone: 'bad' },
    ]);
  });

  it('says so when a command prints nothing', () => {
    assert.deepEqual(terminalLines(run('')).at(-1), { text: '(prints nothing)', tone: 'quiet' });
  });

  it('shows the event a hook reads, and lifts a reply’s multi-line text out of its JSON', () => {
    const reply = JSON.stringify({ decision: 'block', reason: 'failed:\n  ✗ L10 fact' });
    const texts = terminalLines(run(reply, 0, '{"stop_hook_active":false}')).map((l) => l.text);
    assert.deepEqual(texts, [
      '$ npx docattest check',
      'event on stdin: {"stop_hook_active":false}',
      '{',
      '  "decision": "block",',
      '  "reason": "↓ the text below"',
      '}',
      'failed:',
      '  ✗ L10 fact',
    ]);
  });
});

describe('pageMarks', () => {
  it('marks the docs page lines a verdict or a pointer names', () => {
    const output = run(
      '  ✗ L10 fact: expected 3\n  ? docs/retries.md:31 list\nRETRY\n  docs/retries.md:10 proven fact  Tried\n',
    );
    assert.deepEqual(pageMarks(output), [
      { line: 10, symbol: '✗', tone: 'bad' },
      { line: 31, symbol: '?', tone: 'warn' },
      { line: 10, symbol: '→', tone: 'warn' },
    ]);
  });
});

describe('stampFor', () => {
  it('stamps a failure, or a hook that blocks, with the refusal', () => {
    assert.deepEqual(stampFor({ id: 'check' }, run('', 1)), { text: 'Busted', tone: 'bad' });
    assert.deepEqual(
      stampFor({ id: 'check-hook', refusedStamp: 'Sent back' }, run('{"decision":"block"}')),
      {
        text: 'Sent back',
        tone: 'bad',
      },
    );
  });

  it('stamps a passing check, and gives other commands no stamp unless the stop names one', () => {
    assert.deepEqual(stampFor({ id: 'check' }, run('ok')), { text: 'Holds up', tone: 'good' });
    assert.equal(stampFor({ id: 'where' }, run('docs/retries.md:10 fact')), null);
    assert.deepEqual(stampFor({ id: 'commit', passedStamp: 'Committed' }, run('')), {
      text: 'Committed',
      tone: 'good',
    });
  });
});
