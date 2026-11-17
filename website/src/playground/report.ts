/**
 * Turns one recorded command into what the playground's terminal and gutter show. Pure:
 * nothing here touches the page.
 */
import type { CommandOutput } from './recordingFormat.ts';

/** The docs page every stop's proofs live on. */
export const PAGE = 'docs/retries.md';

export type Tone = 'plain' | 'quiet' | 'good' | 'bad' | 'warn';

export interface TerminalLine {
  readonly text: string;
  readonly tone: Tone;
}

/** A line of the docs page that a command's output points at. */
export interface PageMark {
  readonly line: number;
  readonly symbol: string;
  readonly tone: Tone;
}

export interface Stamp {
  readonly text: string;
  readonly tone: 'good' | 'bad';
}

/** The words a command's verdict is stamped with, when the stop chooses its own. */
export interface StampWords {
  readonly id: string;
  readonly refusedStamp?: string;
  readonly passedStamp?: string;
}

const VERDICT_MARK = /^\s+([✓✗!?]) (?:L|docs\/retries\.md:)(\d+)\b/;
const POINTER = /docs\/retries\.md:(\d+) (?:proven )?(?:fact|example|cases|test)\b/;
const VERDICT_TONES: Readonly<Record<string, Tone>> = {
  '✓': 'good',
  '✗': 'bad',
  '!': 'bad',
  '?': 'warn',
};

/** The terminal lines for a recorded command: prompt, event, then its output. */
export function terminalLines(output: CommandOutput): TerminalLine[] {
  const printed = outputLines(output);
  const event: TerminalLine[] =
    output.stdin === undefined ? [] : [{ text: `event on stdin: ${output.stdin}`, tone: 'quiet' }];
  const body: TerminalLine[] =
    printed.length > 0
      ? printed.map((text) => ({ text, tone: toneOf(text) }))
      : [{ text: '(prints nothing)', tone: 'quiet' }];
  return [{ text: `$ ${output.command}`, tone: 'plain' }, ...event, ...body];
}

/** The lines of the docs page a command's output points at, for the gutter. */
export function pageMarks(output: CommandOutput): PageMark[] {
  return outputLines(output).flatMap((text): PageMark[] => {
    const verdict = VERDICT_MARK.exec(text);
    if (verdict?.[1] !== undefined) {
      const symbol = verdict[1];
      return [{ line: Number(verdict[2]), symbol, tone: VERDICT_TONES[symbol] ?? 'plain' }];
    }
    const pointer = POINTER.exec(text);
    return pointer === null ? [] : [{ line: Number(pointer[1]), symbol: '→', tone: 'warn' }];
  });
}

/** The word stamped on the terminal, or null when the command gives no verdict. */
export function stampFor(command: StampWords, output: CommandOutput): Stamp | null {
  const refused = output.exitCode !== 0 || output.stdout.includes('"decision":"block"');
  if (refused) return { text: command.refusedStamp ?? 'Busted', tone: 'bad' };
  const passed = command.passedStamp ?? (command.id.startsWith('check') ? 'Holds up' : undefined);
  return passed === undefined ? null : { text: passed, tone: 'good' };
}

function outputLines(output: CommandOutput): string[] {
  const text = `${output.stdout}${output.stderr}`.trimEnd();
  if (text === '') return [];
  return text.startsWith('{') ? unfoldedJson(text) : text.split('\n');
}

/* A hook answers in one line of JSON whose text is full of \n. Shown as the JSON with that
   text lifted out below it, so it can be read. */
function unfoldedJson(text: string): string[] {
  const lifted: string[] = [];
  const shape = JSON.stringify(
    JSON.parse(text),
    (_key, value: unknown) => {
      if (typeof value !== 'string' || !value.includes('\n')) return value;
      lifted.push(value);
      return '↓ the text below';
    },
    2,
  );
  return [...shape.split('\n'), ...lifted.flatMap((value) => value.split('\n'))];
}

function toneOf(line: string): Tone {
  const text = line.trim();
  if (text === 'ok') return 'good';
  if (text === 'FAILED') return 'bad';
  return VERDICT_TONES[text.charAt(0)] ?? 'plain';
}
