// Recorded outputs ("goldens") for the parity harnesses (tests/parity/*.ts and
// tests/parity/bench.ts), which hold the core's output to them byte for byte.
//
// Where the recordings come from, explained here once for every harness: most were printed by
// "the reference", a TypeScript implementation of docattest that the core was written to
// match and then replaced. It no longer exists, so its recordings are the statement of its
// behaviour that the core keeps. Its unit tests live on as parity cases, and the medians in
// each bench project's timing.json are filed under `reference`. The other recordings were made
// from the core on purpose, with `--update`: the cases of tests/parity/extra/differing.ts,
// where the core does by design what the reference refused, and each deliberate change of
// behaviour since. A case file's `# recorded from:` line says which.
//
// A directory of recordings holds one text file per case, named after the case, and a
// RECORDED.json with the clock the cases were recorded at. A case file is what the harness
// observed, step by step (tests/parity/recording-format.ts says how it is written).
//
// The harnesses compare by default and never write. `--update` writes what the candidate
// printed for every case that differs or has no recording yet, and says which; it is how a
// deliberate change of behaviour is recorded. Nothing is ever updated silently.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { serialise, splitSteps, type Step } from './recording-format.ts';

/** Where two recordings first differ, for a message. */
export interface FirstDifference {
  line: number;
  recorded: string;
  actual: string;
  context: string[];
}

/** A case compared with its recording: how many of its steps match, and where it differs. */
export type Outcome =
  | { same: true; steps: number; written?: true }
  | {
      same: false;
      steps: number;
      recorded: string;
      actual: string;
      file: string;
      first: FirstDifference;
    };

/** What a harness that ran some cases says about them, and whether it wrote. */
export interface RecordingsOptions {
  update?: boolean;
  source?: string;
}

const slugOf = (name: string): string =>
  name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 90) || 'case';

/** File names for every case: the name's slug, with a short hash where two slugs collide. */
function fileNames(names: readonly string[]): Map<string, string> {
  const bySlug = new Map<string, string[]>();
  for (const name of names) bySlug.set(slugOf(name), [...(bySlug.get(slugOf(name)) ?? []), name]);
  const files = new Map<string, string>();
  for (const [slug, all] of bySlug) {
    for (const name of all) {
      const hash = createHash('sha1').update(name).digest('hex').slice(0, 8);
      files.set(name, `${all.length === 1 ? slug : `${slug}-${hash}`}.txt`);
    }
  }
  return files;
}

const header = (name: string, source: string): string => `# ${name}\n# recorded from: ${source}\n`;
const body = (text: string): string =>
  text
    .split('\n')
    .filter((line, k) => !(k < 2 && line.startsWith('# ')))
    .join('\n');

/**
 * The recordings in `dir` for a harness whose cases are `names` (every case it defines,
 * whether or not this run chose them). `update` writes; `source` says, in each file
 * written, what printed it.
 */
export function recordings(
  dir: string,
  names: readonly string[],
  { update = false, source = 'docattest core' }: RecordingsOptions = {},
): Recordings {
  return new Recordings(dir, names, update, source);
}

/** The recordings of one harness, compared with what its cases print now. */
export class Recordings {
  /** The clock every case is run at: the instant the recordings were first made. */
  readonly recordedAt: string;
  /** The cases compared so far. */
  readonly seen = new Set<string>();
  private readonly report = {
    same: [] as string[],
    changed: [] as string[],
    added: [] as string[],
    missing: [] as string[],
  };
  private readonly files: Map<string, string>;
  private readonly metaPath: string;
  private readonly dir: string;
  private readonly update: boolean;
  private readonly source: string;

  constructor(dir: string, names: readonly string[], update: boolean, source: string) {
    if (new Set(names).size !== names.length) throw new Error(`${dir}: two cases share a name`);
    this.dir = dir;
    this.update = update;
    this.source = source;
    this.files = fileNames(names);
    this.metaPath = join(dir, 'RECORDED.json');
    if (!update && !existsSync(this.metaPath)) {
      throw new Error(`no recordings in ${dir}: record them with --update`);
    }
    const meta = existsSync(this.metaPath)
      ? (JSON.parse(readFileSync(this.metaPath, 'utf8')) as { recordedAt: string })
      : { recordedAt: new Date().toISOString() };
    this.recordedAt = meta.recordedAt;
  }

  /**
   * Compare a case with its recording; with `update` (unless `write` is false), write it
   * when it differs.
   */
  check(name: string, steps: readonly Step[], { write = this.update } = {}): Outcome {
    const path = this.pathOf(name);
    this.seen.add(name);
    const actual = serialise(steps);
    const recorded = existsSync(path) ? body(readFileSync(path, 'utf8')) : undefined;
    if (recorded === actual) {
      this.report.same.push(name);
      return { same: true, steps: steps.length };
    }
    if (write) {
      mkdirSync(this.dir, { recursive: true });
      writeFileSync(path, header(name, this.source) + actual);
      this.report[recorded === undefined ? 'added' : 'changed'].push(name);
      return { same: true, written: true, steps: steps.length };
    }
    // Compared only, in a run that records: not a recording that is out of date.
    if (!this.update) this.report[recorded === undefined ? 'missing' : 'changed'].push(name);
    return difference(steps, recorded ?? '', actual, path);
  }

  /**
   * Say what was written, and remove recordings of cases the harness no longer has (only
   * when every case was run). Returns whether every case matched its recording.
   */
  finish({ all }: { all: boolean }): boolean {
    if (!this.update) return this.report.changed.length === 0 && this.report.missing.length === 0;
    mkdirSync(this.dir, { recursive: true });
    if (!existsSync(this.metaPath)) {
      writeFileSync(this.metaPath, `${JSON.stringify({ recordedAt: this.recordedAt }, null, 2)}\n`);
    }
    const names = [...this.files.values()];
    const stale = all
      ? readdirSync(this.dir).filter((f) => f.endsWith('.txt') && !names.includes(f))
      : [];
    for (const f of stale) rmSync(join(this.dir, f));
    this.say(stale);
    return true;
  }

  /** Where a case is recorded; only the harness's own cases are. */
  private pathOf(name: string): string {
    const file = this.files.get(name);
    if (!file) throw new Error(`case "${name}" is not one of the harness's cases`);
    return join(this.dir, file);
  }

  private say(stale: readonly string[]): void {
    const { added, changed, same } = this.report;
    for (const name of added) console.log(`recorded (new): ${name}`);
    for (const name of changed) console.log(`recorded (changed): ${name}  [${this.pathOf(name)}]`);
    for (const f of stale) console.log(`removed (no such case): ${join(this.dir, f)}`);
    console.log(
      `recordings in ${this.dir}: ${changed.length} changed, ${added.length} new, ` +
        `${stale.length} removed, ${same.length} unchanged`,
    );
  }
}

/** A case unlike its recording, with how many of its steps still are (as a harness counts). */
function difference(
  steps: readonly Step[],
  recorded: string,
  actual: string,
  file: string,
): Outcome {
  const was = splitSteps(recorded);
  const same = steps.filter((step, k) => serialise([step]) === was[k]).length;
  return {
    same: false,
    steps: same,
    recorded,
    actual,
    file,
    first: firstDifference(recorded, actual),
  };
}

/** The first line where two recordings differ, for a message. */
function firstDifference(recorded: string, actual: string): FirstDifference {
  const [x, y] = [recorded.split('\n'), actual.split('\n')];
  const at = x.findIndex((line, k) => line !== y[k]);
  const k = at === -1 ? Math.min(x.length, y.length) : at;
  return {
    line: k + 1,
    recorded: x[k] ?? '(end)',
    actual: y[k] ?? '(end)',
    context: x.slice(Math.max(0, k - 3), k),
  };
}

/** Print a case that differs from its recording. */
export function explain(label: string, outcome: Outcome): void {
  if (outcome.same) return;
  if (outcome.recorded === '') {
    console.log(`✗ ${label}: no recording (record it with --update)`);
    return;
  }
  const { line, recorded, actual } = outcome.first;
  console.log(`✗ ${label}  (${outcome.file}, line ${line + 2})`);
  console.log(`    recorded: ${JSON.stringify(recorded).slice(0, 400)}`);
  console.log(`    now:      ${JSON.stringify(actual).slice(0, 400)}`);
}
