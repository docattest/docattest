// `createProject`: the programmatic API, as a thin client of the docattest core. Every
// operation runs the core's command (`check --json`, `index --json`, …) on the project and
// returns what it prints; nothing of docattest's own logic is in this package.
import { UserError } from '../api/errors.js';
import type { Impact, ImpactOptions } from '../api/impact.js';
import type { MutateOptions, Mutation } from '../api/mutate.js';
import type { CheckOptions, Project, ProjectInput } from '../api/project.js';
import type { Report } from '../api/report.js';
import {
  asAudit,
  asExplanation,
  asImpact,
  asIndex,
  asInventory,
  asMutation,
  asNames,
  asReferences,
  asReport,
} from './answers.js';
import { answer, core, failure, runCore, type CoreRun } from './core.js';
import { placeOf, type Place, type Workspace } from './place.js';

/** One run of the core on the project: a command and its arguments. */
type Run = (at: Workspace, command: string[]) => Promise<CoreRun>;

/** A project from a directory or an in-memory map of files. */
export function createProject(input: ProjectInput): Project {
  const place = placeOf(input);
  const env = environment(input);
  const written: string[] = [];
  // The core is looked up once, at the project's first run.
  let program: string | undefined;
  // `--root` comes before any `--`, after which every argument is a page or a name.
  const run: Run = (at, [command = '', ...args]) => {
    program ??= core();
    return runCore(program, [command, '--root', at.root, ...args], at.root, env);
  };
  const query = async (args: string[]) =>
    place.run(undefined, async (at) => answer(await run(at, args)));
  return {
    check: (options = {}) => check(place, run, options),
    index: async () => asIndex(await query(['index', '--json'])),
    explain: async (page, line) => {
      const target = line === undefined ? page : `${page}:${line}`;
      return asExplanation(await query(['explain', '--json', '--', target]));
    },
    where: async (symbol) => asReferences(await query(['where', '--json', '--', symbol])),
    uncovered: async (surface = 'default') =>
      asNames(await query(['uncovered', '--json', `--surface=${surface}`])),
    audit: async () => asAudit(await query(['audit', '--json'])),
    inventory: async (search) => {
      const args = ['inventory', '--json', ...(search === undefined ? [] : ['--', search])];
      return asInventory(await query(args));
    },
    impact: (options = {}) => impact(place, run, options),
    mutate: (options = {}) => mutate(place, run, options),
    async refresh(name?: string, ...rest: unknown[]) {
      if (rest.length > 0) {
        throw new UserError(
          'refresh takes no fetcher: the docattest core fetches each snapshot from its source ' +
            'itself; serve a test source over HTTP instead',
        );
      }
      const files = await refresh(place, run, name);
      for (const file of files) if (!written.includes(file)) written.push(file);
      return files.map((file) => file.slice(file.lastIndexOf('/') + 1, -'.json'.length));
    },
    writes: () => [...written],
  };
}

async function check(place: Place, run: Run, options: CheckOptions): Promise<Report> {
  const { base, pages = [] } = options;
  if (typeof base === 'string' && place.memory) {
    throw new UserError('a git ref as base needs a project on disk; pass `base: { files }`');
  }
  const files = typeof base === 'object' ? base.files : undefined;
  // Files given as the base are committed as HEAD in the project's temporary directory.
  const ref = typeof base === 'string' ? base : files ? 'HEAD' : undefined;
  return place.run(files, async (at) => {
    const args = ['check', '--json', ...(ref === undefined ? [] : [`--base=${ref}`])];
    return asReport(answer(await run(at, [...args, '--', ...pages]), [0, 1]));
  });
}

async function mutate(place: Place, run: Run, options: MutateOptions): Promise<Mutation> {
  const { base, pages = [] } = options;
  if (typeof base === 'string' && place.memory) {
    throw new UserError('a git ref as base needs a project on disk; pass `base: { files }`');
  }
  const files = typeof base === 'object' ? base.files : undefined;
  const ref = typeof base === 'string' ? base : files ? 'HEAD' : undefined;
  return place.run(files, async (at) => {
    const args = ['mutate', '--json', ...(ref === undefined ? [] : [`--base=${ref}`])];
    return asMutation(answer(await run(at, [...args, '--', ...pages])));
  });
}

async function impact(place: Place, run: Run, options: ImpactOptions): Promise<Impact> {
  const { baseFiles } = options;
  if (baseFiles === undefined && place.memory) {
    throw new UserError('an in-memory project has no git history: pass `baseFiles` to impact');
  }
  return place.run(baseFiles, async (at) => {
    const ref = baseFiles === undefined ? (options.base ?? 'HEAD') : 'HEAD';
    const found = asImpact(answer(await run(at, ['impact', '--json', `--base=${ref}`])));
    // Given files are committed as HEAD; the answer is labelled as the caller labelled it.
    return baseFiles === undefined ? found : { ...found, base: options.base ?? 'files' };
  });
}

/** Run `refresh`; returns the files it wrote, in its order, and keeps them in the place. */
async function refresh(place: Place, run: Run, name: string | undefined): Promise<string[]> {
  return place.run(undefined, async (at) => {
    const done = await run(at, ['refresh', ...(name === undefined ? [] : ['--', name])]);
    if (done.code !== 0) throw failure(done);
    const files = done.stdout
      .split('\n')
      .flatMap((line) => (line.startsWith('wrote ') ? [line.slice('wrote '.length)] : []));
    for (const file of files) place.keep(file, at.read(file));
    return files;
  });
}

/** `env.now` and `env.network`, as the core reads them. */
function environment(input: ProjectInput): Record<string, string> {
  const env: Record<string, string> = {};
  const { now, network } = input.env ?? {};
  if (now !== undefined) {
    if (Number.isNaN(now.getTime())) throw new UserError('`env.now` is not a valid date');
    env.DOCATTEST_NOW = now.toISOString();
  }
  if (network !== undefined) env.DOCATTEST_NETWORK = network ? '1' : '0';
  return env;
}
