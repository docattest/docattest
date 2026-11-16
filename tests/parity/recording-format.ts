// How a parity case is written in its recording: what the harness observed, step by step,
// written so that a diff of it reads as a diff of the output (tests/parity/recordings.ts
// says where the recordings are, and when they are written):
//
//   # <the case's exact name>
//   # recorded from: <where the output came from>
//   === <the command line's arguments>
//   exit 1
//   stdout 2 lines
//   <the lines>
//   stderr empty
//
// Text that holds a control character (a carriage return, a NUL) or a lone surrogate is
// written as one JSON string instead (`stdout json`), so that every file stays plain text.

/** Files a step saw by path: their text, `null` for one that is gone, `'directory'` for a `path/`. */
export type ObservedFiles = Record<string, string | null>;

/**
 * A step: the arguments it ran, its exit code, stdout and stderr, and optionally the files
 * that changed (path → contents, `null` for a file that is gone).
 */
export interface Step {
  label: string;
  /** The exit code, or why there is none (a process killed by a signal has `null`). */
  code: number | string | null;
  stdout: string;
  stderr: string;
  files?: ObservedFiles;
}

const CONTROL =
  // eslint-disable-next-line no-control-regex -- control characters are what it finds
  /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]|[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/;

/** One piece of text, as a recording writes it. */
function block(tag: string, text: string): string[] {
  if (text === '') return [`${tag} empty`];
  if (CONTROL.test(text)) return [`${tag} json`, JSON.stringify(text)];
  const lines = text.split('\n');
  const ended = lines.at(-1) === '';
  if (ended) lines.pop();
  const count = `${lines.length} line${lines.length === 1 ? '' : 's'}`;
  return [`${tag} ${count}${ended ? '' : ', no newline at end'}`, ...lines];
}

/** Steps as a recording writes them. */
export function serialise(steps: readonly Step[]): string {
  const out: string[] = [];
  for (const step of steps) {
    out.push(
      `=== ${step.label}`,
      `exit ${String(step.code)}`,
      ...block('stdout', step.stdout),
      ...block('stderr', step.stderr),
    );
    for (const [path, text] of Object.entries(step.files ?? {})) {
      if (text === null) out.push(`removed ${path}`);
      else if (text === 'directory' && path.endsWith('/')) out.push(`directory ${path}`);
      else out.push(...block(`file ${path}`, text));
    }
  }
  return `${out.join('\n')}\n`;
}

/** The files of `after` that are not as they were in `before`; `null` for those gone. */
export function changedFiles(
  before: Readonly<ObservedFiles>,
  after: Readonly<ObservedFiles>,
): ObservedFiles {
  const changed: ObservedFiles = {};
  for (const path of [...new Set([...Object.keys(before), ...Object.keys(after)])].sort()) {
    if (!(path in after)) changed[path] = null;
    else if (before[path] !== after[path]) changed[path] = after[path] ?? null;
  }
  return changed;
}

/** A recording cut into its steps, each as `serialise` wrote it. */
export function splitSteps(text: string): string[] {
  const lines = text.split('\n');
  const steps: string[] = [];
  let k = 0;
  while (lines[k]?.startsWith('=== ')) {
    const start = k;
    k += 2; // `=== <label>` and `exit <code>`
    for (let line = lines[k]; line !== undefined && !line.startsWith('=== ') && line !== '';) {
      k += 1 + extraLines(line);
      line = lines[k];
    }
    steps.push(`${lines.slice(start, k).join('\n')}\n`);
  }
  return steps;
}

/** How many lines follow a block's tag line: its text's, one for `json`, none when empty. */
function extraLines(tag: string): number {
  if (/^(removed|directory) /.test(tag) || tag.endsWith(' empty')) return 0;
  if (tag.endsWith(' json')) return 1;
  return Number(/ (\d+) lines?(, no newline at end)?$/.exec(tag)?.[1] ?? 0);
}
